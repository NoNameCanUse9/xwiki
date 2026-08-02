package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// seedLocalRepo creates a local git repository with two commits for clone tests.
func seedLocalRepo(t *testing.T) string {
	t.Helper()
	dir := t.TempDir() + "/src"
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	run := func(args ...string) string {
		cmd := exec.CommandContext(t.Context(), "git", args...)
		cmd.Dir = dir
		cmd.Env = append(os.Environ(), gitIdentityEnv()...)
		out, err := cmd.CombinedOutput()
		if err != nil {
			t.Fatalf("git %v: %v (%s)", args, err, out)
		}
		return string(out)
	}
	run("init", "-b", "main")
	if err := os.WriteFile(filepath.Join(dir, "README.md"), []byte("# imported\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	run("add", "-A")
	run("commit", "-m", "first")
	if err := os.MkdirAll(filepath.Join(dir, "docs"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "docs", "guide.md"), []byte("# Guide\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	run("add", "-A")
	run("commit", "-m", "second")
	return dir
}

func TestImportRepoFromURL(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	src := seedLocalRepo(t)

	rec := apiRequest(h, http.MethodPost,
		"/api/v1/import/repo?name=imported-site&url=file://"+src, cookie, "")
	if rec.Code != http.StatusCreated {
		t.Fatalf("import repo: %d %s", rec.Code, rec.Body.String())
	}
	var created struct {
		Project struct {
			ID   string `json:"id"`
			Name string `json:"name"`
		} `json:"project"`
		Commits int `json:"commits"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if created.Project.Name != "imported-site" || created.Commits != 2 {
		t.Fatalf("imported wrong: %+v", created)
	}
	// History preserved and readable.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+created.Project.ID+"/commits", cookie, "")
	var list struct {
		Commits []struct {
			Message string `json:"message"`
		} `json:"commits"`
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &list)
	if len(list.Commits) != 2 || list.Commits[0].Message != "second" {
		t.Fatalf("history wrong: %+v", list.Commits)
	}
	// File readable.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+created.Project.ID+"/docs/pages/docs/guide.md", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("guide: %d", rec.Code)
	}

	// Invalid URL -> 400.
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/import/repo?name=bad&url=javascript:alert(1)", cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("bad url: %d", rec.Code)
	}
	// Unauthenticated -> 401.
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/import/repo?name=x&url=file://"+src, "", "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("unauth: %d", rec.Code)
	}
	_ = fmt.Sprintf
}
