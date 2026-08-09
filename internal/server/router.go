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
	ph := handlers.NewProjectHandler(cfg, projectsSvc, searchSvc, log)
	dh := handlers.NewDocsHandler(cfg, projectsSvc, agentSvc, log)
	ch := handlers.NewChangesetHandler(cfg, projectsSvc, agentSvc, searchSvc, log)
	th := handlers.NewTokenHandler(cfg, agentSvc, log)
	xh := handlers.NewTransferHandler(cfg, projectsSvc, agentSvc, log)
	ah := handlers.NewAttachmentHandler(cfg, projectsSvc, agentSvc, log)
	uh := handlers.NewUserHandler(cfg, authSvc, users, log)
	gh := handlers.NewGitHTTPHandler(projectsSvc, agentSvc, log)
	lh := handlers.NewLockHandler(db, log)
	hh := handlers.NewHistoryHandler(cfg, projectsSvc, searchSvc, log)
	sh := handlers.NewSearchHandler(cfg, searchSvc, projectsSvc, agentSvc, log)
	shareH := handlers.NewShareHandler(db, dh, projectsSvc, log)

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
		r.Get("/meta", httpapi.MetaHandler)
		r.NotFound(func(w http.ResponseWriter, r *http.Request) {
			response.WriteError(w, r, http.StatusNotFound, "not_found", "resource not found")
		})
		r.Route("/auth", func(r chi.Router) {
			r.Post("/login", h.Login)
			r.Post("/logout", h.Logout)
			r.Post("/forgot-password", h.ForgotPassword)
			r.Post("/reset-password", h.ResetPassword)
			r.Group(func(r chi.Router) {
				r.Use(middleware.SessionAuth(authSvc))
				r.Get("/me", h.Me)
				r.Post("/password", h.Password)
			})
		})
		r.Group(func(r chi.Router) {
			r.Use(middleware.SessionAuth(authSvc))
			r.Post("/import/bundle", xh.ImportBundle)
			r.Post("/import/repo", xh.ImportRepo)
		})
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
				r.Post("/import-folder", ph.ImportFolder)
				r.Get("/", ph.List)
				r.Get("/{id}", ph.Get)
				r.Patch("/{id}", ph.Rename)
				r.Delete("/{id}", ph.Delete)
				r.Post("/{id}/archive", ph.Archive)
				r.Post("/{id}/unarchive", ph.Unarchive)
				r.Post("/{id}/purge", ph.Purge)
				r.Get("/{id}/docs/tree", dh.Tree)
				r.Get("/{id}/docs/home", dh.Home)
				r.Get("/{id}/docs/pages/*", dh.Page)
				r.Get("/{id}/revision", ch.Revision)
				r.Post("/{id}/changesets", ch.Apply)
				r.Get("/{id}/locks", lh.Status)
				r.Post("/{id}/locks", lh.Acquire)
				r.Delete("/{id}/locks", lh.Release)
				r.Post("/{id}/locks/heartbeat", lh.Heartbeat)
				r.Post("/{id}/locks/force-release", lh.ForceRelease)
				r.Post("/{id}/shares", shareH.Create)
				r.Get("/{id}/search", sh.Search)
				r.Get("/{id}/backlinks", sh.Backlinks)
				r.Get("/{id}/export.zip", xh.ExportZip)
				r.Get("/{id}/export.bundle", xh.ExportBundle)
				r.Post("/{id}/import", xh.Import)
				r.Get("/{id}/attachments/*", ah.Download)
				r.Get("/{id}/audit", th.Audit)
				r.Get("/{id}/commits", hh.Commits)
				r.Get("/{id}/commits/{sha}", hh.Commit)
				r.Get("/{id}/commits/{sha}/diff", hh.Diff)
				r.Post("/{id}/commits/{sha}/revert", hh.Revert)
				r.Get("/{id}/files/history/*", hh.FileHistory)
			})
		})
	})

	// Public per-page share links: /share/{token} renders a standalone page.
	r.Get("/share/{token}", shareH.View)

	// Docs URL is a SPA route for browsers, but agents/crawlers/curl get a
	// server-rendered HTML page so a shared docs link "just works" without
	// JavaScript (e.g. Claude fetching the URL directly).
	spa := spaHandler()
	r.Get("/projects/{id}/docs/*", func(w http.ResponseWriter, r *http.Request) {
		if isBrowserAgent(r) {
			spa.ServeHTTP(w, r)
			return
		}
		dh.ServeView(w, r)
	})
	r.Handle("/git/{projectID}/*", gh)
	r.Handle("/*", spa)
	return r
}

// isBrowserAgent reports whether the client is a real browser (keep the SPA)
// vs an agent/crawler/CLI that should get server-rendered HTML.
func isBrowserAgent(r *http.Request) bool {
	ua := strings.ToLower(r.UserAgent())
	if ua == "" {
		return false
	}
	for _, marker := range []string{"bot", "spider", "crawler", "curl", "wget", "python", "node", "claude", "gptbot"} {
		if strings.Contains(ua, marker) {
			return false
		}
	}
	return strings.Contains(ua, "mozilla")
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
