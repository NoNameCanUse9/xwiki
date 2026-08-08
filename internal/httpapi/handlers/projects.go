package handlers

import (
	"errors"
	"io"
	"log/slog"
	"net/http"

	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
	"agentdocs/internal/search"
)

// ProjectHandler serves the /api/v1/projects endpoints.
type ProjectHandler struct {
	cfg      *config.Config
	svc      *project.Service
	searchSvc *search.Service
	log      *slog.Logger
}

func NewProjectHandler(cfg *config.Config, svc *project.Service, searchSvc *search.Service, log *slog.Logger) *ProjectHandler {
	return &ProjectHandler{cfg: cfg, svc: svc, searchSvc: searchSvc, log: log}
}

type createProjectRequest struct {
	Name        string `json:"name"`
	Description string `json:"description"`
}

// Create handles POST /api/v1/projects.
func (h *ProjectHandler) Create(w http.ResponseWriter, r *http.Request) {
	var req createProjectRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	p, err := h.svc.Create(r.Context(), project.CreateInput{
		Name:        req.Name,
		Description: req.Description,
	})
	if err != nil {
		switch {
		case errors.Is(err, project.ErrInvalid):
			response.WriteError(w, r, http.StatusBadRequest, "invalid_project_name",
				"project name must be 1-64 lowercase letters, digits and single hyphens")
		case errors.Is(err, project.ErrConflict):
			response.WriteError(w, r, http.StatusConflict, "project_name_conflict",
				"a project with this name already exists")
		default:
			h.log.Error("create project failed", "error", err, "request_id", request.RequestID(r))
			response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create project")
		}
		return
	}
	response.WriteJSON(w, http.StatusCreated, map[string]any{"project": p})
}

// List handles GET /api/v1/projects.
func (h *ProjectHandler) List(w http.ResponseWriter, r *http.Request) {
	projects, err := h.svc.List(r.Context())
	if err != nil {
		h.log.Error("list projects failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not list projects")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"projects": projects})
}

// Get handles GET /api/v1/projects/{id}.
func (h *ProjectHandler) Get(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	p, err := h.svc.Get(r.Context(), projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		h.log.Error("get project failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not load project")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"project": p})
}

// ImportFolder handles POST /api/v1/projects/import-folder (multipart: files + name).
func (h *ProjectHandler) ImportFolder(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseMultipartForm(256 << 20); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_upload", "multipart body required")
		return
	}
	name := r.FormValue("name")
	description := r.FormValue("description")

	parsedFiles, ok := r.MultipartForm.File["files"]
	if !ok || len(parsedFiles) == 0 {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_upload", "at least one file required")
		return
	}
	// Go's multipart parser strips directory parts from filename=, so the
	// frontend sends the real relative path in a parallel "paths" field,
	// index-aligned with files.
	paths := r.MultipartForm.Value["paths"]

	var files []project.UploadedFile
	for i, fh := range parsedFiles {
		f, err := fh.Open()
		if err != nil {
			response.WriteError(w, r, http.StatusBadRequest, "invalid_upload", "cannot read uploaded file")
			return
		}
		defer f.Close()
		buf, err := io.ReadAll(f)
		if err != nil {
			response.WriteError(w, r, http.StatusBadRequest, "invalid_upload", "cannot read uploaded file")
			return
		}
		if len(buf) > project.MaxImportFileBytes {
			response.WriteError(w, r, http.StatusRequestEntityTooLarge, "file_too_large", "file exceeds size limit")
			return
		}
		path := fh.Filename
		if i < len(paths) && paths[i] != "" {
			path = paths[i]
		}
		files = append(files, project.UploadedFile{
			Path:    path,
			Content: buf,
		})
	}

	res, err := h.svc.ImportFolder(r.Context(), project.ImportFolderInput{
		Name:        name,
		Description: description,
		Files:       files,
	})
	if err != nil {
		switch {
		case errors.Is(err, project.ErrInvalid):
			response.WriteError(w, r, http.StatusBadRequest, "invalid_project_name",
				"project name must be 1-64 lowercase letters, digits and single hyphens")
		case errors.Is(err, project.ErrConflict):
			response.WriteError(w, r, http.StatusConflict, "project_name_conflict",
				"a project with this name already exists")
		default:
			h.log.Error("import folder failed", "error", err, "request_id", request.RequestID(r))
			response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not import folder")
		}
		return
	}
	// Trigger incremental reindex so the new project is searchable immediately.
	if _, err := h.searchSvc.ReindexProject(r.Context(), res.Project.ID); err != nil {
		h.log.Warn("reindex after import failed", "error", err, "project_id", res.Project.ID)
	}
	response.WriteJSON(w, http.StatusCreated, map[string]any{"project": res.Project, "commits": res.Commits})
}

// Unarchive handles POST /api/v1/projects/{id}/unarchive.
func (h *ProjectHandler) Unarchive(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	p, err := h.svc.Unarchive(r.Context(), projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		h.log.Error("unarchive project failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not unarchive project")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"project": p})
}

// Archive handles POST /api/v1/projects/{id}/archive.
func (h *ProjectHandler) Archive(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	p, err := h.svc.Archive(r.Context(), projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		h.log.Error("archive project failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not archive project")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"project": p})
}

type renameProjectRequest struct {
	Name string `json:"name"`
}

// Rename handles PATCH /api/v1/projects/{id}. It updates the project name
// and refreshes the repository README headline.
func (h *ProjectHandler) Rename(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	var req renameProjectRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	p, err := h.svc.Rename(r.Context(), projectID, project.RenameInput{Name: req.Name})
	if err != nil {
		switch {
		case errors.Is(err, project.ErrNotFound):
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		case errors.Is(err, project.ErrInvalid):
			response.WriteError(w, r, http.StatusBadRequest, "invalid_project_name",
				"project name must be 1-64 lowercase letters, digits and single hyphens")
		case errors.Is(err, project.ErrConflict):
			response.WriteError(w, r, http.StatusConflict, "project_name_conflict",
				"a project with this name already exists")
		default:
			h.log.Error("rename project failed", "error", err, "request_id", request.RequestID(r))
			response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not rename project")
		}
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"project": p})
}

// Delete handles DELETE /api/v1/projects/{id}. The default removes the
// project completely (metadata + repository).
func (h *ProjectHandler) Delete(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if err := h.svc.Delete(r.Context(), projectID); err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		h.log.Error("delete project failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not delete project")
		return
	}
	// Remove the project from the search index so it stops matching queries.
	if err := h.searchSvc.DeleteProject(r.Context(), projectID); err != nil {
		h.log.Warn("delete project index failed", "error", err, "project_id", projectID)
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"deleted": true})
}

type purgePathsRequest struct {
	Paths   []string `json:"paths"`
	Message string   `json:"message,omitempty"`
}

// Purge handles POST /api/v1/projects/{id}/purge. It rewrites history to
// remove the given paths completely (hard delete, irreversible).
func (h *ProjectHandler) Purge(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	var req purgePathsRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	if len(req.Paths) == 0 {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "paths required")
		return
	}
	if err := h.svc.Purge(r.Context(), projectID, req.Paths, req.Message); err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		h.log.Error("purge paths failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not purge paths")
		return
	}
	if _, err := h.searchSvc.ReindexProject(r.Context(), projectID); err != nil {
		h.log.Warn("reindex after purge failed", "error", err, "project_id", projectID)
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"purged": true})
}
