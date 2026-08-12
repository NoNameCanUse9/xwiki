package handlers

import (
	"log/slog"
	"net/http"
	"strconv"
	"strings"

	"xwiki/internal/agent"
	"xwiki/internal/config"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
	"xwiki/internal/project"
	"xwiki/internal/search"
)

// SearchHandler serves project-scoped full-text search.
type SearchHandler struct {
	cfg      *config.Config
	svc      *search.Service
	projects *project.Service
	agentSvc *agent.Service
	log      *slog.Logger
}

// NewSearchHandler wires the search handler.
func NewSearchHandler(cfg *config.Config, svc *search.Service, projects *project.Service, agentSvc *agent.Service, log *slog.Logger) *SearchHandler {
	return &SearchHandler{cfg: cfg, svc: svc, projects: projects, agentSvc: agentSvc, log: log}
}

// Backlinks handles GET /api/v1/projects/{id}/backlinks?path=...
func (h *SearchHandler) Backlinks(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	filePath := r.URL.Query().Get("path")
	if filePath == "" {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_path", "path query required")
		return
	}
	links, err := h.svc.Backlinks(r.Context(), projectID, filePath)
	if err != nil {
		h.log.Error("backlinks failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "backlinks failed")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"path": filePath, "backlinks": links,
	})
}

// Search handles GET /api/v1/projects/{id}/search?q=...
func (h *SearchHandler) Search(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	q := strings.TrimSpace(r.URL.Query().Get("q"))
	if q == "" || len(q) > 200 {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_query", "query must be 1-200 characters")
		return
	}
	limit, _ := strconv.Atoi(r.URL.Query().Get("limit"))
	results, err := h.svc.Search(r.Context(), projectID, q, limit)
	if err != nil {
		h.log.Error("search failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "search failed")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"query":   q,
		"results": results,
	})
}
