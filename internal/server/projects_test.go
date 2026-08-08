package server

import (
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// loginAndGetCookie authenticates the seeded admin user and returns the
// session cookie value for authenticated API calls.
func loginAndGetCookie(t *testing.T, h http.Handler) string {
	t.Helper()
	body := strings.NewReader(`{"username":"admin","password":"secret123"}`)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/auth/login", body))
	if rec.Code != http.StatusOK {
		t.Fatalf("login: status = %d body = %s", rec.Code, rec.Body.String())
	}
	for _, c := range rec.Result().Cookies() {
		if c.Name == "agentdocs_session" {
			return c.Value
		}
	}
	t.Fatal("no session cookie set")
	return ""
}

func apiRequest(h http.Handler, method, path, cookie, payload string) *httptest.ResponseRecorder {
	var body io.Reader
	if payload != "" {
		body = strings.NewReader(payload)
	}
	req := httptest.NewRequest(method, path, body)
	if cookie != "" {
		req.AddCookie(&http.Cookie{Name: "agentdocs_session", Value: cookie})
	}
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	return rec
}

func decodeProjects(t *testing.T, rec *httptest.ResponseRecorder) []map[string]any {
	t.Helper()
	var body struct {
		Projects []map[string]any `json:"projects"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	return body.Projects
}

func TestProjectsRequireAuth(t *testing.T) {
	h := newTestRouter(t)
	rec := apiRequest(h, http.MethodGet, "/api/v1/projects", "", "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("status = %d, want 401", rec.Code)
	}
}

func TestProjectLifecycle(t *testing.T) {
	h := newTestRouter(t)
	cookie := loginAndGetCookie(t, h)

	// Create.
	rec := apiRequest(h, http.MethodPost, "/api/v1/projects", cookie,
		`{"name":"docs-site","description":"产品文档"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var created struct {
		Project struct {
			ID          string `json:"id"`
			Name        string `json:"name"`
			RepoDir     string `json:"repo_dir"`
			Archived    bool   `json:"archived"`
			Description string `json:"description"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if created.Project.ID == "" || created.Project.Name != "docs-site" ||
		created.Project.Description != "产品文档" || created.Project.Archived {
		t.Fatalf("unexpected created project: %+v", created.Project)
	}
	if !strings.HasPrefix(created.Project.RepoDir, "repos/") {
		t.Fatalf("unexpected repo_dir %q", created.Project.RepoDir)
	}
	projectID := created.Project.ID

	// Duplicate name -> 409.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects", cookie, `{"name":"docs-site"}`)
	if rec.Code != http.StatusConflict {
		t.Fatalf("duplicate: status = %d, want 409", rec.Code)
	}

	// Invalid name -> 400.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects", cookie, `{"name":"Bad Name!"}`)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("invalid name: status = %d, want 400", rec.Code)
	}

	// List contains the project.
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("list: status = %d", rec.Code)
	}
	if projects := decodeProjects(t, rec); len(projects) != 1 {
		t.Fatalf("list: want 1 project, got %v", projects)
	}

	// Get by id.
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID, cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("get: status = %d", rec.Code)
	}

	// Get missing -> 404.
	rec = apiRequest(h, http.MethodGet, "/api/v1/projects/prj_missing", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("get missing: status = %d, want 404", rec.Code)
	}

	// Archive -> archived true; idempotent second archive.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects/"+projectID+"/archive", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("archive: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var archived struct {
		Project struct {
			Archived bool `json:"archived"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &archived); err != nil {
		t.Fatal(err)
	}
	if !archived.Project.Archived {
		t.Fatal("archive response must set archived=true")
	}
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects/"+projectID+"/archive", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("second archive: status = %d", rec.Code)
	}

	// Archive missing -> 404.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects/prj_missing/archive", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("archive missing: status = %d, want 404", rec.Code)
	}
}

func TestTwoProjectsHaveIsolatedRepos(t *testing.T) {
	h, svc := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)

	for _, name := range []string{"proj-a", "proj-b"} {
		rec := apiRequest(h, http.MethodPost, "/api/v1/projects", cookie,
			`{"name":"`+name+`"}`)
		if rec.Code != http.StatusCreated {
			t.Fatalf("create %s: status = %d body = %s", name, rec.Code, rec.Body.String())
		}
	}

	projects, err := svc.List(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if len(projects) != 2 {
		t.Fatalf("want 2 projects, got %d", len(projects))
	}
	// Distinct bare repos, each with its own HEAD.
	heads := make(map[string]string)
	for _, p := range projects {
		headBytes, err := os.ReadFile(filepath.Join(svc.ReposRoot(), p.ID, "repo.git", "HEAD"))
		if err != nil {
			t.Fatalf("read HEAD of %s: %v", p.Name, err)
		}
		heads[p.Name] = string(headBytes)
		if heads[p.Name] != "ref: refs/heads/main\n" {
			t.Fatalf("%s HEAD = %q", p.Name, heads[p.Name])
		}
	}
	// Each repo must have exactly one commit and it must differ.
	refA := gitRevParse(t, projects[0].ID, svc.ReposRoot())
	refB := gitRevParse(t, projects[1].ID, svc.ReposRoot())
	if refA == refB {
		t.Fatal("both projects resolve to the same commit — history is not isolated")
	}
}

func gitRevParse(t *testing.T, projectID, reposRoot string) string {
	t.Helper()
	dir := filepath.Join(reposRoot, projectID, "repo.git")
	cmd := exec.CommandContext(t.Context(), "git", "--git-dir", dir, "rev-parse", "HEAD")
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("rev-parse %s: %v", projectID, err)
	}
	return strings.TrimSpace(string(out))
}

// gitHead returns the current HEAD of a project's bare repo (shortcut for
// building changeset payloads).
func gitHead(t *testing.T, projectID, reposRoot string) string {
	t.Helper()
	return gitRevParse(t, projectID, reposRoot)
}

// gitShow returns the content of a path at HEAD in a project's bare repo,
// or "" when the path does not exist.
func gitShow(t *testing.T, projectID, reposRoot, path string) string {
	t.Helper()
	dir := filepath.Join(reposRoot, projectID, "repo.git")
	cmd := exec.CommandContext(t.Context(), "git", "--git-dir", dir, "show", "HEAD:"+path)
	out, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func TestProjectRename(t *testing.T) {
	h, svc := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)

	rec := apiRequest(h, http.MethodPost, "/api/v1/projects", cookie,
		`{"name":"old-name","description":"desc"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var created struct {
		Project struct {
			ID string `json:"id"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	id := created.Project.ID

	// Rename -> new name in metadata.
	rec = apiRequest(h, http.MethodPatch, "/api/v1/projects/"+id, cookie,
		`{"name":"new-name"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("rename: status = %d body = %s", rec.Code, rec.Body.String())
	}
	p, err := svc.Get(t.Context(), id)
	if err != nil {
		t.Fatal(err)
	}
	if p.Name != "new-name" {
		t.Fatalf("name = %q, want new-name", p.Name)
	}

	// README headline updated in the repo.
	readme := gitShow(t, id, svc.ReposRoot(), "README.md")
	if !strings.Contains(readme, "# new-name") {
		t.Fatalf("readme headline not updated: %q", readme)
	}

	// Rename to an existing name -> 409.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects", cookie, `{"name":"other"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create other: status = %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodPatch, "/api/v1/projects/"+id, cookie, `{"name":"other"}`)
	if rec.Code != http.StatusConflict {
		t.Fatalf("rename conflict: status = %d, want 409", rec.Code)
	}
}

func TestProjectDelete(t *testing.T) {
	h, svc := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)

	rec := apiRequest(h, http.MethodPost, "/api/v1/projects", cookie, `{"name":"to-delete"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create: status = %d", rec.Code)
	}
	var created struct {
		Project struct {
			ID string `json:"id"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	id := created.Project.ID
	repoDir := filepath.Join(svc.ReposRoot(), id)
	if _, err := os.Stat(repoDir); err != nil {
		t.Fatalf("repo dir missing before delete: %v", err)
	}

	rec = apiRequest(h, http.MethodDelete, "/api/v1/projects/"+id, cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("delete: status = %d body = %s", rec.Code, rec.Body.String())
	}
	if _, err := svc.Get(t.Context(), id); err == nil {
		t.Fatal("project still present after delete")
	}
	if _, err := os.Stat(repoDir); !os.IsNotExist(err) {
		t.Fatalf("repo dir still present after delete: %v", err)
	}

	// Delete missing -> 404.
	rec = apiRequest(h, http.MethodDelete, "/api/v1/projects/"+id, cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("delete missing: status = %d, want 404", rec.Code)
	}
}

func TestProjectPurge(t *testing.T) {
	h, svc := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)

	rec := apiRequest(h, http.MethodPost, "/api/v1/projects", cookie, `{"name":"purge-me"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create: status = %d", rec.Code)
	}
	var created struct {
		Project struct {
			ID string `json:"id"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	id := created.Project.ID

	// Commit a secret doc.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects/"+id+"/changesets", cookie,
		`{"base_revision":"`+gitHead(t, id, svc.ReposRoot())+`","message":"add secret","changes":[{"op":"create","path":"docs/secret.md","content":"top-secret"}]}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("add secret: status = %d body = %s", rec.Code, rec.Body.String())
	}
	if out := gitShow(t, id, svc.ReposRoot(), "docs/secret.md"); !strings.Contains(out, "top-secret") {
		t.Fatalf("secret doc not committed: %q", out)
	}

	// Purge the path from history.
	rec = apiRequest(h, http.MethodPost, "/api/v1/projects/"+id+"/purge", cookie,
		`{"paths":["docs/secret.md"],"message":"remove secret"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("purge: status = %d body = %s", rec.Code, rec.Body.String())
	}
	if out := gitShow(t, id, svc.ReposRoot(), "docs/secret.md"); out != "" {
		t.Fatalf("secret doc still in history: %q", out)
	}
}
