package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"testing"
)

func TestBacklinksAndHistoricalVersion(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "link-site")

	// Seed: a.md links to b.md; b.md is then updated twice.
	base := getRevision(t, h, cookie, projectID)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"seed links",
		  "changes":[
		    {"op":"create","path":"docs/a.md","content":"see [[docs/b.md|指南]] here\n"},
		    {"op":"create","path":"docs/b.md","content":"# B v1\n"}
		  ]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("seed: %d", rec.Code)
	}
	base = getRevision(t, h, cookie, projectID)
	rec = submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"update b",
		  "changes":[{"op":"update","path":"docs/b.md","content":"# B v2\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("update: %d", rec.Code)
	}

	// Historical version: read b.md as of the seed commit.
	var list struct {
		Commits []struct {
			SHA     string `json:"sha"`
			Message string `json:"message"`
		} `json:"commits"`
	}
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits", cookie, "")
	_ = json.Unmarshal(rec.Body.Bytes(), &list)
	var seedSHA string
	for _, c := range list.Commits {
		if c.Message == "seed links" {
			seedSHA = c.SHA
		}
	}
	if seedSHA == "" {
		t.Fatal("seed commit not found")
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/b.md?format=raw&at="+seedSHA, cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("historical read: %d %s", rec.Code, rec.Body.String())
	}
	var page struct {
		Content string `json:"content"`
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &page)
	if !strings.Contains(page.Content, "v1") || strings.Contains(page.Content, "v2") {
		t.Fatalf("historical content wrong: %q", page.Content)
	}
	// Current version still v2.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/b.md?format=raw", cookie, "")
	_ = json.Unmarshal(rec.Body.Bytes(), &page)
	if !strings.Contains(page.Content, "v2") {
		t.Fatalf("current content wrong: %q", page.Content)
	}

	// Backlinks: b.md is linked from a.md.
	// Reindex first so the link index is built (write hook reindexes too).
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/backlinks?path=docs/b.md", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("backlinks: %d %s", rec.Code, rec.Body.String())
	}
	var bl struct {
		Backlinks []struct {
			Source  string `json:"source"`
			Snippet string `json:"snippet"`
		} `json:"backlinks"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &bl); err != nil {
		t.Fatal(err)
	}
	if len(bl.Backlinks) != 1 || bl.Backlinks[0].Source != "docs/a.md" {
		t.Fatalf("backlinks wrong: %+v", bl.Backlinks)
	}
	if !strings.Contains(bl.Backlinks[0].Snippet, "docs/b.md") {
		t.Fatalf("snippet wrong: %q", bl.Backlinks[0].Snippet)
	}
	// No backlinks for a page nobody links to.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/backlinks?path=docs/a.md", cookie, "")
	_ = json.Unmarshal(rec.Body.Bytes(), &bl)
	if len(bl.Backlinks) != 0 {
		t.Fatalf("unexpected backlinks: %+v", bl.Backlinks)
	}
}
