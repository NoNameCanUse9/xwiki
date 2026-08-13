package handlers

import (
	"errors"
	"log/slog"
	"net/http"

	"xwiki/internal/agent"
	"xwiki/internal/config"
	"xwiki/internal/httpapi/middleware"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
	"xwiki/internal/project"
	"xwiki/internal/search"
)

// TransferHandler serves import/export endpoints.
type TransferHandler struct {
	cfg       *config.Config
	svc       *project.Service
	agentSvc  *agent.Service
	searchSvc *search.Service
	log       *slog.Logger
}

func NewTransferHandler(cfg *config.Config, svc *project.Service, agentSvc *agent.Service, searchSvc *search.Service, log *slog.Logger) *TransferHandler {
	return &TransferHandler{cfg: cfg, svc: svc, agentSvc: agentSvc, searchSvc: searchSvc, log: log}
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
	if !authorizeAgentWrite(h.agentSvc, w, r, projectID) {
		return
	}
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
	h.reindex(r, projectID)
	_ = h.agentSvc.Audit(r.Context(), middleware.ActorType(r), middleware.ActorID(r),
		projectID, "import", "", input.Message, request.RequestID(r))
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"commit": res.Commit, "revision": res.Revision, "imported": res.Imported,
	})
}

// ImportRepo handles POST /api/v1/import/repo?name=...&url=...（clone 远程仓库为新项目）。
func (h *TransferHandler) ImportRepo(w http.ResponseWriter, r *http.Request) {
	name := r.URL.Query().Get("name")
	url := r.URL.Query().Get("url")
	if name == "" || url == "" {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_import", "name and url query params required")
		return
	}
	res, err := h.svc.ImportRepo(r.Context(), name, url)
	if err != nil {
		h.writeError(w, r, err)
		return
	}
	if h.searchSvc != nil {
		_, _ = h.searchSvc.ReindexProject(r.Context(), res.Project.ID)
	}
	_ = h.agentSvc.Audit(r.Context(), "user", middleware.ActorID(r), res.Project.ID,
		"import.repo", "", url, request.RequestID(r))
	response.WriteJSON(w, http.StatusCreated, map[string]any{
		"project": res.Project, "commits": res.Commits,
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
	const maxBundleBytes = 256 << 20
	buf := make([]byte, 0, 1<<20)
	chunk := make([]byte, 1<<20)
	for {
		n, err := file.Read(chunk)
		if n > 0 {
			if len(buf)+n > maxBundleBytes {
				response.WriteError(w, r, http.StatusRequestEntityTooLarge, "bundle_too_large", "bundle exceeds 256 MiB")
				return
			}
			buf = append(buf, chunk[:n]...)
		}
		if err != nil {
			break
		}
	}
	res, err := h.svc.ImportBundle(r.Context(), project.ImportBundleInput{Name: name, Bundle: buf})
	if err != nil {
		h.writeError(w, r, err)
		return
	}
	if h.searchSvc != nil {
		_, _ = h.searchSvc.ReindexProject(r.Context(), res.Project.ID)
	}
	_ = h.agentSvc.Audit(r.Context(), "user", middleware.ActorID(r), res.Project.ID,
		"import.bundle", "", name, request.RequestID(r))
	response.WriteJSON(w, http.StatusCreated, map[string]any{
		"project": res.Project, "commits": res.Commits,
	})
}

func (h *TransferHandler) reindex(r *http.Request, projectID string) {
	if _, err := h.searchSvc.ReindexProject(r.Context(), projectID); err != nil {
		h.log.Warn("reindex after import failed", "error", err, "project_id", projectID)
	}
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
