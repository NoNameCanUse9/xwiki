package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func getRevision(t *testing.T, h http.Handler, cookie, projectID string) string {
	t.Helper()
	rec := apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/revision", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("revision: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var body struct {
		Revision string `json:"revision"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	return body.Revision
}

func submitChangeset(t *testing.T, h http.Handler, cookie, projectID, payload string) *httptest.ResponseRecorder {
	t.Helper()
	t.Helper()
	return apiRequest(h, http.MethodPost, "/api/v1/projects/"+projectID+"/changesets", cookie, payload)
}

func TestChangesetLifecycle(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "write-site")

	// Revision endpoint.
	base := getRevision(t, h, cookie, projectID)
	if len(base) != 40 {
		t.Fatalf("bad revision %q", base)
	}

	// Successful changeset: create + update in one commit.
	payload := fmt.Sprintf(`{"base_revision":"%s","message":"write docs",
	  "changes":[
	    {"op":"create","path":"docs/guide.md","content":"# Guide\n"},
	    {"op":"update","path":"README.md","content":"# write-site\n\nchanged\n"}
	  ]}`, base)
	rec := submitChangeset(t, h, cookie, projectID, payload)
	if rec.Code != http.StatusOK {
		t.Fatalf("changeset: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var res struct {
		Commit   string `json:"commit"`
		Revision string `json:"revision"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &res); err != nil {
		t.Fatal(err)
	}
	if len(res.Commit) != 40 || res.Revision != res.Commit {
		t.Fatalf("bad changeset response: %+v", res)
	}
	// The new content is immediately readable.
	page := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/guide.md", cookie, "")
	if page.Code != http.StatusOK || !strings.Contains(page.Body.String(), "# Guide") {
		t.Fatalf("new doc not readable: %d %s", page.Code, page.Body.String())
	}

	// Stale revision -> 409.
	stale := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"stale",
		  "changes":[{"op":"create","path":"b.md","content":"b"}]}`, base))
	if stale.Code != http.StatusConflict {
		t.Fatalf("stale: status = %d, want 409", stale.Code)
	}

	// Invalid changeset -> 400.
	bad := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"bad",
		  "changes":[{"op":"create","path":"../evil.md","content":"x"}]}`, res.Revision))
	if bad.Code != http.StatusBadRequest {
		t.Fatalf("bad path: status = %d, want 400", bad.Code)
	}

	// Dry run -> no commit, preview returned.
	dry := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"dry","dry_run":true,
		  "changes":[{"op":"create","path":"dry.md","content":"d"}]}`, res.Revision))
	if dry.Code != http.StatusOK {
		t.Fatalf("dry run: status = %d body = %s", dry.Code, dry.Body.String())
	}
	var dryResp struct {
		Preview struct {
			Tree    string `json:"tree"`
			Changes []struct {
				Path string `json:"path"`
			} `json:"changes"`
		} `json:"preview"`
	}
	if err := json.Unmarshal(dry.Body.Bytes(), &dryResp); err != nil {
		t.Fatal(err)
	}
	if dryResp.Preview.Tree == "" || len(dryResp.Preview.Changes) != 1 {
		t.Fatalf("dry run response wrong: %+v", dryResp)
	}
	// dry.md must not exist after dry run.
	probe := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/dry.md", cookie, "")
	if probe.Code != http.StatusNotFound {
		t.Fatalf("dry run wrote a file: status = %d", probe.Code)
	}

	// Unauthenticated -> 401.
	anon := submitChangeset(t, h, "", projectID, payload)
	if anon.Code != http.StatusUnauthorized {
		t.Fatalf("anon: status = %d, want 401", anon.Code)
	}
}
func TestCommitAuthorFromSession(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "author-site")
	base := getRevision(t, h, cookie, projectID)
	// 空 message -> 后端生成默认（时间 + admin 修改）
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","changes":[{"op":"create","path":"docs/a.md","content":"# A\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("write: %d %s", rec.Code, rec.Body.String())
	}
	var list struct {
		Commits []struct {
			SHA     string `json:"sha"`
			Message string `json:"message"`
			Author  string `json:"author"`
		} `json:"commits"`
	}
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/commits", cookie, "")
	if err := json.Unmarshal(rec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	if len(list.Commits) < 2 {
		t.Fatalf("commits: %d", len(list.Commits))
	}
	top := list.Commits[0]
	if !strings.Contains(top.Message, "Admin 修改") || !strings.Contains(top.Message, "docs/a.md") {
		t.Fatalf("default message wrong: %q", top.Message)
	}
	if top.Author != "Admin" { // display_name of admin
		t.Fatalf("author wrong: %q", top.Author)
	}
}
