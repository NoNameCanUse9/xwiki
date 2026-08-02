package server

import (
	"database/sql"
	"io/fs"
	"log/slog"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/handlers"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/user"
	"agentdocs/web"
)

func NewRouter(cfg *config.Config, log *slog.Logger, db *sql.DB, users *user.Store, authSvc *auth.Service) http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.RequestID)
	r.Use(middleware.RequestLogger(log))
	r.Use(middleware.Recoverer(log))
	r.Use(cors.Handler(cors.Options{
		AllowedOrigins:   []string{cfg.WebOrigin},
		AllowedMethods:   []string{"GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"},
		AllowedHeaders:   []string{"Content-Type", "Authorization", "Idempotency-Key", "X-Request-ID"},
		AllowCredentials: true,
		MaxAge:           300,
	}))

	h := handlers.NewAuthHandler(cfg, authSvc, users, log)

	r.Get("/healthz", func(w http.ResponseWriter, r *http.Request) {
		response.WriteJSON(w, http.StatusOK, map[string]any{"status": "ok"})
	})
	r.Get("/readyz", func(w http.ResponseWriter, r *http.Request) {
		if err := db.PingContext(r.Context()); err != nil {
			response.WriteError(w, r, http.StatusServiceUnavailable, "not_ready", "database unavailable")
			return
		}
		response.WriteJSON(w, http.StatusOK, map[string]any{"status": "ready"})
	})

	r.Route("/api/v1", func(r chi.Router) {
		r.NotFound(func(w http.ResponseWriter, r *http.Request) {
			response.WriteError(w, r, http.StatusNotFound, "not_found", "resource not found")
		})
		r.Route("/auth", func(r chi.Router) {
			r.Post("/login", h.Login)
			r.Post("/logout", h.Logout)
			r.Group(func(r chi.Router) {
				r.Use(middleware.SessionAuth(authSvc))
				r.Get("/me", h.Me)
				r.Post("/password", h.Password)
			})
		})
	})

	r.Handle("/*", spaHandler())
	return r
}

// spaHandler serves the embedded frontend with SPA fallback; unknown /api
// paths get a JSON 404 instead of the SPA index.
func spaHandler() http.Handler {
	sub, err := fs.Sub(web.Dist, "dist")
	if err != nil {
		panic(err)
	}
	files := http.FileServer(http.FS(sub))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/api/") {
			response.WriteError(w, r, http.StatusNotFound, "not_found", "resource not found")
			return
		}
		path := strings.TrimPrefix(r.URL.Path, "/")
		if path == "" {
			path = "index.html"
		}
		if _, err := fs.Stat(sub, path); err != nil {
			r.URL.Path = "/"
		}
		files.ServeHTTP(w, r)
	})
}
