package handlers

import (
	"bytes"
	"encoding/base64"
	"errors"
	"fmt"
	"html"
	"io"
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

// viewPageTemplate wraps server-rendered markdown into a minimal, readable
// HTML page for agents and crawlers that fetch the docs URL directly
// (no JavaScript). Browsers keep the interactive SPA instead.
const viewPageTemplate = `<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>__TITLE__</title>
<style>
body{margin:0;background:#fff;color:#1f2328;font:16px/1.7 -apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif}
main{max-width:760px;margin:0 auto;padding:40px 24px 80px}
h1,h2,h3,h4{line-height:1.35;margin:1.2em 0 .5em}
h1{font-size:1.75rem;padding-bottom:.35em;border-bottom:1px solid #d8dee4}
h2{font-size:1.375rem;padding-bottom:.3em;border-bottom:1px solid #d8dee4}
h3{font-size:1.125rem}
p{margin:.9em 0}
a{color:#0969da;text-decoration:none}a:hover{text-decoration:underline}
code{background:#f6f8fa;border-radius:4px;padding:.15em .35em;font-size:.9em}
pre{background:#f6f8fa;border-radius:8px;padding:14px 16px;overflow-x:auto}
pre code{background:none;padding:0}
blockquote{border-left:4px solid #d8dee4;margin:1em 0;padding:.2em 1em;color:#57606a}
table{border-collapse:collapse;margin:1em 0}
th,td{border:1px solid #d8dee4;padding:6px 12px}
th{background:#f6f8fa}
ul,ol{padding-left:1.6em}
hr{border:none;border-top:1px solid #d8dee4;margin:1.5em 0}
img{max-width:100%}
.admonition{border-left:4px solid #d8dee4;border-radius:6px;padding:.6em 1em;margin:1em 0;background:#f6f8fa}
.admonition-title{font-weight:600}
</style>
</head>
<body><main>__BODY__</main></body>
</html>`

// ServeView renders a document as a plain HTML page for non-browser
// clients (agents, curl, crawlers) hitting the docs URL directly. Browsers
// get the interactive SPA via the router. Public read-only: no auth, so a
// shared link "just works" — documents are already readable via the API.
func (h *DocsHandler) ServeView(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	filePath := request.PathParam(r, "*")
	if filePath == "" {
		filePath = "README.md"
	}
	if !validateDocPath(filePath) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_doc_path", "invalid document path")
		return
	}
	repo, err := h.repoFor(r, projectID)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		return
	}
	branch, err := repo.DefaultBranch(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve branch")
		return
	}
	content, err := repo.ReadBlobAt(r.Context(), branch, filePath)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "doc_not_found", "document not found")
		return
	}
	if len(content) > maxDocBlobBytes {
		response.WriteError(w, r, http.StatusRequestEntityTooLarge, "doc_too_large", "document exceeds size limit")
		return
	}
	var buf bytes.Buffer
	if err := h.markdown.Convert(rewriteWikiLinks(content, projectID), &buf); err != nil {
		h.log.Error("view render failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not render document")
		return
	}
	title := filePath
	if i := strings.LastIndex(title, "/"); i >= 0 {
		title = title[i+1:]
	}
	title = strings.TrimSuffix(title, ".md")
	page := strings.ReplaceAll(viewPageTemplate, "__TITLE__", html.EscapeString(title))
	page = strings.ReplaceAll(page, "__BODY__", buf.String())
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	_, _ = io.WriteString(w, page)
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
	for _, candidate := range []string{"docs/index.md", "docs/README.md", "README.md"} {
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
