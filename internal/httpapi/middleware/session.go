package middleware

import (
	"context"
	"net/http"

	"agentdocs/internal/auth"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/user"
)

type ctxKey int

const userKey ctxKey = 0

// SessionAuth requires a valid session cookie and stores the user in context.
func SessionAuth(svc *auth.Service) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Requests already authenticated by AgentAuth (Bearer) pass through.
			if AgentTokenID(r) != "" {
				next.ServeHTTP(w, r)
				return
			}
			cookie, err := r.Cookie("agentdocs_session")
			if err != nil {
				response.WriteError(w, r, http.StatusUnauthorized, "authentication_required", "login required")
				return
			}
			_, u, err := svc.ResolveSession(r.Context(), cookie.Value)
			if err != nil {
				response.WriteError(w, r, http.StatusUnauthorized, "authentication_required", "session is invalid or expired")
				return
			}
			ctx := context.WithValue(r.Context(), userKey, u)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	}
}

// UserFrom returns the authenticated user stored by SessionAuth, or nil.
func UserFrom(r *http.Request) *user.User {
	u, _ := r.Context().Value(userKey).(*user.User)
	return u
}
