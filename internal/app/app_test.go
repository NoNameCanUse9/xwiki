package app

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"agentdocs/internal/config"
)

func newTestApp(t *testing.T) *App {
	t.Helper()
	cfg := config.Load()
	cfg.DataDir = t.TempDir()
	a, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { a.Close() })
	return a
}

func createAdmin(t *testing.T, a *App) {
	t.Helper()
	if err := a.CreateAdmin(context.Background(), "admin", "secret123"); err != nil {
		t.Fatal(err)
	}
}

func doLogin(t *testing.T, h http.Handler, username, password string) (int, []*http.Cookie, string) {
	t.Helper()
	body := strings.NewReader(`{"username":"` + username + `","password":"` + password + `"}`)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/login", body)
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	return rec.Code, rec.Result().Cookies(), rec.Body.String()
}

func TestCreateAdmin(t *testing.T) {
	a := newTestApp(t)
	if err := a.CreateAdmin(context.Background(), "admin", "secret123"); err != nil {
		t.Fatalf("CreateAdmin: %v", err)
	}
	if err := a.CreateAdmin(context.Background(), "admin", "secret123"); err == nil {
		t.Fatal("duplicate admin allowed")
	}
	if err := a.CreateAdmin(context.Background(), "ab", "secret123"); err == nil {
		t.Fatal("short username allowed")
	}
	if err := a.CreateAdmin(context.Background(), "admin2", "short"); err == nil {
		t.Fatal("short password allowed")
	}
}

func TestLoginWrongPasswordReturns401(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	code, _, body := doLogin(t, a.Handler(), "admin", "wrong")
	if code != http.StatusUnauthorized {
		t.Fatalf("status = %d, body=%s", code, body)
	}
}

func TestLoginSuccessAndMe(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	code, cookies, body := doLogin(t, a.Handler(), "admin", "secret123")
	if code != http.StatusOK {
		t.Fatalf("login status = %d, body=%s", code, body)
	}
	if len(cookies) == 0 {
		t.Fatal("no session cookie")
	}
	for _, c := range cookies {
		if c.Name == "agentdocs_session" {
			if !c.HttpOnly {
				t.Fatal("session cookie not HttpOnly")
			}
		}
	}
	req := httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec := httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("me status = %d, body=%s", rec.Code, rec.Body.String())
	}
	var resp struct {
		User struct {
			Username string `json:"username"`
			IsAdmin  bool   `json:"is_admin"`
		} `json:"user"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatal(err)
	}
	if resp.User.Username != "admin" || !resp.User.IsAdmin {
		t.Fatalf("unexpected me body: %s", rec.Body.String())
	}
}

func TestMeRequiresSession(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	rec := httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil))
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("status = %d, want 401", rec.Code)
	}
}

func TestLogout(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	_, cookies, _ := doLogin(t, a.Handler(), "admin", "secret123")

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/logout", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec := httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("logout status = %d", rec.Code)
	}

	req = httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec = httptest.NewRecorder()
	a.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("me after logout status = %d, want 401", rec.Code)
	}
}

func TestChangePassword(t *testing.T) {
	a := newTestApp(t)
	createAdmin(t, a)
	_, cookies, _ := doLogin(t, a.Handler(), "admin", "secret123")

	post := func(payload string) *httptest.ResponseRecorder {
		req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/password",
			strings.NewReader(payload))
		req.Header.Set("Content-Type", "application/json")
		for _, c := range cookies {
			req.AddCookie(c)
		}
		rec := httptest.NewRecorder()
		a.Handler().ServeHTTP(rec, req)
		return rec
	}

	if rec := post(`{"current_password":"wrong","new_password":"newsecret456"}`); rec.Code != http.StatusUnauthorized {
		t.Fatalf("wrong current password: status = %d", rec.Code)
	}
	if rec := post(`{"current_password":"secret123","new_password":"newsecret456"}`); rec.Code != http.StatusOK {
		t.Fatalf("change password: status = %d", rec.Code)
	}
	if code, _, _ := doLogin(t, a.Handler(), "admin", "secret123"); code != http.StatusUnauthorized {
		t.Fatalf("old password still works: status = %d", code)
	}
	if code, _, _ := doLogin(t, a.Handler(), "admin", "newsecret456"); code != http.StatusOK {
		t.Fatalf("new password rejected: status = %d", code)
	}
}

func TestSessionPersistsAcrossRestart(t *testing.T) {
	cfg := config.Load()
	cfg.DataDir = t.TempDir()

	a1, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	createAdmin(t, a1)
	_, cookies, _ := doLogin(t, a1.Handler(), "admin", "secret123")
	if err := a1.Close(); err != nil {
		t.Fatal(err)
	}

	// Simulate a server restart: new App over the same data directory.
	a2, err := New(cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer a2.Close()
	req := httptest.NewRequest(http.MethodGet, "/api/v1/auth/me", nil)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	rec := httptest.NewRecorder()
	a2.Handler().ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("me after restart = %d, want 200", rec.Code)
	}
}
