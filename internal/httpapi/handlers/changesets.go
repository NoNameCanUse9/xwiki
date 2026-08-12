package handlers

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
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

// ChangesetHandler serves write endpoints for project documents.
type ChangesetHandler struct {
	cfg       *config.Config
	svc       *project.Service
	agentSvc  *agent.Service
	searchSvc *search.Service
	log       *slog.Logger
}

func NewChangesetHandler(cfg *config.Config, svc *project.Service, agentSvc *agent.Service, searchSvc *search.Service, log *slog.Logger) *ChangesetHandler {
	return &ChangesetHandler{cfg: cfg, svc: svc, agentSvc: agentSvc, searchSvc: searchSvc, log: log}
}

// Revision handles GET /api/v1/projects/{id}/revision.
func (h *ChangesetHandler) Revision(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	repo, err := h.svc.OpenRepo(r.Context(), projectID)
	if err != nil {
		h.writeProjectError(w, r, err)
		return
	}
	rev, err := repo.Revision(r.Context())
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not resolve revision")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"revision": rev})
}

// Apply handles POST /api/v1/projects/{id}/changesets.
func (h *ChangesetHandler) Apply(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")

	// Read the raw body once: hashed for idempotency, then decoded.
	body, err := io.ReadAll(io.LimitReader(r.Body, h.cfg.MaxBodyBytes+1))
	if err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_changeset", "invalid request body")
		return
	}
	var input project.ChangesetInput
	if err := unmarshalStrict(body, &input); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_changeset", "invalid request body")
		return
	}
	if r.URL.Query().Get("dry_run") == "true" {
		input.DryRun = true
	}

	// Agent tokens: scope and project binding.
	if secret := middleware.AgentSecret(r); secret != "" {
		if !authorizeAgentWrite(h.agentSvc, w, r, projectID) {
			return
		}
	}

	key := r.Header.Get("Idempotency-Key")
	name, email := middleware.CommitAuthorIdentity(r)
	run := func() (string, error) {
		res, err := h.svc.ApplyChangeset(r.Context(), projectID, input, project.CommitAuthor{Name: name, Email: email})
		if err != nil {
			return "", err
		}
		return agent.MarshalResult(map[string]any{
			"commit":   res.Commit,
			"revision": res.Revision,
			"preview":  res.Preview,
		})
	}
	runWithError := func() (string, error) {
		s, err := run()
		return s, err
	}
	resultJSON, replayed, err := h.agentSvc.ApplyIdempotent(r.Context(), key, projectID, agent.RequestHash(body), runWithError)
	if err != nil {
		h.writeApplyError(w, r, err)
		return
	}
	if !replayed && !input.DryRun {
		// Audit the write (first execution only).
		first := ""
		if len(input.Changes) > 0 {
			first = input.Changes[0].Path
		}
		_ = h.agentSvc.Audit(r.Context(), middleware.ActorType(r), middleware.ActorID(r),
			projectID, "change", first, input.Message, request.RequestID(r))
		// Incremental reindex; failures only degrade search freshness.
		if h.searchSvc != nil {
			if _, err := h.searchSvc.ReindexProject(r.Context(), projectID); err != nil {
				h.log.Warn("reindex failed", "error", err, "project_id", projectID)
			}
		}
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte(resultJSON))
}

// authorizeAgentRead enforces project binding for agent-token reads.
func authorizeAgentRead(svc *agent.Service, w http.ResponseWriter, r *http.Request, projectID string) bool {
	secret := middleware.AgentSecret(r)
	if secret == "" {
		return true // session user
	}
	if _, err := svc.Authorize(r.Context(), secret, projectID, false); err != nil {
		response.WriteError(w, r, http.StatusForbidden, "agent_forbidden", "token cannot access this project")
		return false
	}
	return true
}

// authorizeAgentWrite enforces scope and project binding for agent writes.
func authorizeAgentWrite(svc *agent.Service, w http.ResponseWriter, r *http.Request, projectID string) bool {
	secret := middleware.AgentSecret(r)
	if secret == "" {
		return true
	}
	if _, err := svc.Authorize(r.Context(), secret, projectID, true); err != nil {
		response.WriteError(w, r, http.StatusForbidden, "agent_forbidden",
			"token lacks write permission")
		return false
	}
	return true
}

func (h *ChangesetHandler) writeApplyError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, agent.ErrIdempotencyConflict):
		response.WriteError(w, r, http.StatusConflict, "idempotency_conflict",
			"idempotency key reused with a different request body")
	case errors.Is(err, project.ErrNotFound):
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
	case errors.Is(err, project.ErrConflict):
		response.WriteError(w, r, http.StatusConflict, "revision_conflict",
			"base revision is stale; reload and retry")
	case errors.Is(err, project.ErrArchived):
		response.WriteError(w, r, http.StatusGone, "project_archived", "project is archived")
	default:
		h.log.Error("apply changeset failed", "error", err, "request_id", request.RequestID(r))
		response.WriteError(w, r, http.StatusBadRequest, "invalid_changeset", err.Error())
	}
}

func (h *ChangesetHandler) writeProjectError(w http.ResponseWriter, r *http.Request, err error) {
	if errors.Is(err, project.ErrNotFound) {
		response.WriteError(w, r, http.StatusNotFound, "project_not_found", "project not found")
		return
	}
	h.log.Error("project operation failed", "error", err, "request_id", request.RequestID(r))
	response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "operation failed")
}

func unmarshalStrict(body []byte, dst any) error {
	dec := json.NewDecoder(bytes.NewReader(body))
	dec.DisallowUnknownFields()
	return dec.Decode(dst)
}
