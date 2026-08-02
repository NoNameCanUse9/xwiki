package server

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"testing"
)

// png1x1 is a minimal 1x1 PNG blob used to verify raw byte fidelity.
var png1x1 = []byte{
	0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
	0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
	0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
	0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
	0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41,
	0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
	0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
	0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
	0x42, 0x60, 0x82,
}

func TestAttachmentDownload(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "attach-site")
	base := getRevision(t, h, cookie, projectID)

	// Write a binary attachment via base64 encoding.
	b64 := base64.StdEncoding.EncodeToString(png1x1)
	rec := submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"add logo",
		  "changes":[{"op":"create","path":"attachments/logo.png","content":"%s","encoding":"base64"}]}`,
			base, b64))
	if rec.Code != http.StatusOK {
		t.Fatalf("write attachment: status = %d body = %s", rec.Code, rec.Body.String())
	}

	// Download returns the raw bytes with an extension-derived content type.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/attachments/attachments/logo.png", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("download: status = %d body = %s", rec.Code, rec.Body.String())
	}
	if ct := rec.Header().Get("Content-Type"); !strings.HasPrefix(ct, "image/png") {
		t.Fatalf("content-type = %q, want image/png", ct)
	}
	if cd := rec.Header().Get("Content-Disposition"); !strings.Contains(cd, "inline") {
		t.Fatalf("content-disposition = %q, want inline", cd)
	}
	if !bytes.Equal(rec.Body.Bytes(), png1x1) {
		t.Fatalf("body mismatch: got %d bytes, want %d", len(rec.Body.Bytes()), len(png1x1))
	}

	// Missing attachment -> 404 with attachment_not_found.
	rec = apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/attachments/attachments/missing.png", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("missing: status = %d body = %s", rec.Code, rec.Body.String())
	}
	var body struct {
		Error struct {
			Code string `json:"code"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "attachment_not_found" {
		t.Fatalf("missing code = %q, want attachment_not_found", body.Error.Code)
	}
}

func TestAttachmentDownloadRejectsTraversal(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "attach-traverse")
	rec := apiRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/attachments/../README.md", cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("traversal: status = %d body = %s", rec.Code, rec.Body.String())
	}
}

func TestAttachmentDownloadRequiresAuth(t *testing.T) {
	h := newTestRouter(t)
	rec := apiRequest(h, http.MethodGet,
		"/api/v1/projects/prj_missing/attachments/attachments/logo.png", "", "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("unauth: status = %d body = %s", rec.Code, rec.Body.String())
	}
}
