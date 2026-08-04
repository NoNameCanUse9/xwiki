package server

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

type lockResponse struct {
	Lock *struct {
		Path      string `json:"path"`
		UserID    string `json:"user_id"`
		Username  string `json:"username"`
		ExpiresAt string `json:"expires_at"`
	} `json:"lock"`
}

// createAndLoginUser creates a non-admin user via the API and logs in.
func createAndLoginUser(t *testing.T, h http.Handler, adminCookie, username string) string {
	t.Helper()
	rec := apiRequest(h, http.MethodPost, "/api/v1/users", adminCookie,
		`{"username":"`+username+`","password":"password123","display_name":"`+username+`"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create user %s: %d %s", username, rec.Code, rec.Body.String())
	}
	return loginCookie(t, h, username, "password123")
}

func loginCookie(t *testing.T, h http.Handler, username, password string) string {
	t.Helper()
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/auth/login",
		strings.NewReader(`{"username":"`+username+`","password":"`+password+`"}`)))
	if rec.Code != http.StatusOK {
		t.Fatalf("login %s: %d %s", username, rec.Code, rec.Body.String())
	}
	for _, c := range rec.Result().Cookies() {
		if c.Name == "agentdocs_session" {
			return c.Value
		}
	}
	t.Fatal("no session cookie set")
	return ""
}

func lockPath(method, projectID, path string) string {
	return "/api/v1/projects/" + projectID + "/locks?path=" + path
}

func getLock(t *testing.T, h http.Handler, cookie, projectID, path string) *lockResponse {
	t.Helper()
	rec := apiRequest(h, http.MethodGet, lockPath("", projectID, path), cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("status: code = %d body = %s", rec.Code, rec.Body.String())
	}
	var body lockResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	return &body
}

func acquireLock(t *testing.T, h http.Handler, cookie, projectID, path string) *httptest.ResponseRecorder {
	t.Helper()
	return apiRequest(h, http.MethodPost, lockPath("", projectID, path), cookie, "")
}

func TestEditLockLifecycle(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookieA := loginAndGetCookie(t, h) // admin
	cookieB := createAndLoginUser(t, h, cookieA, "user_b")
	projectID, _ := createProjectViaAPI(t, h, cookieA, "lock-site")
	const path = "docs/guide.md"

	// No lock yet.
	if lock := getLock(t, h, cookieA, projectID, path).Lock; lock != nil {
		t.Fatalf("expected no lock, got %+v", lock)
	}

	// User A acquires.
	if rec := acquireLock(t, h, cookieA, projectID, path); rec.Code != http.StatusOK {
		t.Fatalf("acquire: code = %d body = %s", rec.Code, rec.Body.String())
	}
	if lock := getLock(t, h, cookieA, projectID, path).Lock; lock == nil || lock.Path != path || lock.Username == "" {
		t.Fatalf("lock not visible after acquire: %+v", lock)
	}

	// User B is rejected with page_locked + the holder's username.
	rec := acquireLock(t, h, cookieB, projectID, path)
	if rec.Code != http.StatusConflict {
		t.Fatalf("second acquire: code = %d, want 409", rec.Code)
	}
	var errBody struct {
		Error struct {
			Code    string `json:"code"`
			Message string `json:"message"`
			Data    struct {
				Lock struct {
					Username string `json:"username"`
				} `json:"lock"`
			} `json:"data"`
		} `json:"error"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &errBody); err != nil {
		t.Fatal(err)
	}
	if errBody.Error.Code != "page_locked" || errBody.Error.Data.Lock.Username == "" {
		t.Fatalf("conflict body = %s", rec.Body.String())
	}

	// B cannot release or heartbeat A's lock.
	if r := apiRequest(h, http.MethodDelete, lockPath("", projectID, path), cookieB, ""); r.Code != http.StatusForbidden {
		t.Fatalf("foreign release: code = %d, want 403", r.Code)
	}
	if r := apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/locks/heartbeat?path="+path, cookieB, ""); r.Code != http.StatusForbidden {
		t.Fatalf("foreign heartbeat: code = %d, want 403", r.Code)
	}

	// A renews the lease.
	if rec := apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/locks/heartbeat?path="+path, cookieA, ""); rec.Code != http.StatusOK {
		t.Fatalf("heartbeat: code = %d body = %s", rec.Code, rec.Body.String())
	}

	// B force-releases (any signed-in user may).
	if rec := apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/locks/force-release?path="+path, cookieB, ""); rec.Code != http.StatusOK {
		t.Fatalf("force-release: code = %d body = %s", rec.Code, rec.Body.String())
	}
	if lock := getLock(t, h, cookieB, projectID, path).Lock; lock != nil {
		t.Fatalf("lock still present after force-release: %+v", lock)
	}

	// A re-acquires after the force-release, then releases normally.
	if rec := acquireLock(t, h, cookieA, projectID, path); rec.Code != http.StatusOK {
		t.Fatalf("re-acquire: code = %d body = %s", rec.Code, rec.Body.String())
	}
	if rec := apiRequest(h, http.MethodDelete, lockPath("", projectID, path), cookieA, ""); rec.Code != http.StatusOK {
		t.Fatalf("release: code = %d body = %s", rec.Code, rec.Body.String())
	}
	if lock := getLock(t, h, cookieA, projectID, path).Lock; lock != nil {
		t.Fatalf("lock still present after release: %+v", lock)
	}

	// Releasing an already-free lock is a no-op, not an error.
	if rec := apiRequest(h, http.MethodDelete, lockPath("", projectID, path), cookieA, ""); rec.Code != http.StatusOK {
		t.Fatalf("double release: code = %d body = %s", rec.Code, rec.Body.String())
	}
}

func TestEditLockRejectsBadPath(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "lock-badpath")
	for _, p := range []string{"", "../etc/passwd", "/abs/path"} {
		rec := apiRequest(h, http.MethodPost,
			"/api/v1/projects/"+projectID+"/locks?path="+p, cookie, "")
		if rec.Code != http.StatusBadRequest {
			t.Fatalf("path %q: code = %d, want 400", p, rec.Code)
		}
	}
}

func TestLockConflictEnvelope(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "lock-env")
	const path = "a.md"
	if rec := acquireLock(t, h, cookie, projectID, path); rec.Code != http.StatusOK {
		t.Fatalf("acquire: %d %s", rec.Code, rec.Body.String())
	}
	rec := acquireLock(t, h, cookie, projectID, path)
	if rec.Code != http.StatusConflict {
		t.Fatalf("code = %d", rec.Code)
	}
	if !strings.Contains(rec.Body.String(), `"code":"page_locked"`) {
		t.Fatalf("envelope = %s", rec.Body.String())
	}
}
