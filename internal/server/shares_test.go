package server

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"
)

func TestShareLinkLifecycle(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "share-site")

	// Seed a page.
	rev := getRevision(t, h, cookie, projectID)
	submitChangeset(t, h, cookie, projectID,
		`{"base_revision":"`+rev+`","message":"seed",
		  "changes":[{"op":"create","path":"guide.md","content":"# Guide\n\nShared **content**.\n"}]}`)

	// Create a share for that single page.
	rec := apiRequest(h, http.MethodPost, "/api/v1/projects/"+projectID+"/shares",
		cookie, `{"path":"guide.md"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("create share: %d %s", rec.Code, rec.Body.String())
	}
	var body struct {
		Token string `json:"token"`
		URL   string `json:"url"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Token == "" || !strings.HasPrefix(body.URL, "/share/") {
		t.Fatalf("bad share response: %+v", body)
	}

	// Re-sharing the same page returns the same token (idempotent).
	rec2 := apiRequest(h, http.MethodPost, "/api/v1/projects/"+projectID+"/shares",
		cookie, `{"path":"guide.md"}`)
	var body2 struct {
		Token string `json:"token"`
	}
	if err := json.Unmarshal(rec2.Body.Bytes(), &body2); err != nil {
		t.Fatal(err)
	}
	if body2.Token != body.Token {
		t.Fatalf("re-share token differs: %s vs %s", body2.Token, body.Token)
	}

	// The share URL is publicly readable without any auth (no cookie, no UA).
	rec3 := apiRequest(h, http.MethodGet, body.URL, "", "")
	if rec3.Code != http.StatusOK {
		t.Fatalf("share view: %d %s", rec3.Code, rec3.Body.String())
	}
	if !strings.Contains(rec3.Body.String(), "<h1>Guide</h1>") ||
		!strings.Contains(rec3.Body.String(), "<strong>content</strong>") {
		t.Fatalf("share view missing content: %s", rec3.Body.String())
	}

	// Unknown share -> 404.
	if rec4 := apiRequest(h, http.MethodGet, "/share/does-not-exist", "", ""); rec4.Code != http.StatusNotFound {
		t.Fatalf("missing share: %d", rec4.Code)
	}

	// Bad share input -> 400.
	if rec5 := apiRequest(h, http.MethodPost, "/api/v1/projects/"+projectID+"/shares",
		cookie, `{"path":"../escape"}`); rec5.Code != http.StatusBadRequest {
		t.Fatalf("bad path share: %d", rec5.Code)
	}
}
