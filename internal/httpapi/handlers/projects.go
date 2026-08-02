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

// ProjectHandler serves the /api/v1/projects endpoints.
type ProjectHandler struct {
	cfg *config.Config
	svc *project.Service
	log *slog.Logger
}

func NewProjectHandler(cfg *config.Config, svc *project.Service, log *slog.Logger) *ProjectHandler {
	return &ProjectHandler{cfg: cfg, svc: svc, log: log}
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
