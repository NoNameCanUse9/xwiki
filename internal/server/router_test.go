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

	"xwiki/internal/agent"
	"xwiki/internal/auth"
	"xwiki/internal/config"
	"xwiki/internal/platform/clock"
	"xwiki/internal/project"
	"xwiki/internal/search"
	"xwiki/internal/store/sqlite"
	"xwiki/internal/user"
)

func newTestRouter(t *testing.T) http.Handler {
	h, _ := newTestRouterWithService(t)
	return h
}

func newTestRouterWithService(t *testing.T) (http.Handler, *project.Service) {
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
	projectsSvc := project.NewService(db, cfg.DataDir, clock.Real{})
	log := slog.New(slog.NewTextHandler(io.Discard, nil))

	// Seed the admin user used by loginAndGetCookie.
	now := time.Now().UTC()
	hash, err := auth.HashPassword("secret123")
	if err != nil {
		t.Fatal(err)
	}
	if err := users.Create(t.Context(), &user.User{
		ID: "usr_admin", Username: "admin", DisplayName: "Admin",
		PasswordHash: hash, IsAdmin: true, CreatedAt: now, UpdatedAt: now,
	}); err != nil {
		t.Fatal(err)
	}
	return NewRouter(cfg, log, db, users, authSvc, projectsSvc, agent.NewService(db, clock.Real{}), search.NewService(db, projectsSvc)), projectsSvc
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
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "<div id=\"root\"></div>") {
		t.Fatalf("root: status=%d body=%q", rec.Code, rec.Body.String())
	}
	rec = httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/login", nil))
	if rec.Code != http.StatusOK || !strings.Contains(rec.Body.String(), "<div id=\"root\"></div>") {
		t.Fatalf("spa fallback: status=%d body=%q", rec.Code, rec.Body.String())
	}
}
