package middleware

import (
	"context"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"

	"xwiki/internal/agent"
	"xwiki/internal/httpapi/response"
)

type agentCtxKey int

const (
	agentTokenKey agentCtxKey = iota
	agentNameKey
	agentSecretKey
	agentProjectIDsKey
)

// AgentAuth authenticates Bearer agent tokens. Requests without a Bearer
// header pass through untouched so SessionAuth can handle them (OR semantics).
// The raw secret is stored in context for project/path-scoped re-checks.
func AgentAuth(svc *agent.Service) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			hdr := r.Header.Get("Authorization")
			if !strings.HasPrefix(hdr, "Bearer ") {
				next.ServeHTTP(w, r)
				return
			}
			secret := strings.TrimSpace(strings.TrimPrefix(hdr, "Bearer "))
			t, err := svc.Authorize(r.Context(), secret, "", false)
			if err != nil {
				response.WriteError(w, r, http.StatusUnauthorized, "invalid_token", "invalid or revoked agent token")
				return
			}
			ctx := contextWithAgent(r.Context(), t.ID, t.Name, secret, t.ProjectIDs)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

func contextWithAgent(ctx context.Context, tokenID, tokenName, secret string, projectIDs []string) context.Context {
	ctx = context.WithValue(ctx, agentTokenKey, tokenID)
	ctx = context.WithValue(ctx, agentNameKey, tokenName)
	ctx = context.WithValue(ctx, agentSecretKey, secret)
	return context.WithValue(ctx, agentProjectIDsKey, append([]string(nil), projectIDs...))
}

// AgentTokenName returns the authenticated token's display name.
func AgentTokenName(r *http.Request) string {
	n, _ := r.Context().Value(agentNameKey).(string)
	return n
}

// CommitAuthorIdentity returns the git identity of the authenticated actor.
func CommitAuthorIdentity(r *http.Request) (name, email string) {
	if tokenID := AgentTokenID(r); tokenID != "" {
		n := AgentTokenName(r)
		if n == "" {
			n = tokenID
		}
		return n, "token-" + tokenID + "@xwiki.local"
	}
	if u := UserFrom(r); u != nil {
		n := u.DisplayName
		if n == "" {
			n = u.Username
		}
		return n, u.Username + "@xwiki.local"
	}
	return "anonymous", "anonymous@xwiki.local"
}

// AgentTokenID returns the authenticated agent token id, or "" for session users.
func AgentTokenID(r *http.Request) string {
	id, _ := r.Context().Value(agentTokenKey).(string)
	return id
}

// AgentSecret returns the raw agent token secret, or "" when the request was
// authenticated by session cookie instead.
func AgentSecret(r *http.Request) string {
	s, _ := r.Context().Value(agentSecretKey).(string)
	return s
}

// AgentProjectIDs returns the projects explicitly bound to the authenticated
// token. Session requests return nil.
func AgentProjectIDs(r *http.Request) []string {
	ids, _ := r.Context().Value(agentProjectIDsKey).([]string)
	return ids
}

// AgentProjectAccess rejects a token that is not bound to the project in the
// current {id} route. Session users pass through unchanged.
func AgentProjectAccess(svc *agent.Service) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			secret := AgentSecret(r)
			if secret == "" {
				next.ServeHTTP(w, r)
				return
			}
			projectID := r.PathValue("id")
			if projectID == "" {
				projectID = chi.URLParam(r, "id")
			}
			if _, err := svc.Authorize(r.Context(), secret, projectID, false); err != nil {
				response.WriteError(w, r, http.StatusForbidden, "agent_forbidden", "token is not authorized for this project")
				return
			}
			next.ServeHTTP(w, r)
		})
	}
}

// HumanOnly keeps administrative project operations behind a user session.
func HumanOnly(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if AgentTokenID(r) != "" {
			response.WriteError(w, r, http.StatusForbidden, "session_required", "this operation requires a user session")
			return
		}
		next.ServeHTTP(w, r)
	})
}

// ActorID returns the authenticated actor id (session user or agent token).
func ActorID(r *http.Request) string {
	if id := AgentTokenID(r); id != "" {
		return id
	}
	if u := UserFrom(r); u != nil {
		return u.ID
	}
	return ""
}

// ActorType returns "token" or "user" for the authenticated actor.
func ActorType(r *http.Request) string {
	if AgentTokenID(r) != "" {
		return "token"
	}
	return "user"
}
