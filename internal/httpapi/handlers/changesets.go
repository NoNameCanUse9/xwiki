package handlers

import (
	"errors"
	"log/slog"
	"net/http"

	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

// ChangesetHandler serves write endpoints for project documents.
type ChangesetHandler struct {
	cfg *config.Config
	svc *project.Service
	log *slog.Logger
}

func NewChangesetHandler(cfg *config.Config, svc *project.Service, log *slog.Logger) *ChangesetHandler {
	return &ChangesetHandler{cfg: cfg, svc: svc, log: log}
}

// Revision handles GET /api/v1/projects/{id}/revision.
func (h *ChangesetHandler) Revision(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	repo, err := h.svc.OpenRepo(r.Context(), projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		h.log.Error("open repo failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not read repository")
		return
	}
	rev, err := repo.Revision(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve revision")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"revision": rev})
}

// Apply handles POST /api/v1/projects/{id}/changesets.
func (h *ChangesetHandler) Apply(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	var input project.ChangesetInput
	if err := request.DecodeJSON(w, r, &input, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_changeset", "invalid request body")
		return
	}
	if r.URL.Query().Get("dry_run") == "true" {
		input.DryRun = true
	}
	res, err := h.svc.ApplyChangeset(r.Context(), projectID, input)
	if err != nil {
		switch {
		case errors.Is(err, project.ErrNotFound):
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		case errors.Is(err, project.ErrConflict):
			response.WriteError(w, r, http.StatusConflict, "revision_conflict",
				"base revision is stale; reload and retry")
		case errors.Is(err, project.ErrArchived):
			response.WriteError(w, r, http.StatusGone, "project_archived", "project is archived")
		default:
			h.log.Error("apply changeset failed", "error", err, "request_id", request.RequestID(r))
			response.WriteError(w, r, http.StatusBadRequest, "invalid_changeset", err.Error())
		}
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"commit":   res.Commit,
		"revision": res.Revision,
		"preview":  res.Preview,
	})
}
