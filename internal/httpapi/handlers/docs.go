package handlers

import (
	"bytes"
	"encoding/base64"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"path"
	"regexp"
	"strings"

	"github.com/yuin/goldmark"
	"github.com/yuin/goldmark/extension"
	"github.com/yuin/goldmark/parser"
	"github.com/yuin/goldmark/renderer"
	"github.com/yuin/goldmark/util"

	"agentdocs/internal/markdownx"

	"agentdocs/internal/agent"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/project"
)

// maxDocBlobBytes is the readable document size cap (see project.MaxDocBlobBytes).
const maxDocBlobBytes = project.MaxDocBlobBytes

// DocsHandler serves read-only document endpoints backed directly by Git.
type DocsHandler struct {
	cfg      *config.Config
	svc      *project.Service
	agentSvc *agent.Service
	log      *slog.Logger
	markdown goldmark.Markdown
}

func NewDocsHandler(cfg *config.Config, svc *project.Service, agentSvc *agent.Service, log *slog.Logger) *DocsHandler {
	return &DocsHandler{
		cfg:      cfg,
		svc:      svc,
		agentSvc: agentSvc,
		log:      log,
		markdown: goldmark.New(
			goldmark.WithExtensions(extension.GFM),
			goldmark.WithExtensions(extension.Footnote),
			goldmark.WithParserOptions(
				parser.WithBlockParsers(util.Prioritized(markdownx.NewAdmonitionParser(), 50)),
			),
			goldmark.WithRendererOptions(
				renderer.WithNodeRenderers(util.Prioritized(markdownx.NewAdmonitionRenderer(), 50)),
			),
		),
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
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
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
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	filePath := request.PathParam(r, "*")
	format := r.URL.Query().Get("format")
	if format == "" {
		format = "raw"
	}
	if format != "raw" && format != "html" && format != "base64" {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_format", "format must be raw, html or base64")
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
	// Historical version: ?at=<commit sha> reads the file as of that commit.
	rev := branch
	if at := r.URL.Query().Get("at"); at != "" {
		rev = at
	}
	content, err := repo.ReadBlobAt(r.Context(), rev, filePath)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "doc_not_found", "document not found")
		return
	}
	if len(content) > maxDocBlobBytes {
		response.WriteError(w, r, http.StatusRequestEntityTooLarge, "doc_too_large", "document exceeds size limit")
		return
	}
	resp := map[string]any{"path": filePath, "format": format}
	if format == "base64" {
		resp["encoding"] = "base64"
		resp["content"] = base64.StdEncoding.EncodeToString(content)
	} else if format == "html" {
		var buf bytes.Buffer
		if err := h.markdown.Convert(rewriteWikiLinks(content, projectID), &buf); err != nil {
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

// wikiLinkRe matches [[path]] and [[path|label]] wiki links.
var wikiLinkRe = regexp.MustCompile(`\[\[([^\]|]+)(?:\|([^\]]+))?\]\]`)

// rewriteWikiLinks converts [[path|label]] into markdown links pointing at the
// project's docs viewer route, so internal pages are clickable.
func rewriteWikiLinks(content []byte, projectID string) []byte {
	return wikiLinkRe.ReplaceAllFunc(content, func(m []byte) []byte {
		parts := wikiLinkRe.FindSubmatch(m)
		path := string(parts[1])
		label := path
		if len(parts) > 2 && len(parts[2]) > 0 {
			label = string(parts[2])
		}
		return []byte(fmt.Sprintf("[%s](/projects/%s/docs/%s)", label, projectID, path))
	})
}

// Home handles GET /api/v1/projects/{id}/docs/home — renders README.md,
// falling back to docs/README.md.
func (h *DocsHandler) Home(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
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
	for _, candidate := range []string{"README.md", "docs/README.md"} {
		content, err := repo.ReadBlob(r.Context(), branch, candidate)
		if err != nil {
			continue
		}
		if len(content) > maxDocBlobBytes {
			continue
		}
		var buf bytes.Buffer
		if err := h.markdown.Convert(rewriteWikiLinks(content, projectID), &buf); err != nil {
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
