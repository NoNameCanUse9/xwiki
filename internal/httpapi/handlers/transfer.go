package handlers

import (
	"errors"
	"log/slog"
	"net/http"

	"agentdocs/internal/agent"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

// TransferHandler serves import/export endpoints.
type TransferHandler struct {
	cfg      *config.Config
	svc      *project.Service
	agentSvc *agent.Service
	log      *slog.Logger
}

func NewTransferHandler(cfg *config.Config, svc *project.Service, agentSvc *agent.Service, log *slog.Logger) *TransferHandler {
	return &TransferHandler{cfg: cfg, svc: svc, agentSvc: agentSvc, log: log}
}

// ExportZip handles GET /api/v1/projects/{id}/export.zip.
func (h *TransferHandler) ExportZip(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	data, err := h.svc.ExportZip(r.Context(), projectID)
	if err != nil {
		h.writeError(w, r, err)
		return
	}
	w.Header().Set("Content-Type", "application/zip")
	w.Header().Set("Content-Disposition", `attachment; filename="project.zip"`)
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(data)
}

// ExportBundle handles GET /api/v1/projects/{id}/export.bundle.
func (h *TransferHandler) ExportBundle(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	data, err := h.svc.ExportBundle(r.Context(), projectID)
	if err != nil {
		h.writeError(w, r, err)
		return
	}
	w.Header().Set("Content-Type", "application/octet-stream")
	w.Header().Set("Content-Disposition", `attachment; filename="project.bundle"`)
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write(data)
}

// Import handles POST /api/v1/projects/{id}/import.
func (h *TransferHandler) Import(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	var input project.ImportZipInput
	if err := request.DecodeJSON(w, r, &input, h.cfg.MaxBodyBytes*4); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_import", "invalid import body")
		return
	}
	res, err := h.svc.ImportZip(r.Context(), projectID, input)
	if err != nil {
		h.writeError(w, r, err)
		return
	}
	_ = h.agentSvc.Audit(r.Context(), middleware.ActorType(r), middleware.ActorID(r),
		projectID, "import", "", input.Message, request.RequestID(r))
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"commit": res.Commit, "revision": res.Revision, "imported": res.Imported,
	})
}

// ImportBundle handles POST /api/v1/import/bundle?name=...
func (h *TransferHandler) ImportBundle(w http.ResponseWriter, r *http.Request) {
	name := r.URL.Query().Get("name")
	if err := r.ParseMultipartForm(256 << 20); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_upload", "multipart body required")
		return
	}
	file, _, err := r.FormFile("file")
	if err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_upload", "file field required")
		return
	}
	defer file.Close()
	buf := make([]byte, 0, 1<<20)
	chunk := make([]byte, 1<<20)
	for {
		n, err := file.Read(chunk)
		buf = append(buf, chunk[:n]...)
		if err != nil {
			break
		}
		if len(buf) > 256<<20 {
			response.WriteError(w, r, http.StatusRequestEntityTooLarge, "bundle_too_large", "bundle exceeds 256 MiB")
			return
		}
	}
	res, err := h.svc.ImportBundle(r.Context(), project.ImportBundleInput{Name: name, Bundle: buf})
	if err != nil {
		h.writeError(w, r, err)
		return
	}
	_ = h.agentSvc.Audit(r.Context(), "user", middleware.ActorID(r), res.Project.ID,
		"import.bundle", "", name, request.RequestID(r))
	response.WriteJSON(w, http.StatusCreated, map[string]any{
		"project": res.Project, "commits": res.Commits,
	})
}

func (h *TransferHandler) writeError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, project.ErrNotFound):
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
	case errors.Is(err, project.ErrConflict):
		response.WriteError(w, r, http.StatusConflict, "revision_conflict", "base revision is stale")
	case errors.Is(err, project.ErrInvalid):
		response.WriteError(w, r, http.StatusBadRequest, "invalid_import", err.Error())
	case errors.Is(err, project.ErrArchived):
		response.WriteError(w, r, http.StatusGone, "project_archived", "project is archived")
	default:
		h.log.Error("transfer failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "transfer failed")
	}
}
