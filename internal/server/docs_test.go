package server

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"strings"
	"testing"
)

// appendDocToRepo commits a new markdown file to a project's bare repo using
// plumbing commands (no worktree), preserving existing root entries.
func appendDocToRepo(t *testing.T, repoDir, path, content, branch string) {
	t.Helper()
	env := append(os.Environ(),
		"GIT_AUTHOR_NAME=AgentDocs", "GIT_AUTHOR_EMAIL=agentdocs@local",
		"GIT_COMMITTER_NAME=AgentDocs", "GIT_COMMITTER_EMAIL=agentdocs@local",
		"GIT_CONFIG_NOSYSTEM=1",
	)
	runGit := func(args ...string) string {
		t.Helper()
		cmd := exec.CommandContext(t.Context(), "git", append([]string{"--git-dir", repoDir}, args...)...)
		cmd.Env = env
		var out, errBuf bytes.Buffer
		cmd.Stdout = &out
		cmd.Stderr = &errBuf
		if err := cmd.Run(); err != nil {
			t.Fatalf("git %v: %v (%s)", args, err, errBuf.String())
		}
		return strings.TrimSpace(out.String())
	}

	blobFile := t.TempDir() + "/blob.md"
	if err := os.WriteFile(blobFile, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	blobSHA := runGit("hash-object", "-w", blobFile)

	dir, file := splitPath(path)
	dirTree := mktreeFromInput(t, repoDir, env, fmt.Sprintf("100644 blob %s\t%s\n", blobSHA, file))
	combined := runGit("ls-tree", branch)
	if combined != "" {
		combined += "\n"
	}
	combined += fmt.Sprintf("040000 tree %s\t%s\n", dirTree, dir)
	newTree := mktreeFromInput(t, repoDir, env, combined)
	parent := runGit("rev-parse", "HEAD")
	commit := runGit("commit-tree", newTree, "-p", parent, "-m", "add "+path)
	runGit("update-ref", "refs/heads/"+branch, commit)
}

func splitPath(p string) (dir, file string) {
	if i := strings.LastIndex(p, "/"); i >= 0 {
		return p[:i], p[i+1:]
	}
	return "", p
}

func mktreeFromInput(t *testing.T, repoDir string, env []string, input string) string {
	t.Helper()
	treeFile := t.TempDir() + "/tree.txt"
	if err := os.WriteFile(treeFile, []byte(input), 0o600); err != nil {
		t.Fatal(err)
	}
	f, err := os.Open(treeFile)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	cmd := exec.CommandContext(t.Context(), "git", "--git-dir", repoDir, "mktree")
	cmd.Env = env
	cmd.Stdin = f
	var treeOut, treeErr bytes.Buffer
	cmd.Stdout = &treeOut
	cmd.Stderr = &treeErr
	if err := cmd.Run(); err != nil {
		t.Fatalf("mktree: %v (%s)", err, treeErr.String())
	}
	return strings.TrimSpace(treeOut.String())
}

func createProjectViaAPI(t *testing.T, h http.Handler, cookie, name string) (id, repoDir string) {
	t.Helper()
	rec := apiRequest(h, http.MethodPost, "/api/v1/projects", cookie,
		fmt.Sprintf(`{"name":"%s"}`, name))
	if rec.Code != http.StatusCreated {
		t.Fatalf("create %s: status = %d body = %s", name, rec.Code, rec.Body.String())
	}
	var created struct {
		Project struct {
			ID      string `json:"id"`
			RepoDir string `json:"repo_dir"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	return created.Project.ID, created.Project.RepoDir
}

func TestDocsTreePagesHome(t *testing.T) {
	h, svc := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "docs-site")

	// Append docs/guide.md straight into the repo (no index, no DB).
	repo, err := svc.OpenRepo(t.Context(), projectID)
	if err != nil {
		t.Fatal(err)
	}
	appendDocToRepo(t, repo.Dir, "docs/guide.md", "# Guide\n\nhello *world*\n", "main")

	// Tree listing (root + docs/).
	rec := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/tree?path=", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("tree: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var treeResp struct {
		Tree []map[string]string `json:"tree"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &treeResp); err != nil {
		t.Fatal(err)
	}
	names := map[string]string{}
	for _, e := range treeResp.Tree {
		names[e["name"]] = e["type"]
	}
	if names["README.md"] != "blob" || names["docs"] != "tree" {
		t.Fatalf("root tree wrong: %v", names)
	}

	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/tree?path=docs", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("docs tree: status = %d", rec.Code)
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &treeResp)
	if len(treeResp.Tree) != 1 || treeResp.Tree[0]["name"] != "guide.md" {
		t.Fatalf("docs tree wrong: %v", treeResp.Tree)
	}

	// Raw page read.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/guide.md", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("page raw: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var page struct {
		Path   string `json:"path"`
		Format string `json:"format"`
		Raw    string `json:"content"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &page); err != nil {
		t.Fatal(err)
	}
	if page.Path != "docs/guide.md" || page.Format != "raw" || !strings.Contains(page.Raw, "# Guide") {
		t.Fatalf("raw page wrong: %+v", page)
	}

	// HTML render.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/guide.md?format=html", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("page html: status = %d", rec.Code)
	}
	var htmlPage struct {
		Format  string `json:"format"`
		Content string `json:"content"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &htmlPage); err != nil {
		t.Fatal(err)
	}
	if htmlPage.Format != "html" || !strings.Contains(htmlPage.Content, "<h1>Guide</h1>") ||
		!strings.Contains(htmlPage.Content, "<em>world</em>") {
		t.Fatalf("html page wrong: %+v", htmlPage.Content)
	}

	// Home = README.md.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/home", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("home: status = %d", rec.Code)
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &htmlPage); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(htmlPage.Content, "docs-site") {
		t.Fatalf("home content wrong: %s", htmlPage.Content)
	}

	// Errors: traversal -> 400, missing -> 404, unauthenticated -> 401.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/../README.md", cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("traversal: status = %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/missing.md", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("missing: status = %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/tree?path=", "", "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("unauth: status = %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/prj_missing/docs/home", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("missing project: status = %d", rec.Code)
	}
}

func TestWikiLinksRenderAsProjectLinks(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "wiki-site")
	base := getRevision(t, h, cookie, projectID)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"wikilinks",
		  "changes":[{"op":"create","path":"docs/links.md",
		    "content":"见 [[docs/guide.md|指南]] 与 [[docs/plain.md]]\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("write: %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/links.md?format=html", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("page: %d", rec.Code)
	}
	var page struct {
		Content string `json:"content"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &page); err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(page.Content, `href="/projects/`+projectID+`/docs/docs/guide.md"`) {
		t.Fatalf("guide link missing: %s", page.Content)
	}
	if !strings.Contains(page.Content, ">指南</a>") {
		t.Fatalf("label missing: %s", page.Content)
	}
	if !strings.Contains(page.Content, `href="/projects/`+projectID+`/docs/docs/plain.md"`) {
		t.Fatalf("plain link missing: %s", page.Content)
	}
}

func TestDocsViewServeRenderedPage(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "view-site")

	// Seed a markdown file.
	rev := getRevision(t, h, cookie, projectID)
	submitChangeset(t, h, cookie, projectID,
		`{"base_revision":"`+rev+`","message":"seed",
		  "changes":[{"op":"create","path":"guide.md","content":"# Guide\n\nHello **world**.\n"}]}`)

	path := "/projects/" + projectID + "/docs/guide.md"

	// Non-browser client (agent/curl) gets the server-rendered page.
	req := httptest.NewRequest(http.MethodGet, path, nil)
	req.Header.Set("User-Agent", "ClaudeBot/1.0")
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("agent view: status = %d body = %s", rec.Code, rec.Body.String())
	}
	if !strings.Contains(rec.Body.String(), "<h1>Guide</h1>") ||
		!strings.Contains(rec.Body.String(), "<strong>world</strong>") {
		t.Fatalf("agent view missing rendered content: %s", rec.Body.String())
	}
	if ct := rec.Header().Get("Content-Type"); !strings.HasPrefix(ct, "text/html") {
		t.Fatalf("agent view content-type = %s", ct)
	}

	// Browser gets the SPA shell.
	req = httptest.NewRequest(http.MethodGet, path, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) Chrome/120")
	rec = httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("browser view: status = %d", rec.Code)
	}
	if strings.Contains(rec.Body.String(), "<h1>Guide</h1>") {
		t.Fatalf("browser should get the SPA shell, got rendered page")
	}
	if !strings.Contains(rec.Body.String(), "<!doctype html>") {
		t.Fatalf("browser should get an html document")
	}

	// Unknown doc -> 404.
	req = httptest.NewRequest(http.MethodGet, "/projects/"+projectID+"/docs/missing.md", nil)
	req.Header.Set("User-Agent", "ClaudeBot/1.0")
	rec = httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusNotFound {
		t.Fatalf("missing doc: status = %d", rec.Code)
	}
}
