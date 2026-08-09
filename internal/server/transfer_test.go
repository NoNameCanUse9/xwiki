package server

import (
	"archive/zip"
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

func TestOpenAPIRoute(t *testing.T) {
	h := newTestRouter(t)
	rec := apiRequest(h, http.MethodGet, "/api/openapi.json", "", "")
	if rec.Code != http.StatusOK {
		t.Fatalf("openapi: %d", rec.Code)
	}
	var spec map[string]any
	if err := yaml.Unmarshal(rec.Body.Bytes(), &spec); err != nil {
		t.Fatal(err)
	}
	if spec["openapi"] != "3.0.3" {
		t.Fatalf("version: %v", spec["openapi"])
	}
}

func TestZipExportImportRoundTrip(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "transfer-site")
	base := getRevision(t, h, cookie, projectID)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"seed",
		  "changes":[{"op":"create","path":"docs/a.md","content":"# A\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("seed: %d", rec.Code)
	}

	// Export zip.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/export.zip", cookie, "")
	if rec.Code != http.StatusOK || rec.Header().Get("Content-Type") != "application/zip" {
		t.Fatalf("export zip: %d %s", rec.Code, rec.Header().Get("Content-Type"))
	}
	zipBytes := rec.Body.Bytes()
	zr, err := zip.NewReader(bytes.NewReader(zipBytes), int64(len(zipBytes)))
	if err != nil {
		t.Fatal(err)
	}
	names := map[string]bool{}
	for _, f := range zr.File {
		names[f.Name] = true
	}
	if !names["docs/a.md"] || !names["README.md"] {
		t.Fatalf("zip missing entries: %v", names)
	}

	// Import into a second project.
	projectID2, _ := createProjectViaAPI(t, h, cookie, "import-target")
	base2 := getRevision(t, h, cookie, projectID2)
	files := []map[string]string{}
	for _, f := range zr.File {
		rc, err := f.Open()
		if err != nil {
			t.Fatal(err)
		}
		var buf bytes.Buffer
		_, _ = buf.ReadFrom(rc)
		_ = rc.Close()
		files = append(files, map[string]string{
			"path": f.Name, "content": base64.StdEncoding.EncodeToString(buf.Bytes()),
		})
	}
	payload, _ := json.Marshal(map[string]any{
		"base_revision": base2, "message": "import", "files": files,
	})
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID2+"/import", cookie, string(payload))
	if rec.Code != http.StatusOK {
		t.Fatalf("import: %d %s", rec.Code, rec.Body.String())
	}
	var imported struct {
		Imported int `json:"imported"`
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &imported)
	if imported.Imported < 2 {
		t.Fatalf("imported %d", imported.Imported)
	}
	// Content matches.
	page := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID2+"/docs/pages/docs/a.md", cookie, "")
	if !strings.Contains(page.Body.String(), "# A") {
		t.Fatalf("imported content wrong: %s", page.Body.String())
	}
}

func TestBundleExportImport(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "bundle-src")
	base := getRevision(t, h, cookie, projectID)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"seed",
		  "changes":[{"op":"create","path":"docs/b.md","content":"# B\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("seed: %d", rec.Code)
	}

	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/export.bundle", cookie, "")
	if rec.Code != http.StatusOK || len(rec.Body.Bytes()) == 0 {
		t.Fatalf("export bundle: %d", rec.Code)
	}
	bundle := rec.Body.Bytes()

	// Import as a new project via multipart.
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	fw, err := mw.CreateFormFile("file", "repo.bundle")
	if err != nil {
		t.Fatal(err)
	}
	_, _ = fw.Write(bundle)
	_ = mw.Close()
	req := httptest.NewRequest(http.MethodPost,
		"/api/v1/import/bundle?name=bundle-in", &body)
	req.Header.Set("Content-Type", mw.FormDataContentType())
	req.AddCookie(&http.Cookie{Name: "agentdocs_session", Value: cookie})
	rec2 := httptest.NewRecorder()
	h.ServeHTTP(rec2, req)
	if rec2.Code != http.StatusCreated {
		t.Fatalf("bundle import: %d %s", rec2.Code, rec2.Body.String())
	}
	var created struct {
		Project struct {
			ID   string `json:"id"`
			Name string `json:"name"`
		} `json:"project"`
		Commits int `json:"commits"`
	}
	if err := json.Unmarshal(rec2.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if created.Project.Name != "bundle-in" || created.Commits < 2 {
		t.Fatalf("bundle import wrong: %+v", created)
	}
	// History preserved: commits list shows 2+ entries.
	rec3 := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+created.Project.ID+"/commits", cookie, "")
	var list struct {
		Commits []map[string]any `json:"commits"`
	}
	_ = json.Unmarshal(rec3.Body.Bytes(), &list)
	if len(list.Commits) < 2 {
		t.Fatalf("bundle history lost: %d", len(list.Commits))
	}
}

func TestPageBase64Format(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "bin-site")
	base := getRevision(t, h, cookie, projectID)
	// Write a binary file via base64 encoding.
	b64 := base64.StdEncoding.EncodeToString([]byte{0x00, 0x01, 0xFE})
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"binary",
		  "changes":[{"op":"create","path":"docs/img.bin","content":"%s","encoding":"base64"}]}`, base, b64))
	if rec.Code != http.StatusOK {
		t.Fatalf("binary write: %d %s", rec.Code, rec.Body.String())
	}
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/pages/docs/img.bin?format=base64", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("base64 read: %d", rec.Code)
	}
	var page struct {
		Format   string `json:"format"`
		Encoding string `json:"encoding"`
		Content  string `json:"content"`
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &page)
	if page.Format != "base64" || page.Encoding != "base64" || page.Content != b64 {
		t.Fatalf("base64 page wrong: %+v", page)
	}
}
