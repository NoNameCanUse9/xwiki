package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"testing"
)

func TestHistoryEndpoints(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "hist-site")

	// Seed two writes through the API (phase 4 pipeline).
	base := getRevision(t, h, cookie, projectID)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"first write",
		  "changes":[{"op":"create","path":"docs/a.md","content":"# A\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("seed 1: %d %s", rec.Code, rec.Body.String())
	}
	base = getRevision(t, h, cookie, projectID)
	rec = submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"second write",
		  "changes":[{"op":"update","path":"docs/a.md","content":"# A v2\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("seed 2: %d %s", rec.Code, rec.Body.String())
	}

	// Commits list contains root README + both API writes (acceptance 1).
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("commits: %d", rec.Code)
	}
	var list struct {
		Commits []struct {
			SHA     string `json:"sha"`
			Message string `json:"message"`
		} `json:"commits"`
		HasMore bool `json:"has_more"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	if len(list.Commits) != 3 {
		t.Fatalf("want 3 commits, got %d", len(list.Commits))
	}
	if list.Commits[0].Message != "second write" || list.Commits[2].Message != "Initialize project hist-site" {
		t.Fatalf("commit list wrong: %+v", list.Commits)
	}
	headSHA := list.Commits[0].SHA

	// Search applies before pagination and reports whether another page exists.
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits?q=write&limit=1&offset=0", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("search commits: %d body=%s", rec.Code, rec.Body.String())
	}
	var searched struct {
		Commits []struct {
			Message string `json:"message"`
		} `json:"commits"`
		HasMore bool `json:"has_more"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &searched); err != nil {
		t.Fatal(err)
	}
	if len(searched.Commits) != 1 || !strings.Contains(searched.Commits[0].Message, "write") || !searched.HasMore {
		t.Fatalf("search page = %+v", searched)
	}
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits?q=write&limit=1&offset=1", cookie, "")
	if err := json.Unmarshal(rec.Body.Bytes(), &searched); err != nil {
		t.Fatal(err)
	}
	if len(searched.Commits) != 1 || searched.HasMore {
		t.Fatalf("last search page = %+v", searched)
	}

	// Commit detail with file list.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/commits/"+headSHA, cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("commit detail: %d", rec.Code)
	}
	var detail struct {
		Commit struct {
			Files []struct {
				Status string `json:"status"`
				Path   string `json:"path"`
			} `json:"files"`
		} `json:"commit"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &detail); err != nil {
		t.Fatal(err)
	}
	if len(detail.Commit.Files) != 1 || detail.Commit.Files[0].Path != "docs/a.md" {
		t.Fatalf("detail files wrong: %+v", detail.Commit.Files)
	}

	// File history.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/files/history/docs/a.md", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("file history: %d body=%s", rec.Code, rec.Body.String())
	}
	var fh struct {
		Commits []map[string]string `json:"commits"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &fh); err != nil {
		t.Fatal(err)
	}
	if len(fh.Commits) != 2 {
		t.Fatalf("a.md history: want 2, got %d", len(fh.Commits))
	}

	// Diff numstat + patch.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/commits/"+headSHA+"/diff?format=numstat", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("numstat: %d", rec.Code)
	}
	var diff struct {
		Stats []struct {
			Path    string `json:"path"`
			Added   int    `json:"added"`
			Deleted int    `json:"deleted"`
		} `json:"stats"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &diff); err != nil {
		t.Fatal(err)
	}
	if len(diff.Stats) != 1 || diff.Stats[0].Path != "docs/a.md" {
		t.Fatalf("numstat wrong: %+v", diff.Stats)
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/commits/"+headSHA+"/diff?format=patch", cookie, "")
	if !strings.Contains(rec.Body.String(), "diff --git") {
		t.Fatalf("patch wrong: %.120s", rec.Body.String())
	}

	// Revert -> new commit, count +1, original still listed (acceptance 2).
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/commits/"+headSHA+"/revert", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("revert: %d %s", rec.Code, rec.Body.String())
	}
	var rev struct {
		Commit struct {
			SHA string `json:"sha"`
		} `json:"commit"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &rev); err != nil {
		t.Fatal(err)
	}
	if rev.Commit.SHA == "" || rev.Commit.SHA == headSHA {
		t.Fatalf("revert sha wrong: %+v", rev)
	}
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits", cookie, "")
	_ = json.Unmarshal(rec.Body.Bytes(), &list)
	if len(list.Commits) != 4 {
		t.Fatalf("want 4 commits after revert, got %d", len(list.Commits))
	}
	// Content reverted: a.md back to v1.
	page := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/a.md", cookie, "")
	if !strings.Contains(page.Body.String(), "# A") || strings.Contains(page.Body.String(), "v2") {
		t.Fatalf("a.md not reverted: %s", page.Body.String())
	}

	// Unknown commit -> 404.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/commits/deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("unknown commit: %d", rec.Code)
	}
	// Unauthenticated -> 401.
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits", "", "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("anon commits: %d", rec.Code)
	}
}
