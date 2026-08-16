package handlers

import (
	"errors"
	"log/slog"
	"net/http"
	"strconv"

	"xwiki/internal/agent"
	"xwiki/internal/config"
	"xwiki/internal/httpapi/middleware"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
)

// TokenHandler manages agent tokens (session-authenticated only).
type TokenHandler struct {
	cfg *config.Config
	svc *agent.Service
	log *slog.Logger
}

func NewTokenHandler(cfg *config.Config, svc *agent.Service, log *slog.Logger) *TokenHandler {
	return &TokenHandler{cfg: cfg, svc: svc, log: log}
}

type createTokenRequest struct {
	Name       string   `json:"name"`
	Scope      string   `json:"scope"`
	ProjectIDs []string `json:"project_ids"`
}

// Create handles POST /api/v1/tokens.
func (h *TokenHandler) Create(w http.ResponseWriter, r *http.Request) {
	var req createTokenRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	created, err := h.svc.Create(r.Context(), agent.CreateInput{
		Name: req.Name, Scope: req.Scope, ProjectIDs: req.ProjectIDs,
	})
	if err != nil {
		if errors.Is(err, agent.ErrInvalid) {
			response.WriteError(w, r, http.StatusBadRequest, "invalid_token_input",
				"name/scope required, scope read|write, at least one project")
			return
		}
		h.log.Error("create token failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create token")
		return
	}
	_ = h.svc.Audit(r.Context(), "user", middleware.ActorID(r), "", "token.create", "", created.Token.Name, request.RequestID(r))
	response.WriteJSON(w, http.StatusCreated, map[string]any{
		"token":  created.Token,
		"secret": created.Secret,
	})
}

// List handles GET /api/v1/tokens.
func (h *TokenHandler) List(w http.ResponseWriter, r *http.Request) {
	tokens, err := h.svc.List(r.Context())
	if err != nil {
		h.log.Error("list tokens failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not list tokens")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"tokens": tokens})
}

// Revoke handles DELETE /api/v1/tokens/{id}.
func (h *TokenHandler) Revoke(w http.ResponseWriter, r *http.Request) {
	tokenID := request.PathParam(r, "id")
	if err := h.svc.Revoke(r.Context(), tokenID); err != nil {
		if errors.Is(err, agent.ErrNotFound) {
			response.WriteError(w, r, http.StatusNotFound, "token_not_found", "token not found")
			return
		}
		h.log.Error("revoke token failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not revoke token")
		return
	}
	_ = h.svc.Audit(r.Context(), "user", middleware.ActorID(r), "", "token.revoke", "", tokenID, request.RequestID(r))
	response.WriteJSON(w, http.StatusOK, map[string]any{"ok": true})
}

// Audit handles GET /api/v1/projects/{id}/audit (session only).
// Pagination mirrors the commits endpoint: limit (default 20, max 100),
// offset, and a has_more flag.
func (h *TokenHandler) Audit(w http.ResponseWriter, r *http.Request) {
	if !sessionOnly(w, r) {
		return
	}
	projectID := request.PathParam(r, "id")
	limit, _ := strconv.Atoi(r.URL.Query().Get("limit"))
	offset, _ := strconv.Atoi(r.URL.Query().Get("offset"))
	entries, hasMore, err := h.svc.StoreRecent(r.Context(), projectID, limit, offset)
	if err != nil {
		h.log.Error("audit failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not read audit log")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"entries": entries, "has_more": hasMore})
}
