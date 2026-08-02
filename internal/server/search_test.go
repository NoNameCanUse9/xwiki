package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"testing"
)

func TestSearchAfterWriteAndRevert(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "search-site")

	// Write a document with a distinctive keyword.
	base := getRevision(t, h, cookie, projectID)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"add searchable",
		  "changes":[{"op":"create","path":"docs/keyword.md","content":"# K\n\nwalrus pineapple\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("write: %d %s", rec.Code, rec.Body.String())
	}

	// Search finds it right after the write (incremental index hook).
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/search?q=pineapple", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("search: %d %s", rec.Code, rec.Body.String())
	}
	var body struct {
		Results []struct {
			Path    string `json:"path"`
			Snippet string `json:"snippet"`
		} `json:"results"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if len(body.Results) != 1 || body.Results[0].Path != "docs/keyword.md" {
		t.Fatalf("search results wrong: %+v", body.Results)
	}

	// Empty query -> 400.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/search?q=", cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("empty query: %d", rec.Code)
	}

	// Revert removes the document from the index.
	head := getRevision(t, h, cookie, projectID)
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/commits/"+head+"/revert", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("revert: %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/search?q=pineapple", cookie, "")
	_ = json.Unmarshal(rec.Body.Bytes(), &body)
	if len(body.Results) != 0 {
		t.Fatalf("reverted doc still searchable: %+v", body.Results)
	}
}
