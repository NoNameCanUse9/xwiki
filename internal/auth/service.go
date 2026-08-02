package auth

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"errors"
	"time"

	"agentdocs/internal/platform/clock"
	"agentdocs/internal/platform/id"
	"agentdocs/internal/user"
)

var (
	ErrInvalidCredentials = errors.New("invalid credentials")
	ErrSessionNotFound    = errors.New("session not found")
	ErrDisabled           = errors.New("account is disabled")
)

type Session struct {
	ID        string
	UserID    string
	TokenHash string
	ExpiresAt time.Time
	CreatedAt time.Time
}

// Service manages login and sessions. Only token hashes are stored.
type Service struct {
	db    *sql.DB
	clock clock.Clock
	ttl   time.Duration
}

func NewService(db *sql.DB, clk clock.Clock, ttl time.Duration) *Service {
	return &Service{db: db, clock: clk, ttl: ttl}
}

func hashToken(token string) string {
	sum := sha256.Sum256([]byte(token))
	return base64.RawStdEncoding.EncodeToString(sum[:])
}

// Login verifies credentials and creates a session, returning the user and
// the raw session token (shown to the client exactly once).
func (s *Service) Login(ctx context.Context, users *user.Store, username, password string) (*user.User, string, error) {
	u, err := users.GetByUsername(ctx, username)
	if err != nil {
		return nil, "", ErrInvalidCredentials
	}
	if u.Disabled() {
		return nil, "", ErrDisabled
	}
	ok, err := VerifyPassword(password, u.PasswordHash)
	if err != nil || !ok {
		return nil, "", ErrInvalidCredentials
	}
	token, err := s.CreateSession(ctx, u.ID)
	if err != nil {
		return nil, "", err
	}
	return u, token, nil
}

func (s *Service) CreateSession(ctx context.Context, userID string) (string, error) {
	raw := make([]byte, 32)
	if _, err := rand.Read(raw); err != nil {
		return "", err
	}
	token := base64.RawURLEncoding.EncodeToString(raw)
	now := s.clock.Now().UTC()
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at, last_used_at)
		VALUES (?, ?, ?, ?, ?, ?)`,
		id.New("ses"), userID, hashToken(token),
		now.Add(s.ttl).Format(time.RFC3339),
		now.Format(time.RFC3339), now.Format(time.RFC3339))
	if err != nil {
		return "", err
	}
	return token, nil
}

// ResolveSession returns the session and its user for a raw token, or
// ErrSessionNotFound when the token is unknown or expired.
func (s *Service) ResolveSession(ctx context.Context, token string) (*Session, *user.User, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT s.id, s.user_id, s.expires_at, s.created_at,
		       u.id, u.username, u.display_name, u.is_admin, u.created_at, u.updated_at
		FROM sessions s
		JOIN users u ON u.id = s.user_id
		WHERE s.token_hash = ?`, hashToken(token))

	ses := &Session{}
	u := &user.User{}
	var isAdmin int
	var sesExpires, sesCreated, uCreated, uUpdated string
	err := row.Scan(&ses.ID, &ses.UserID, &sesExpires, &sesCreated,
		&u.ID, &u.Username, &u.DisplayName, &isAdmin, &uCreated, &uUpdated)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, nil, ErrSessionNotFound
		}
		return nil, nil, err
	}
	u.IsAdmin = isAdmin != 0
	ses.ExpiresAt, _ = time.Parse(time.RFC3339, sesExpires)
	ses.CreatedAt, _ = time.Parse(time.RFC3339, sesCreated)
	u.CreatedAt, _ = time.Parse(time.RFC3339, uCreated)
	u.UpdatedAt, _ = time.Parse(time.RFC3339, uUpdated)

	if !ses.ExpiresAt.After(s.clock.Now()) {
		_ = s.DeleteSession(ctx, ses.ID)
		return nil, nil, ErrSessionNotFound
	}
	_, _ = s.db.ExecContext(ctx, `UPDATE sessions SET last_used_at = ? WHERE id = ?`,
		s.clock.Now().UTC().Format(time.RFC3339), ses.ID)
	return ses, u, nil
}

func (s *Service) DeleteSession(ctx context.Context, sessionID string) error {
	_, err := s.db.ExecContext(ctx, `DELETE FROM sessions WHERE id = ?`, sessionID)
	return err
}

func (s *Service) DeleteSessionByToken(ctx context.Context, token string) error {
	_, err := s.db.ExecContext(ctx, `DELETE FROM sessions WHERE token_hash = ?`, hashToken(token))
	return err
}
