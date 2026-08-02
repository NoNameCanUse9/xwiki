package handlers

import (
	"errors"
	"log/slog"
	"net/http"
	"strconv"

	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

// HistoryHandler serves read-only history and revert endpoints.
type HistoryHandler struct {
	cfg *config.Config
	svc *project.Service
	log *slog.Logger
}

func NewHistoryHandler(cfg *config.Config, svc *project.Service, log *slog.Logger) *HistoryHandler {
	return &HistoryHandler{cfg: cfg, svc: svc, log: log}
}

// Commits handles GET /api/v1/projects/{id}/commits.
func (h *HistoryHandler) Commits(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	limit, _ := strconv.Atoi(r.URL.Query().Get("limit"))
	offset, _ := strconv.Atoi(r.URL.Query().Get("offset"))
	commits, err := h.svc.ListCommits(r.Context(), projectID, limit, offset)
	if err != nil {
		h.writeRepoError(w, r, err)
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"commits": commits})
}

// Commit handles GET /api/v1/projects/{id}/commits/{sha}.
func (h *HistoryHandler) Commit(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	sha := request.PathParam(r, "sha")
	detail, err := h.svc.GetCommit(r.Context(), projectID, sha)
	if err != nil {
		h.writeRepoError(w, r, err)
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"commit": detail})
}

// FileHistory handles GET /api/v1/projects/{id}/files/{path}/history.
func (h *HistoryHandler) FileHistory(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	filePath := request.PathParam(r, "*")
	commits, err := h.svc.FileHistory(r.Context(), projectID, filePath)
	if err != nil {
		h.writeRepoError(w, r, err)
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"path": filePath, "commits": commits})
}

// Diff handles GET /api/v1/projects/{id}/commits/{sha}/diff?format=numstat|patch.
func (h *HistoryHandler) Diff(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	sha := request.PathParam(r, "sha")
	format := r.URL.Query().Get("format")
	if format == "" {
		format = "numstat"
	}
	diff, err := h.svc.CommitDiff(r.Context(), projectID, sha, format)
	if err != nil {
		h.writeRepoError(w, r, err)
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"sha": diff.SHA, "format": format, "stats": diff.Stats, "patch": diff.Patch,
	})
}

// Revert handles POST /api/v1/projects/{id}/commits/{sha}/revert.
func (h *HistoryHandler) Revert(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	sha := request.PathParam(r, "sha")
	var body struct {
		Message string `json:"message"`
	}
	_ = request.DecodeJSON(w, r, &body, h.cfg.MaxBodyBytes) // message optional
	commit, err := h.svc.RevertCommit(r.Context(), projectID, sha, body.Message)
	if err != nil {
		h.writeRepoError(w, r, err)
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"commit": commit})
}

func (h *HistoryHandler) writeRepoError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, project.ErrNotFound):
		response.WriteError(w, r, http.StatusNotFound, "commit_not_found", "commit not found")
	case errors.Is(err, project.ErrConflict):
		response.WriteError(w, r, http.StatusConflict, "revert_conflict", "revert conflicts with current content")
	case errors.Is(err, project.ErrArchived):
		response.WriteError(w, r, http.StatusGone, "project_archived", "project is archived")
	case errors.Is(err, project.ErrInvalid):
		response.WriteError(w, r, http.StatusBadRequest, "invalid_path", "invalid path")
	default:
		h.log.Error("history operation failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "history operation failed")
	}
}
