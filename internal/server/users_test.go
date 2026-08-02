package server

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"
	"testing"
)

func TestUserManagement(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h) // admin

	// Create a member user.
	rec := apiRequest(h, http.MethodPost, "/api/v1/users", cookie,
		`{"username":"alice","password":"password123","display_name":"Alice"}`)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create user: %d %s", rec.Code, rec.Body.String())
	}
	var created struct {
		User struct {
			ID       string `json:"id"`
			Username string `json:"username"`
			Disabled bool   `json:"disabled"`
		} `json:"user"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if created.User.Username != "alice" || created.User.Disabled {
		t.Fatalf("created user wrong: %+v", created.User)
	}

	// Duplicate username -> 409.
	rec = apiRequest(h, http.MethodPost, "/api/v1/users", cookie,
		`{"username":"alice","password":"password123"}`)
	if rec.Code != http.StatusConflict {
		t.Fatalf("duplicate user: %d", rec.Code)
	}

	// Weak password -> 400.
	rec = apiRequest(h, http.MethodPost, "/api/v1/users", cookie,
		`{"username":"bob","password":"short"}`)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("weak password: %d", rec.Code)
	}

	// Member can log in.
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		`{"username":"alice","password":"password123"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("alice login: %d", rec.Code)
	}
	aliceCookie := ""
	for _, c := range rec.Result().Cookies() {
		if c.Name == "agentdocs_session" {
			aliceCookie = c.Value
		}
	}

	// Member cannot manage users (403).
	rec = apiRequest(h, http.MethodGet, "/api/v1/users", aliceCookie, "")
	if rec.Code != http.StatusForbidden {
		t.Fatalf("member list users: %d, want 403", rec.Code)
	}

	// Admin disables alice.
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/users/"+created.User.ID+"/disable", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("disable: %d %s", rec.Code, rec.Body.String())
	}
	var disabled struct {
		User struct {
			Disabled bool `json:"disabled"`
		} `json:"user"`
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &disabled)
	if !disabled.User.Disabled {
		t.Fatal("disable did not mark user")
	}

	// Disabled user login rejected (403).
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		`{"username":"alice","password":"password123"}`)
	if rec.Code != http.StatusForbidden {
		t.Fatalf("disabled login: %d, want 403", rec.Code)
	}

	// Admin cannot disable self.
	rec = apiRequest(h, http.MethodPost, "/api/v1/users/usr_admin/disable", cookie, "")
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("disable self: %d, want 400", rec.Code)
	}

	// Re-enable works.
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/users/"+created.User.ID+"/enable", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("enable: %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodPost, "/api/v1/auth/login", "",
		`{"username":"alice","password":"password123"}`)
	if rec.Code != http.StatusOK {
		t.Fatalf("alice relogin: %d", rec.Code)
	}

	// List shows both users.
	rec = apiRequest(h, http.MethodGet, "/api/v1/users", cookie, "")
	var list struct {
		Users []struct {
			Username string `json:"username"`
		} `json:"users"`
	}
	_ = json.Unmarshal(rec.Body.Bytes(), &list)
	if len(list.Users) != 2 {
		t.Fatalf("list users: %d", len(list.Users))
	}
}

func TestUnarchiveProject(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "restore-site")

	rec := apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/archive", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("archive: %d", rec.Code)
	}
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/unarchive", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("unarchive: %d %s", rec.Code, rec.Body.String())
	}
	var body struct {
		Project struct {
			Archived   bool   `json:"archived"`
			ArchivedAt string `json:"archived_at"`
		} `json:"project"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Project.Archived || body.Project.ArchivedAt != "" {
		t.Fatalf("unarchived project wrong: %+v", body.Project)
	}
	// Writes work again after restore.
	base := getRevision(t, h, cookie, projectID)
	rec = submitChangeset(t, h, cookie, projectID,
		fmt.Sprintf(`{"base_revision":"%s","message":"write after restore",
		  "changes":[{"op":"create","path":"docs/back.md","content":"# Back\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("write after restore: %d %s", rec.Code, strings.TrimSpace(rec.Body.String()))
	}
	// Idempotent unarchive.
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/unarchive", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("second unarchive: %d", rec.Code)
	}
	// Missing project -> 404.
	rec = apiRequest(h, http.MethodPost,
		"/api/v1/projects/prj_missing/unarchive", cookie, "")
	if rec.Code != http.StatusNotFound {
		t.Fatalf("unarchive missing: %d", rec.Code)
	}
}
