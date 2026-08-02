package handlers

import (
	"errors"
	"log/slog"
	"net/http"

	"agentdocs/internal/auth"
	"agentdocs/internal/config"
	"agentdocs/internal/httpapi/middleware"
	"agentdocs/internal/httpapi/request"
	"agentdocs/internal/httpapi/response"
	"agentdocs/internal/user"
)

type AuthHandler struct {
	cfg   *config.Config
	svc   *auth.Service
	users *user.Store
	log   *slog.Logger
}

func NewAuthHandler(cfg *config.Config, svc *auth.Service, users *user.Store, log *slog.Logger) *AuthHandler {
	return &AuthHandler{cfg: cfg, svc: svc, users: users, log: log}
}

type loginRequest struct {
	Username string `json:"username"`
	Password string `json:"password"`
}

func (h *AuthHandler) Login(w http.ResponseWriter, r *http.Request) {
	var req loginRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	u, token, err := h.svc.Login(r.Context(), h.users, req.Username, req.Password)
	if err != nil {
		if errors.Is(err, auth.ErrDisabled) {
			response.WriteError(w, r, http.StatusForbidden, "account_disabled", "account is disabled")
			return
		}
		response.WriteError(w, r, http.StatusUnauthorized, "invalid_credentials", "invalid username or password")
		return
	}
	http.SetCookie(w, &http.Cookie{
		Name:     "agentdocs_session",
		Value:    token,
		Path:     "/",
		HttpOnly: true,
		Secure:   h.cfg.SecureCookies,
		SameSite: http.SameSiteLaxMode,
		MaxAge:   int(h.cfg.SessionTTL.Seconds()),
	})
	response.WriteJSON(w, http.StatusOK, map[string]any{"user": publicUser(u)})
}

func (h *AuthHandler) Logout(w http.ResponseWriter, r *http.Request) {
	if cookie, err := r.Cookie("agentdocs_session"); err == nil {
		_ = h.svc.DeleteSessionByToken(r.Context(), cookie.Value)
	}
	http.SetCookie(w, &http.Cookie{
		Name:     "agentdocs_session",
		Value:    "",
		Path:     "/",
		HttpOnly: true,
		Secure:   h.cfg.SecureCookies,
		SameSite: http.SameSiteLaxMode,
		MaxAge:   -1,
	})
	response.WriteJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func (h *AuthHandler) Me(w http.ResponseWriter, r *http.Request) {
	u := middleware.UserFrom(r)
	response.WriteJSON(w, http.StatusOK, map[string]any{"user": publicUser(u)})
}

type passwordRequest struct {
	CurrentPassword string `json:"current_password"`
	NewPassword     string `json:"new_password"`
}

func (h *AuthHandler) Password(w http.ResponseWriter, r *http.Request) {
	var req passwordRequest
	if err := request.DecodeJSON(w, r, &req, h.cfg.MaxBodyBytes); err != nil {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "invalid request body")
		return
	}
	if len(req.NewPassword) < 8 {
		response.WriteError(w, r, http.StatusBadRequest, "validation_failed", "new password must be at least 8 characters")
		return
	}
	me := middleware.UserFrom(r)
	fresh, err := h.users.GetByUsername(r.Context(), me.Username)
	if err != nil {
		response.WriteError(w, r, http.StatusUnauthorized, "invalid_credentials", "current password is incorrect")
		return
	}
	ok, err := auth.VerifyPassword(req.CurrentPassword, fresh.PasswordHash)
	if err != nil || !ok {
		response.WriteError(w, r, http.StatusUnauthorized, "invalid_credentials", "current password is incorrect")
		return
	}
	hash, err := auth.HashPassword(req.NewPassword)
	if err != nil {
		h.log.Error("hash password", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "internal error")
		return
	}
	if err := h.users.UpdatePassword(r.Context(), fresh.ID, hash); err != nil {
		h.log.Error("update password", "error", err)
		response.WriteError(w, r, http.StatusInternalServerError, "internal_error", "internal error")
		return
	}
	response.WriteJSON(w, http.StatusOK, map[string]any{"ok": true})
}

func publicUser(u *user.User) map[string]any {
	return map[string]any{
		"id":           u.ID,
		"username":     u.Username,
		"display_name": u.DisplayName,
		"is_admin":     u.IsAdmin,
	}
}
