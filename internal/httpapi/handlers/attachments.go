package handlers

import (
	"errors"
	"log/slog"
	"mime"
	"net/http"
	"path/filepath"
	"strconv"

	"agentdocs/internal/agent"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

// AttachmentHandler serves binary attachment downloads backed directly by Git.
type AttachmentHandler struct {
	cfg      *config.Config
	svc      *project.Service
	agentSvc *agent.Service
	log      *slog.Logger
}

func NewAttachmentHandler(cfg *config.Config, svc *project.Service, agentSvc *agent.Service, log *slog.Logger) *AttachmentHandler {
	return &AttachmentHandler{cfg: cfg, svc: svc, agentSvc: agentSvc, log: log}
}

// Download handles GET /api/v1/projects/{id}/attachments/{path:*}: streams the
// raw attachment bytes with a Content-Type derived from the file extension.
func (h *AttachmentHandler) Download(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	filePath := request.PathParam(r, "*")
	if !validateDocPath(filePath) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_doc_path", "invalid attachment path")
		return
	}
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
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
	branch, err := repo.DefaultBranch(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve branch")
		return
	}
	content, err := repo.ReadBlob(r.Context(), branch, filePath)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "attachment_not_found", "attachment not found")
		return
	}
	contentType := mime.TypeByExtension(filepath.Ext(filePath))
	if contentType == "" {
		contentType = "application/octet-stream"
	}
	w.Header().Set("Content-Type", contentType)
	w.Header().Set("Content-Disposition", `inline; filename="`+filepath.Base(filePath)+`"`)
	w.Header().Set("Content-Length", strconv.Itoa(len(content)))
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(content)
}
