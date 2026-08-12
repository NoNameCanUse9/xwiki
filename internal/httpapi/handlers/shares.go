package handlers

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"log/slog"
	"net/http"
	"time"

	"xwiki/internal/httpapi/middleware"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
	"xwiki/internal/project"
)

// ShareHandler creates per-page share links and serves them publicly.
// A share pins a single document (project + path); GET /share/{token}
// renders that page as standalone HTML with no authentication, so the
// link can be handed out directly (to humans or agents).
type ShareHandler struct {
	db        *sql.DB
	docs      *DocsHandler
	projectSvc *project.Service
	log       *slog.Logger
}

// NewShareHandler builds the share handler.
func NewShareHandler(db *sql.DB, docs *DocsHandler, projectSvc *project.Service, log *slog.Logger) *ShareHandler {
	return &ShareHandler{db: db, docs: docs, projectSvc: projectSvc, log: log}
}

// newShareToken returns a URL-safe random token.
func newShareToken() (string, error) {
	buf := make([]byte, 16)
	if _, err := rand.Read(buf); err != nil {
		return "", err
	}
	return hex.EncodeToString(buf), nil
}

type shareRow struct {
	Token     string
	ProjectID string
	Path      string
}

// Create handles POST /api/v1/shares — session users share the current page.
func (h *ShareHandler) Create(w http.ResponseWriter, r *http.Request) {
	var req struct {
		Path string `json:"path"`
	}
	if err := request.DecodeJSON(w, r, &req, 1<<20); err != nil || req.Path == "" || !validateDocPath(req.Path) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_share_input", "a valid document path is required")
		return
	}
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.docs.agentSvc, w, r, projectID) {
		return
	}
	// The page must exist (project not found -> 404 for the caller).
	if _, err := h.projectSvc.OpenRepo(r.Context(), projectID); err != nil {
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		return
	}

	// Reuse an existing share for the same page so re-sharing is idempotent.
	var token string
	err := h.db.QueryRowContext(r.Context(),
		`SELECT token FROM shares WHERE project_id = ? AND path = ?`,
		projectID, req.Path).Scan(&token)
	if err == nil {
		response.WriteJSON(w, http.StatusOK, map[string]any{
			"token": token,
			"url":   "/share/" + token,
		})
		return
	}
	if err != sql.ErrNoRows {
		h.log.Error("share lookup failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create share")
		return
	}

	token, err = newShareToken()
	if err != nil {
		h.log.Error("share token generation failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create share")
		return
	}
	actor := middleware.ActorID(r)
	if actor == "" {
		actor = "unknown"
	}
	if _, err := h.db.ExecContext(r.Context(),
		`INSERT INTO shares (token, project_id, path, created_by, created_at) VALUES (?, ?, ?, ?, ?)`,
		token, projectID, req.Path, actor, time.Now().UnixMilli()); err != nil {
		h.log.Error("share insert failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create share")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{
		"token": token,
		"url":   "/share/" + token,
	})
}

// View handles GET /share/{token} — public standalone HTML page.
func (h *ShareHandler) View(w http.ResponseWriter, r *http.Request) {
	token := request.PathParam(r, "token")
	var row shareRow
	err := h.db.QueryRowContext(r.Context(),
		`SELECT token, project_id, path FROM shares WHERE token = ?`, token).
		Scan(&row.Token, &row.ProjectID, &row.Path)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "share_not_found", "share not found")
		return
	}
	page, err := h.docs.renderDocHTML(r, row.ProjectID, row.Path)
	if err != nil {
		h.docs.writeViewError(w, r, err)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte(page))
}
