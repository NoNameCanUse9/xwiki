package server

import (
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

func newTestRouter(t *testing.T) http.Handler {
	t.Helper()
	cfg := config.Load()
	cfg.DataDir = t.TempDir()
	db, err := sqlite.Open(cfg.DataDir)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	users := user.NewStore(db)
	authSvc := auth.NewService(db, clock.Real{}, 24*time.Hour)
	log := slog.New(slog.NewTextHandler(io.Discard, nil))
	return NewRouter(cfg, log, db, users, authSvc)
}

func TestHealthAndReady(t *testing.T) {
	h := newTestRouter(t)
	for _, path := range []string{"/healthz", "/readyz"} {
		rec := httptest.NewRecorder()
		h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, path, nil))
		if rec.Code != http.StatusOK {
			t.Fatalf("%s: status = %d", path, rec.Code)
		}
	}
}

func TestUnknownAPIPathReturnsJSON404(t *testing.T) {
	h := newTestRouter(t)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/nope", nil))
	if rec.Code != http.StatusNotFound {
		t.Fatalf("status = %d, want 404", rec.Code)
	}
	var body struct {
		Error struct {
			Code string `json:"code"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "not_found" {
		t.Fatalf("code = %q", body.Error.Code)
	}
}

func TestSPAServesPlaceholderAndFallsBack(t *testing.T) {
	h := newTestRouter(t)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "AgentDocs placeholder") {
		t.Fatalf("root: status=%d body=%q", rec.Code, rec.Body.String())
	}
	rec = httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/login", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "AgentDocs placeholder") {
		t.Fatalf("spa fallback: status=%d body=%q", rec.Code, rec.Body.String())
	}
}
