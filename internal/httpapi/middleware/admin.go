package middleware

import (
	"net/http"

	"agentdocs/internal/httpapi/response"
)

// AdminOnly rejects non-admin session users.
func AdminOnly(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		u := UserFrom(r)
		if u == nil || !u.IsAdmin {
			response.WriteError(w, r, http.StatusForbidden, "admin_required", "admin privileges required")
			return
		}
		next.ServeHTTP(w, r)
	})
}
