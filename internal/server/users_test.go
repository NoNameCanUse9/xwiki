package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"testing"
)

func TestDeleteUser(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)

	// Create a member.
	rec := apiRequest(h, http.MethodPost, "/api/v1/users", cookie,
		`{"username":"alice","password":"firstpass123","display_name":"Alice"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create user: %d %s", rec.Code, rec.Body.String())
	}
	var created struct {
		User struct {
			ID string `json:"id"`
		} `json:"user"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	aliceID := created.User.ID

	// Alice logs in and keeps a session.
	aliceCookie := loginAndGetCookieWith(t, h, "alice", "firstpass123")

	// Admin-only route: a non-admin cannot delete users.
	rec = apiRequest(h, http.MethodDelete, "/api/v1/users/"+aliceID, aliceCookie, "")
	if rec.Code != http.StatusForbidden {
		t.Fatalf("non-admin delete: %d, want 403", rec.Code)
	}

	// Deleting the user invalidates their existing session.
	rec = apiRequest(h, http.MethodDelete, "/api/v1/users/"+aliceID, cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("delete user: %d %s", rec.Code, rec.Body.String())
	}
	rec = apiRequest(h, http.MethodGet, "/api/v1/auth/me", aliceCookie, "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("deleted user session: %d, want 401", rec.Code)
	}
	// Login also fails.
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		`{"username":"alice","password":"firstpass123"}`)
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("deleted user login: %d, want 401", rec.Code)
	}
}

func TestDeleteUserProtections(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)

	// Cannot delete yourself.
	rec := apiRequest(h, http.MethodDelete, "/api/v1/users/usr_admin", cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("delete self: %d, want 400", rec.Code)
	}
	// Cannot delete an admin account.
	rec = apiRequest(h, http.MethodPost, "/api/v1/users", cookie,
		`{"username":"bob","password":"password123","is_admin":true}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create admin: %d", rec.Code)
	}
	var created struct {
		User struct {
			ID string `json:"id"`
		} `json:"user"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	rec = apiRequest(h, http.MethodDelete, "/api/v1/users/"+created.User.ID, cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("delete admin: %d, want 400", rec.Code)
	}
	// Unknown user -> 404.
	rec = apiRequest(h, http.MethodDelete, "/api/v1/users/usr_missing", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("delete missing: %d, want 404", rec.Code)
	}
}

func loginAndGetCookieWith(t *testing.T, h http.Handler, username, password string) string {
	t.Helper()
	rec := apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		fmt.Sprintf(`{"username":%q,"password":%q}`, username, password))
	if rec.Code != http.StatusOK {
		t.Fatalf("login %s: %d %s", username, rec.Code, rec.Body.String())
	}
	cookie := ""
	for _, c := range rec.Result().Cookies() {
		if c.Name == "xwiki_session" {
			cookie = c.Value
		}
	}
	if cookie == "" {
		t.Fatal("no session cookie set")
	}
	return cookie
}
