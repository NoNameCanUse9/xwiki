package middleware

import (
	"context"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"agentdocs/internal/auth"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/platform/clock"
	"agentdocs/internal/store/sqlite"
	"agentdocs/internal/user"
)

func newAuthService(t *testing.T) *auth.Service {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return auth.NewService(db, clock.Real{}, 24*time.Hour)
}

func discardLog() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

func TestRequestIDMiddleware(t *testing.T) {
	var got string
	h := RequestID(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		got = request.RequestID(r)
		w.WriteHeader(http.StatusOK)
	}))
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if got == "" {
		t.Fatal("request id not set")
	}
	if rec.Header().Get("X-Request-ID") != got {
		t.Fatal("request id header mismatch")
	}
}

func TestSessionAuthRejectsMissingOrInvalidCookie(t *testing.T) {
	h := SessionAuth(newAuthService(t))(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("no cookie: status = %d, want 401", rec.Code)
	}

	rec = httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.AddCookie(&http.Cookie{Name: "agentdocs_session", Value: "garbage"})
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("bad cookie: status = %d, want 401", rec.Code)
	}

	var body struct {
		Error struct {
			Code string `json:"code"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Error.Code != "authentication_required" {
		t.Fatalf("code = %q", body.Error.Code)
	}
}

func TestSessionAuthAcceptsValidSession(t *testing.T) {
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	users := user.NewStore(db)
	now := time.Now().UTC()
	if err := users.Create(context.Background(), &user.User{
		ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "x", IsAdmin: true, CreatedAt: now, UpdatedAt: now,
	}); err != nil {
		t.Fatal(err)
	}
	svc := auth.NewService(db, clock.Real{}, 24*time.Hour)
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}

	h := SessionAuth(svc)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		u := UserFrom(r)
		if u == nil || u.Username != "admin" {
			t.Errorf("user not in context: %+v", u)
		}
		w.WriteHeader(http.StatusOK)
	}))
	rec := httptest.NewRecorder()
	req := httptest.NewRequest(http.MethodGet, "/", nil)
	req.AddCookie(&http.Cookie{Name: "agentdocs_session", Value: token})
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rec.Code)
	}
}

func TestRecoverer(t *testing.T) {
	h := Recoverer(discardLog())(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		panic("boom")
	}))
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/", nil))
	if rec.Code != http.StatusInternalServerError {
		t.Fatalf("status = %d, want 500", rec.Code)
	}
}
