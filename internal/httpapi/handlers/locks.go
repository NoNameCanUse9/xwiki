package handlers

import (
	"context"
	"database/sql"
	"errors"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"xwiki/internal/agent"
	"xwiki/internal/httpapi/middleware"
	"xwiki/internal/httpapi/request"
	"xwiki/internal/httpapi/response"
)

// Exclusive per-page edit lock: only one user may hold the lock on a given
// page at a time. Locks expire lazily — every operation purges rows whose
// lease has elapsed, so a crashed or offline editor frees the page after
// lockLease without anyone having to intervene.
const (
	lockLease = 5 * time.Minute
	lockCode  = "page_locked"
)

// LockHandler serves exclusive per-page edit locks.
type LockHandler struct {
	db       *sql.DB
	agentSvc *agent.Service
	log      *slog.Logger
}

// NewLockHandler builds the edit-lock handler.
func NewLockHandler(db *sql.DB, agentSvc *agent.Service, log *slog.Logger) *LockHandler {
	return &LockHandler{db: db, agentSvc: agentSvc, log: log}
}

// LockInfo is the wire representation of an edit lock.
type LockInfo struct {
	Path       string `json:"path"`
	UserID     string `json:"user_id"`
	Username   string `json:"username"`
	AcquiredAt string `json:"acquired_at"`
	ExpiresAt  string `json:"expires_at"`
}

func validLockPath(p string) bool {
	return p != "" && !strings.HasPrefix(p, "/") && !strings.Contains(p, "..")
}

// lockOwner resolves the acting identity: the session user, or a stable
// "agent" identity when the request came through an agent bearer token.
func lockOwner(r *http.Request) (userID, username string) {
	if u := middleware.UserFrom(r); u != nil {
		return u.ID, u.Username
	}
	return "agent", "agent"
}

func (h *LockHandler) purgeExpired(ctx context.Context) {
	_, _ = h.db.ExecContext(ctx,
		`DELETE FROM edit_locks WHERE expires_at < ?`, time.Now().UnixMilli())
}

func (h *LockHandler) get(ctx context.Context, projectID, path string) (*LockInfo, error) {
	row := h.db.QueryRowContext(ctx,
		`SELECT user_id, username, acquired_at, expires_at
		   FROM edit_locks WHERE project_id = ? AND path = ?`, projectID, path)
	var userID, username string
	var acquired, expires int64
	if err := row.Scan(&userID, &username, &acquired, &expires); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, nil
		}
		return nil, err
	}
	return &LockInfo{
		Path:       path,
		UserID:     userID,
		Username:   username,
		AcquiredAt: time.UnixMilli(acquired).UTC().Format(time.RFC3339),
		ExpiresAt:  time.UnixMilli(expires).UTC().Format(time.RFC3339),
	}, nil
}

func (h *LockHandler) holderOnly(ctx context.Context, r *http.Request, projectID, path string) (userID string, err error) {
	userID, _ = lockOwner(r)
	existing, err := h.get(ctx, projectID, path)
	if err != nil {
		return "", err
	}
	if existing == nil {
		return userID, errLockNotFound
	}
	if existing.UserID != userID {
		return userID, errLockNotHolder
	}
	return userID, nil
}

var (
	errLockNotFound  = errors.New("lock not found")
	errLockNotHolder = errors.New("lock held by another user")
)

// Status handles GET /api/v1/projects/{id}/locks?path=…
func (h *LockHandler) Status(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentRead(h.agentSvc, w, r, projectID) {
		return
	}
	path := r.URL.Query().Get("path")
	if !validLockPath(path) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_lock_path", "a valid path query parameter is required")
		return
	}
	h.purgeExpired(r.Context())
	lock, err := h.get(r.Context(), projectID, path)
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not read lock state")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"lock": lock})
}

// Acquire handles POST /api/v1/projects/{id}/locks?path=…
// 409 page_locked when another user holds the lock.
func (h *LockHandler) Acquire(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentWrite(h.agentSvc, w, r, projectID) {
		return
	}
	path := r.URL.Query().Get("path")
	if !validLockPath(path) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_lock_path", "a valid path query parameter is required")
		return
	}
	ctx := r.Context()
	h.purgeExpired(ctx)

	existing, err := h.get(ctx, projectID, path)
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not read lock state")
		return
	}
	if existing != nil {
		response.WriteErrorWith(w, r, http.StatusConflict, lockCode,
			"该页面正被 "+existing.Username+" 编辑", map[string]any{"lock": existing})
		return
	}

	userID, username := lockOwner(r)
	now := time.Now().UnixMilli()
	if _, err := h.db.ExecContext(ctx,
		`INSERT INTO edit_locks (project_id, path, user_id, username, acquired_at, expires_at)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		projectID, path, userID, username, now, now+lockLease.Milliseconds()); err != nil {
		// Primary-key race: someone else acquired in the meantime.
		existing, getErr := h.get(ctx, projectID, path)
		if getErr == nil && existing != nil {
			response.WriteErrorWith(w, r, http.StatusConflict, lockCode,
				"该页面正被 "+existing.Username+" 编辑", map[string]any{"lock": existing})
			return
		}
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not acquire lock")
		return
	}

	info, _ := h.get(ctx, projectID, path)
	response.WriteJSON(w, http.StatusOK, map[string]any{"lock": info})
}

// Release handles DELETE /api/v1/projects/{id}/locks?path=…
// Only the holder may release; releasing a missing/expired lock is a no-op.
func (h *LockHandler) Release(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentWrite(h.agentSvc, w, r, projectID) {
		return
	}
	path := r.URL.Query().Get("path")
	if !validLockPath(path) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_lock_path", "a valid path query parameter is required")
		return
	}
	ctx := r.Context()
	h.purgeExpired(ctx)

	userID, err := h.holderOnly(ctx, r, projectID, path)
	switch {
	case errors.Is(err, errLockNotFound):
		response.WriteJSON(w, http.StatusOK, map[string]any{"released": false})
		return
	case errors.Is(err, errLockNotHolder):
		response.WriteError(w, r, http.StatusForbidden, "not_lock_holder", "lock is held by another user")
		return
	case err != nil:
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not release lock")
		return
	}
	if _, err := h.db.ExecContext(ctx,
		`DELETE FROM edit_locks WHERE project_id = ? AND path = ? AND user_id = ?`,
		projectID, path, userID); err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not release lock")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"released": true})
}

// Heartbeat handles POST /api/v1/projects/{id}/locks/heartbeat?path=…
// The holder renews the lease; a lost/expired lock answers 409 lock_lost so
// the editor can surface the conflict instead of editing a dead lock.
func (h *LockHandler) Heartbeat(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !authorizeAgentWrite(h.agentSvc, w, r, projectID) {
		return
	}
	path := r.URL.Query().Get("path")
	if !validLockPath(path) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_lock_path", "a valid path query parameter is required")
		return
	}
	ctx := r.Context()
	h.purgeExpired(ctx)

	userID, err := h.holderOnly(ctx, r, projectID, path)
	switch {
	case errors.Is(err, errLockNotFound):
		response.WriteError(w, r, http.StatusConflict, "lock_lost", "the edit lock no longer exists")
		return
	case errors.Is(err, errLockNotHolder):
		response.WriteError(w, r, http.StatusForbidden, "not_lock_holder", "lock is held by another user")
		return
	case err != nil:
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not renew lock")
		return
	}

	now := time.Now().UnixMilli()
	if _, err := h.db.ExecContext(ctx,
		`UPDATE edit_locks SET expires_at = ? WHERE project_id = ? AND path = ? AND user_id = ?`,
		now+lockLease.Milliseconds(), projectID, path, userID); err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not renew lock")
		return
	}
	lock, _ := h.get(ctx, projectID, path)
	response.WriteJSON(w, http.StatusOK, map[string]any{"lock": lock})
}

// ForceRelease handles POST /api/v1/projects/{id}/locks/force-release?path=…
// Any signed-in user may force a lock open (the holder's uncommitted draft is
// discarded); the confirm dialog on the client spells that out.
func (h *LockHandler) ForceRelease(w http.ResponseWriter, r *http.Request) {
	projectID := request.PathParam(r, "id")
	if !sessionOnly(w, r) {
		return
	}
	path := r.URL.Query().Get("path")
	if !validLockPath(path) {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_lock_path", "a valid path query parameter is required")
		return
	}
	if _, err := h.db.ExecContext(r.Context(),
		`DELETE FROM edit_locks WHERE project_id = ? AND path = ?`,
		projectID, path); err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not force-release lock")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"released": true})
}
