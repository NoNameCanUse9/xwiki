package handlers

import (
	"log/slog"
	"net/http"
	"strings"
	"time"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/platform/id"
	"agentdocs/internal/user"
)

// UserHandler manages accounts (admin only).
type UserHandler struct {
	cfg  *config.Config
	auth *auth.Service
	svc  *user.Store
	log  *slog.Logger
}

func NewUserHandler(cfg *config.Config, authSvc *auth.Service, users *user.Store, log *slog.Logger) *UserHandler {
	return &UserHandler{cfg: cfg, auth: authSvc, svc: users, log: log}
}

type createUserRequest struct {
	Username    string `json:"username"`
	Password    string `json:"password"`
	DisplayName string `json:"display_name"`
	IsAdmin     bool   `json:"is_admin"`
}

// Create handles POST /api/v1/users (admin only).
func (h *UserHandler) Create(w http.ResponseWriter, r *http.Request) {
	var req createUserRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	req.Username = strings.TrimSpace(req.Username)
	if len(req.Username) < 3 || len(req.Username) > 64 {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_username", "username must be 3-64 characters")
		return
	}
	if len(req.Password) < 8 {
		response.WriteError(w, r, http.StatusBadRequest, "invalid_password", "password must be at least 8 characters")
		return
	}
	if _, err := h.svc.GetByUsername(r.Context(), req.Username); err == nil {
		response.WriteError(w, r, http.StatusConflict, "username_conflict", "username already exists")
		return
	}
	hash, err := auth.HashPassword(req.Password)
	if err != nil {
		h.log.Error("hash password failed", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create user")
		return
	}
	now := time.Now().UTC()
	u := &user.User{
		ID: id.New("usr"), Username: req.Username,
		DisplayName:  firstNonEmpty(req.DisplayName, req.Username),
		PasswordHash: hash, IsAdmin: req.IsAdmin,
		CreatedAt: now, UpdatedAt: now,
	}
	if err := h.svc.Create(r.Context(), u); err != nil {
		h.log.Error("create user failed", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not create user")
		return
	}
	response.WriteJSON(w, http.StatusCreated, map[string]any{"user": publicUserView(u)})
}

// List handles GET /api/v1/users (admin only).
func (h *UserHandler) List(w http.ResponseWriter, r *http.Request) {
	users, err := h.svc.List(r.Context())
	if err != nil {
		h.log.Error("list users failed", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not list users")
		return
	}
	view := make([]map[string]any, 0, len(users))
	for _, u := range users {
		view = append(view, publicUserView(u))
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"users": view})
}

// Disable handles POST /api/v1/users/{id}/disable (admin only).
func (h *UserHandler) Disable(w http.ResponseWriter, r *http.Request) {
	h.setDisabled(w, r, true)
}

// Enable handles POST /api/v1/users/{id}/enable (admin only).
func (h *UserHandler) Enable(w http.ResponseWriter, r *http.Request) {
	h.setDisabled(w, r, false)
}

func (h *UserHandler) setDisabled(w http.ResponseWriter, r *http.Request, disabled bool) {
	userID := request.PathParam(r, "id")
	me := middleware.UserFrom(r)
	if me != nil && me.ID == userID {
		response.WriteError(w, r, http.StatusBadRequest, "cannot_disable_self", "cannot disable your own account")
		return
	}
	target, err := h.svc.GetByID(r.Context(), userID)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "user_not_found", "user not found")
		return
	}
	if target.IsAdmin && disabled {
		response.WriteError(w, r, http.StatusBadRequest, "cannot_disable_admin", "admin accounts cannot be disabled")
		return
	}
	if err := h.svc.SetDisabled(r.Context(), userID, disabled, time.Now().UTC()); err != nil {
		h.log.Error("set disabled failed", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not update user")
		return
	}
	updated, err := h.svc.GetByID(r.Context(), userID)
	if err != nil {
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not load user")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"user": publicUserView(updated)})
}

// Delete handles DELETE /api/v1/users/{id} (admin only).
func (h *UserHandler) Delete(w http.ResponseWriter, r *http.Request) {
	userID := request.PathParam(r, "id")
	me := middleware.UserFrom(r)
	if me != nil && me.ID == userID {
		response.WriteError(w, r, http.StatusBadRequest, "cannot_delete_self", "cannot delete your own account")
		return
	}
	target, err := h.svc.GetByID(r.Context(), userID)
	if err != nil {
		response.WriteError(w, r, http.StatusNotFound, "user_not_found", "user not found")
		return
	}
	if target.IsAdmin {
		response.WriteError(w, r, http.StatusBadRequest, "cannot_delete_admin", "admin accounts cannot be deleted")
		return
	}
	if err := h.svc.Delete(r.Context(), userID); err != nil {
		if err == user.ErrNotFound {
			response.WriteError(w, r, http.StatusNotFound, "user_not_found", "user not found")
			return
		}
		h.log.Error("delete user failed", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "could not delete user")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func publicUserView(u *user.User) map[string]any {
	return map[string]any{
		"id":           u.ID,
		"username":     u.Username,
		"display_name": u.DisplayName,
		"is_admin":     u.IsAdmin,
		"disabled":     u.Disabled(),
		"created_at":   u.CreatedAt,
	}
}

func firstNonEmpty(a, b string) string {
	if strings.TrimSpace(a) != "" {
		return a
	}
	return b
}
