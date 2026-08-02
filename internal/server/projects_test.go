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
	}}

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
