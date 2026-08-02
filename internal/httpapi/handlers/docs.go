package handlers

import (
	"bytes"
	"errors"
	"log/slog"
	"net/http"
	"path"
	"strings"

	"github.com/yuin/goldmark"
	"github.com/yuin/goldmark/extension"

	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

// maxDocBlobBytes caps the size of a single readable document.
const maxDocBlobBytes = 2 << 20 // 2 MiB

// DocsHandler serves read-only document endpoints backed directly by Git.
type DocsHandler struct {
	cfg     *config.Config
	svc     *project.Service
	log     *slog.Logger
	markdown goldmark.Markdown
}

func NewDocsHandler(cfg *config.Config, svc *project.Service, log *slog.Logger) *DocsHandler {
	return &DocsHandler{
		cfg:      cfg,
		svc:      svc,
		log:      log,
		markdown: goldmark.New(goldmark.WithExtensions(extension.GFM)),
	}
}

// validateDocPath rejects traversal, absolute and empty paths.
func validateDocPath(p string) bool {
	if p == "" || strings.HasPrefix(p, "/") || strings.Contains(p, "\\") {
		return false
	}
	clean := path.Clean(p)
	return clean != ".." && !strings.HasPrefix(clean, "../")
}

func (h *DocsHandler) repoFor(r *http.Request, projectID string) (*project.Repo, error) {
	repo, err := h.svc.OpenRepo(r.Context(), projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			return nil, err
		}
		h.log.Error("open repo failed", "error", err, "request_id", request.RequestID(r))
		return nil, err
	}
	return repo, nil
}

// Tree handles GET /api/v1/projects/{id}/docs/tree?path=...
func (h *DocsHandler) Tree(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	dirPath := r.URL.Query().Get("path")
	if dirPath != "" && !validateDocPath(dirPath) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_doc_path", "invalid directory path")
		return
	}
	repo, err := h.repoFor(r, projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not read repository")
		return
	}
	branch, err := repo.DefaultBranch(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve branch")
		return
	}
	if _, err := repo.ResolveTree(r.Context(), branch, dirPath); err != nil {
		response.WriteError(w, r, http.StatusNotFound, "doc_not_found", "directory not found")
		return
	}
	entries, err := repo.ListTree(r.Context(), branch, dirPath)
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not list directory")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"path": dirPath,
		"tree": entries,
	})
}

// Page handles GET /api/v1/projects/{id}/docs/pages/{path:*}.
func (h *DocsHandler) Page(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	filePath := request.PathParam(r, "*")
	format := r.URL.Query().Get("format")
	if format == "" {
		format = "raw"
	}
	if format != "raw" && format != "html" {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_format", "format must be raw or html")
		return
	}
	if !validateDocPath(filePath) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_doc_path", "invalid document path")
		return
	}
	repo, err := h.repoFor(r, projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
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
		response.WriteError(w, r, http.StatusNotFound, "doc_not_found", "document not found")
		return
	}
	if len(content) > maxDocBlobBytes {
		response.WriteError(w, r, http.StatusRequestEntityTooLarge, "doc_too_large", "document exceeds size limit")
		return
	}
	resp := map[string]any{"path": filePath, "format": format}
	if format == "html" {
		var buf bytes.Buffer
		if err := h.markdown.Convert(content, &buf); err != nil {
			h.log.Error("markdown render failed", "error", err, "request_id", request.RequestID(r))
			response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not render document")
			return
		}
		resp["content"] = buf.String()
	} else {
		resp["content"] = string(content)
	}
	response.WriteJSON(w, http.StatusOK, resp)
}

// Home handles GET /api/v1/projects/{id}/docs/home — renders README.md,
// falling back to docs/README.md.
func (h *DocsHandler) Home(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	repo, err := h.repoFor(r, projectID)
	if err != nil {
		if errors.Is(err, project.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
			return
		}
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not read repository")
		return
	}
	branch, err := repo.DefaultBranch(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve branch")
		return
	}
	for _, candidate := range []string{"README.md", "docs/README.md"} {
		content, err := repo.ReadBlob(r.Context(), branch, candidate)
		if err != nil {
			continue
		}
		if len(content) > maxDocBlobBytes {
			continue
		}
		var buf bytes.Buffer
		if err := h.markdown.Convert(content, &buf); err != nil {
			h.log.Error("markdown render failed", "error", err, "request_id", request.RequestID(r))
			response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not render document")
			return
		}
		response.WriteJSON(w, http.StatusOK, map[string]any{
			"path": candidate, "format": "html", "content": buf.String(),
		})
		return
	}
	response.WriteError(w, r, http.StatusNotFound, "doc_not_found", "project has no README")
}
