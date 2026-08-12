package server

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

// createAgentToken creates a token via the API and returns its secret.
func createAgentToken(t *testing.T, h http.Handler, cookie, payload string) string {
	t.Helper()
	rec := apiRequest(h, http.MethodPost, "/api/v1/tokens", cookie, payload)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create token: %d %s", rec.Code, rec.Body.String())
	}
	var body struct {
		Secret string `json:"secret"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	return body.Secret
}

func bearerRequest(h http.Handler, method, path, secret, payload string) *httptest.ResponseRecorder {
	var body io.Reader
	if payload != "" {
		body = strings.NewReader(payload)
	}
	req := httptest.NewRequest(method, path, body)
	if secret != "" {
		req.Header.Set("Authorization", "Bearer "+secret)
	}
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	return rec
}

func TestAgentTokenLifecycle(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	projectID, _ := createProjectViaAPI(t, h, cookie, "agent-site")

	// Create a write token bound to the project.
	secret := createAgentToken(t, h, cookie,
		fmt.Sprintf(`{"name":"ci-bot","scope":"write","project_ids":["%s"]}`, projectID))

	// Read as token.
	rec := bearerRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/tree?path=", secret, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("token read: %d %s", rec.Code, rec.Body.String())
	}

	// Write within the bound project.
	base := getRevision(t, h, cookie, projectID)
	rec = bearerRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/changesets", secret,
		fmt.Sprintf(`{"base_revision":"%s","message":"agent write",
		  "changes":[{"op":"create","path":"docs/agent.md","content":"# Agent\n"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("token write: %d %s", rec.Code, rec.Body.String())
	}

	// A project-bound token can write any path in that project.
	base = getRevision(t, h, cookie, projectID)
	rec = bearerRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/changesets", secret,
		fmt.Sprintf(`{"base_revision":"%s","message":"any path",
		  "changes":[{"op":"create","path":"README.md","content":"x"}]}`, base))
	if rec.Code != http.StatusOK {
		t.Fatalf("project-scoped write: %d, want 200", rec.Code)
	}

	// Acceptance 1: another project is unreachable.
	otherPID, _ := createProjectViaAPI(t, h, cookie, "other-site")
	rec = bearerRequest(h, http.MethodGet,
		"/api/v1/projects/"+otherPID+"/docs/tree?path=", secret, "")
	if rec.Code != http.StatusForbidden {
		t.Fatalf("cross-project read: %d, want 403", rec.Code)
	}

	// Read-scope token cannot write.
	readSecret := createAgentToken(t, h, cookie,
		fmt.Sprintf(`{"name":"reader","scope":"read","project_ids":["%s"]}`, projectID))
	rec = bearerRequest(h, http.MethodPost,
		"/api/v1/projects/"+projectID+"/changesets", readSecret,
		fmt.Sprintf(`{"base_revision":"%s","message":"nope",
		  "changes":[{"op":"create","path":"docs/x.md","content":"x"}]}`, base))
	if rec.Code != http.StatusForbidden {
		t.Fatalf("read-scope write: %d, want 403", rec.Code)
	}

	// Acceptance 3: idempotency key replay produces no second commit.
	baseNow := getRevision(t, h, cookie, projectID)
	payload := fmt.Sprintf(`{"base_revision":"%s","message":"idem",
	  "changes":[{"op":"create","path":"docs/idem.md","content":"i\n"}]}`, baseNow)
	reqBody := strings.NewReader(payload)
	req := httptest.NewRequest(http.MethodPost,
		"/api/v1/projects/"+projectID+"/changesets", reqBody)
	req.Header.Set("Authorization", "Bearer "+secret)
	req.Header.Set("Idempotency-Key", "idem-1")
	rec1 := httptest.NewRecorder()
	h.ServeHTTP(rec1, req)
	if rec1.Code != http.StatusOK {
		t.Fatalf("idem first: %d %s", rec1.Code, rec1.Body.String())
	}
	t.Logf("first body: %s", rec1.Body.String())
	req2 := httptest.NewRequest(http.MethodPost,
		"/api/v1/projects/"+projectID+"/changesets", strings.NewReader(payload))
	req2.Header.Set("Authorization", "Bearer "+secret)
	req2.Header.Set("Idempotency-Key", "idem-1")
	rec2 := httptest.NewRecorder()
	h.ServeHTTP(rec2, req2)
	if rec2.Code != http.StatusOK || rec2.Body.String() != rec1.Body.String() {
		t.Fatalf("idem replay mismatch: %d vs %d", rec2.Code, rec1.Code)
	}
	headAfterFirst := getRevision(t, h, cookie, projectID)
	if headAfterFirst == baseNow {
		t.Fatal("first idempotent request did not commit")
	}
	headAfterReplay := getRevision(t, h, cookie, projectID)
	if headAfterFirst != headAfterReplay {
		t.Fatalf("idempotent replay created a commit: %s -> %s", headAfterFirst, headAfterReplay)
	}

	// Reused key with different payload -> 409.
	req3 := httptest.NewRequest(http.MethodPost,
		"/api/v1/projects/"+projectID+"/changesets",
		strings.NewReader(strings.Replace(payload, "idem.md", "idem2.md", 1)))
	req3.Header.Set("Authorization", "Bearer "+secret)
	req3.Header.Set("Idempotency-Key", "idem-1")
	rec3 := httptest.NewRecorder()
	h.ServeHTTP(rec3, req3)
	if rec3.Code != http.StatusConflict {
		t.Fatalf("idem conflict: %d, want 409", rec3.Code)
	}

	// Revoke kills the token.
	var list struct {
		Tokens []struct {
			ID   string `json:"id"`
			Name string `json:"name"`
		} `json:"tokens"`
	}
	rec = apiRequest(h, http.MethodGet, "/api/v1/tokens", cookie, "")
	if err := json.Unmarshal(rec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	var tokenID string
	for _, token := range list.Tokens {
		if token.Name == "ci-bot" {
			tokenID = token.ID
			break
		}
	}
	if tokenID == "" {
		t.Fatalf("ci-bot missing from token list: %+v", list.Tokens)
	}
	rec = apiRequest(h, http.MethodDelete, "/api/v1/tokens/"+tokenID, cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("revoke: %d", rec.Code)
	}
	rec = bearerRequest(h, http.MethodGet,
		"/api/v1/projects/"+projectID+"/docs/tree?path=", secret, "")
	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("revoked token still works: %d", rec.Code)
	}

	// Audit trail has the change entry.
	entries := agentAudit(t, h, cookie, projectID)
	found := false
	for _, e := range entries {
		if e.Action == "change" && e.ActorType == "token" {
			found = true
		}
	}
	if !found {
		t.Fatalf("audit missing token change: %+v", entries)
	}
}

func TestAgentTokenProjectVisibilityAndManagementBoundary(t *testing.T) {
	h, _ := newTestRouterWithService(t)
	cookie := loginAndGetCookie(t, h)
	allowedID, _ := createProjectViaAPI(t, h, cookie, "agent-visible")
	hiddenID, _ := createProjectViaAPI(t, h, cookie, "agent-hidden")
	secret := createAgentToken(t, h, cookie,
		fmt.Sprintf(`{"name":"reader","scope":"read","project_ids":["%s"]}`, allowedID))

	rec := bearerRequest(h, http.MethodGet, "/api/v1/projects", secret, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("list projects: %d %s", rec.Code, rec.Body.String())
	}
	projects := decodeProjects(t, rec)
	if len(projects) != 1 || projects[0]["id"] != allowedID {
		t.Fatalf("token project list = %#v, want only %s", projects, allowedID)
	}

	for _, tc := range []struct {
		name string
		path string
		want int
	}{
		{name: "get allowed", path: "/api/v1/projects/" + allowedID, want: http.StatusOK},
		{name: "get hidden", path: "/api/v1/projects/" + hiddenID, want: http.StatusForbidden},
	} {
		t.Run(tc.name, func(t *testing.T) {
			rec := bearerRequest(h, http.MethodGet, tc.path, secret, "")
			if rec.Code != tc.want {
				t.Fatalf("status = %d body = %s, want %d", rec.Code, rec.Body.String(), tc.want)
			}
		})
	}

	for _, tc := range []struct {
		name    string
		method  string
		path    string
		payload string
	}{
		{name: "create", method: http.MethodPost, path: "/api/v1/projects", payload: `{"name":"forbidden-create"}`},
		{name: "rename", method: http.MethodPatch, path: "/api/v1/projects/" + allowedID, payload: `{"name":"forbidden-rename"}`},
		{name: "archive", method: http.MethodPost, path: "/api/v1/projects/" + allowedID + "/archive"},
		{name: "delete", method: http.MethodDelete, path: "/api/v1/projects/" + allowedID},
		{name: "restore deleted", method: http.MethodPost, path: "/api/v1/projects/" + allowedID + "/restore"},
		{name: "purge deleted", method: http.MethodDelete, path: "/api/v1/projects/" + allowedID + "/purge"},
		{name: "revert", method: http.MethodPost, path: "/api/v1/projects/" + allowedID + "/commits/deadbeef/revert"},
		{name: "force unlock", method: http.MethodPost, path: "/api/v1/projects/" + allowedID + "/locks/force-release"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			rec := bearerRequest(h, tc.method, tc.path, secret, tc.payload)
			if rec.Code != http.StatusForbidden {
				t.Fatalf("status = %d body = %s, want 403", rec.Code, rec.Body.String())
			}
		})
	}
}

func agentAudit(t *testing.T, h http.Handler, cookie, projectID string) []struct {
	Action    string `json:"action"`
	ActorType string `json:"actor_type"`
} {
	t.Helper()
	rec := apiRequest(h, http.MethodGet, "/api/v1/projects/"+projectID+"/audit", cookie, "")
	if rec.Code != http.StatusOK {
		t.Fatalf("audit: %d", rec.Code)
	}
	var body struct {
		Entries []struct {
			Action    string `json:"action"`
			ActorType string `json:"actor_type"`
		} `json:"entries"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	return body.Entries
}
