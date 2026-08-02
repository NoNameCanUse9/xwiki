package server

import (
	"database/sql"
	"io/fs"
	"log/slog"
	"net/http"
	"strings"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/cors"

	"agentdocs/internal/agent"
	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi"
	"agentdocs/internal/httpapi/handlers"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
	"agentdocs/internal/search"
	"agentdocs/internal/user"
	"agentdocs/web"
)

func NewRouter(cfg *config.Config, log *slog.Logger, db *sql.DB, users *user.Store, authSvc *auth.Service, projectsSvc *project.Service, agentSvc *agent.Service, searchSvc *search.Service) http.Handler {
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
	ph := handlers.NewProjectHandler(cfg, projectsSvc, log)
	dh := handlers.NewDocsHandler(cfg, projectsSvc, agentSvc, log)
	ch := handlers.NewChangesetHandler(cfg, projectsSvc, agentSvc, searchSvc, log)
	th := handlers.NewTokenHandler(cfg, agentSvc, log)
	xh := handlers.NewTransferHandler(cfg, projectsSvc, agentSvc, log)
	uh := handlers.NewUserHandler(cfg, authSvc, users, log)
	hh := handlers.NewHistoryHandler(cfg, projectsSvc, searchSvc, log)
	sh := handlers.NewSearchHandler(cfg, searchSvc, projectsSvc, agentSvc, log)

	r.Get("/api/openapi.json", httpapi.OpenAPIHandler)
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
		r.Post("/import/bundle", xh.ImportBundle)
		r.Route("/users", func(r chi.Router) {
			r.Group(func(r chi.Router) {
				r.Use(middleware.SessionAuth(authSvc))
				r.Use(middleware.AdminOnly)
				r.Get("/", uh.List)
				r.Post("/", uh.Create)
				r.Post("/{id}/disable", uh.Disable)
				r.Post("/{id}/enable", uh.Enable)
			})
		})
		r.Route("/tokens", func(r chi.Router) {
			r.Group(func(r chi.Router) {
				r.Use(middleware.SessionAuth(authSvc))
				r.Post("/", th.Create)
				r.Get("/", th.List)
				r.Delete("/{id}", th.Revoke)
			})
		})
		r.Route("/projects", func(r chi.Router) {
			r.Group(func(r chi.Router) {
				r.Use(middleware.AgentAuth(agentSvc))
				r.Use(middleware.SessionAuth(authSvc))
				r.Post("/", ph.Create)
				r.Get("/", ph.List)
				r.Get("/{id}", ph.Get)
				r.Post("/{id}/archive", ph.Archive)
				r.Post("/{id}/unarchive", ph.Unarchive)
				r.Get("/{id}/docs/tree", dh.Tree)
				r.Get("/{id}/docs/home", dh.Home)
				r.Get("/{id}/docs/pages/*", dh.Page)
				r.Get("/{id}/revision", ch.Revision)
				r.Post("/{id}/changesets", ch.Apply)
				r.Get("/{id}/search", sh.Search)
			r.Get("/{id}/export.zip", xh.ExportZip)
			r.Get("/{id}/export.bundle", xh.ExportBundle)
			r.Post("/{id}/import", xh.Import)
			r.Get("/{id}/audit", th.Audit)
			r.Get("/{id}/commits", hh.Commits)
				r.Get("/{id}/commits/{sha}", hh.Commit)
				r.Get("/{id}/commits/{sha}/diff", hh.Diff)
				r.Post("/{id}/commits/{sha}/revert", hh.Revert)
				r.Get("/{id}/files/history/*", hh.FileHistory)
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
