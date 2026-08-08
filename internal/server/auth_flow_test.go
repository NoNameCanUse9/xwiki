package server

import (
	"bytes"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"regexp"
	"strings"
	"testing"
	"time"

	"agentdocs/internal/agent"
	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/project"
	"agentdocs/internal/search"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

// newTestRouterWithLog builds the router exactly like newTestRouter but
// captures the server log so tests can read the out-of-band reset token.
func newTestRouterWithLog(t *testing.T) (http.Handler, *bytes.Buffer) {
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
	var logBuf bytes.Buffer
	log := slog.New(slog.NewTextHandler(&logBuf, nil))

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
	return NewRouter(cfg, log, db, users, authSvc, projectsSvc,
		agent.NewService(db, clock.Real{}), search.NewService(db, projectsSvc)), &logBuf
}

// resetTokenFromLog extracts the reset_token value from the captured log.
var resetTokenRe = regexp.MustCompile(`reset_token=(\S+)`)

// TestForgotPasswordDoesNotLeakAccounts verifies the endpoint returns the
// same response for existing and unknown usernames.
func TestForgotPasswordDoesNotLeakAccounts(t *testing.T) {
	h, _ := newTestRouterWithLog(t)
	rec := apiRequest(h, http.MethodPost, "/api/v1/auth/forgot-password", "",
		`{"username":"admin"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("forgot-password (known user): status = %d body = %s", rec.Code, rec.Body.String())
	}
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/forgot-password", "",
		`{"username":"no_such_user"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("forgot-password (unknown user): status = %d body = %s", rec.Code, rec.Body.String())
	}
	var body map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if _, ok := body["ok"]; !ok {
		t.Fatalf("response missing ok: %v", body)
	}
}

// TestResetPasswordFlow covers the full forgot → reset → old sessions die →
// new password works cycle end to end.
func TestResetPasswordFlow(t *testing.T) {
	h, logBuf := newTestRouterWithLog(t)

	// Existing session that must be killed by the reset.
	oldCookie := loginAndGetCookie(t, h)
	if oldCookie == "" {
		t.Fatal("no session cookie")
	}

	// Mint a reset token for the seeded admin; the handler logs it.
	rec := apiRequest(h, http.MethodPost, "/api/v1/auth/forgot-password", "",
		`{"username":"admin"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("forgot-password: %d %s", rec.Code, rec.Body.String())
	}
	m := resetTokenRe.FindStringSubmatch(logBuf.String())
	if m == nil {
		t.Fatalf("reset token not found in log: %s", logBuf.String())
	}
	resetToken := m[1]

	// Reset with a short password must be rejected.
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/reset-password", "",
		`{"token":"`+resetToken+`","new_password":"short"}`)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("short password: status = %d", rec.Code)
	}

	// Valid reset.
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/reset-password", "",
		`{"token":"`+resetToken+`","new_password":"newsecret123"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("reset-password: %d %s", rec.Code, rec.Body.String())
	}

	// Old session must no longer resolve.
	rec = apiRequest(h, http.MethodGet, "/api/v1/auth/me", oldCookie, "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("old session after reset: status = %d, want 401", rec.Code)
	}

	// Old password rejected, new password works.
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		`{"username":"admin","password":"secret123"}`)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("old password login: status = %d, want 401", rec.Code)
	}
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		`{"username":"admin","password":"newsecret123"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("new password login: %d %s", rec.Code, rec.Body.String())
	}

	// The consumed token must not be replayable.
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/reset-password", "",
		`{"token":"`+resetToken+`","new_password":"anotherpass1"}`)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("token replay: status = %d, want 400", rec.Code)
	}
}

var _ = io.Discard // keep io import honest if unused in future edits
var _ = strings.TrimSpace
