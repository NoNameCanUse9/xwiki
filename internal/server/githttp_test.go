package server

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// gitRun runs a git command in dir with the given env, returning stdout.
func gitRun(t *testing.T, dir string, env []string, args ...string) string {
	t.Helper()
	cmd := exec.CommandContext(t.Context(), "git", args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), env...)
	var out, errBuf bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		t.Fatalf("git %v: %v (%s)", args, err, errBuf.String())
	}
	return strings.TrimSpace(out.String())
}

func TestGitHTTPCloneAndPush(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "git-http-site")

	// Create a write token for the project.
	secret := createAgentToken(t, h, cookie,
		fmt.Sprintf(`{"name":"git-bot","scope":"write","project_ids":["%s"]}`, projectID))

	// Clone via the smart HTTP endpoint.
	url := fmt.Sprintf("http://x:%s@127.0.0.1/git/%s", secret, projectID)
	cloneDir := t.TempDir() + "/clone"

	// Serve the handler directly with a real HTTP server on a local port.
	srv := httptest.NewServer(h)
	defer srv.Close()
	cloneURL := strings.Replace(url, "127.0.0.1", strings.TrimPrefix(srv.URL, "http://"), 1)
	gitRun(t, t.TempDir(), nil, "clone", cloneURL, cloneDir)
	if _, err := os.Stat(filepath.Join(cloneDir, "README.md")); err != nil {
		t.Fatalf("clone missing README: %v", err)
	}

	// Modify + commit + push.
	if err := os.WriteFile(filepath.Join(cloneDir, "README.md"), []byte("# git-http-site\n\npushed via git client\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	gitRun(t, cloneDir, nil, "add", "-A")
	gitRun(t, cloneDir, gitIdentityEnv(), "commit", "-m", "push via git client")
	gitRun(t, cloneDir, nil, "push", "origin", "main")

	// The API sees the pushed commit immediately.
	rec := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/commits", cookie, "")
	var list struct {
		Commits []struct {
			Message string `json:"message"`
		} `json:"commits"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	if len(list.Commits) < 2 || list.Commits[0].Message != "push via git client" {
		t.Fatalf("pushed commit missing from history: %+v", list.Commits)
	}
}

func TestGitHTTPAuthMatrix(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "git-auth-site")
	readSecret := createAgentToken(t, h, cookie,
		fmt.Sprintf(`{"name":"reader","scope":"read","project_ids":["%s"]}`, projectID))
	writeSecret := createAgentToken(t, h, cookie,
		fmt.Sprintf(`{"name":"writer","scope":"write","project_ids":["%s"]}`, projectID))

	req := func(path, secret string) *httptest.ResponseRecorder {
		httpreq := httptest.NewRequest(http.MethodGet, path, nil)
		if secret != "" {
			httpreq.Header.Set("Authorization", "Basic "+base64Encode("x:"+secret))
		}
		rec := httptest.NewRecorder()
		h.ServeHTTP(rec, httpreq)
		return rec
	}

	// No credentials -> 401.
	if rec := req("/git/"+projectID+"/info/refs?service=git-upload-pack", ""); rec.Code != http.StatusUnauthorized {
		t.Fatalf("no auth: %d", rec.Code)
	}
	// Read token -> upload-pack allowed (http-backend runs; expect its 200 or project listing).
	if rec := req("/git/"+projectID+"/info/refs?service=git-upload-pack", readSecret); rec.Code == http.StatusUnauthorized || rec.Code == http.StatusForbidden {
		t.Fatalf("read token upload-pack: %d", rec.Code)
	}
	// Read token -> receive-pack forbidden.
	if rec := req("/git/"+projectID+"/info/refs?service=git-receive-pack", readSecret); rec.Code != http.StatusForbidden {
		t.Fatalf("read token receive-pack: %d, want 403", rec.Code)
	}
	// Write token -> receive-pack allowed.
	if rec := req("/git/"+projectID+"/info/refs?service=git-receive-pack", writeSecret); rec.Code == http.StatusUnauthorized || rec.Code == http.StatusForbidden {
		t.Fatalf("write token receive-pack: %d", rec.Code)
	}
	// Unknown project -> 404.
	if rec := req("/git/prj_missing/info/refs?service=git-upload-pack", writeSecret); rec.Code != http.StatusNotFound {
		t.Fatalf("unknown project: %d", rec.Code)
	}
}

func gitIdentityEnv() []string {
	return []string{
		"GIT_AUTHOR_NAME=Test", "GIT_AUTHOR_EMAIL=test@xwiki.local",
		"GIT_COMMITTER_NAME=Test", "GIT_COMMITTER_EMAIL=test@xwiki.local",
		"GIT_CONFIG_NOSYSTEM=1",
	}
}

func base64Encode(s string) string {
	return base64.StdEncoding.EncodeToString([]byte(s))
}
