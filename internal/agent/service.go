package agent

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"strings"

	"xwiki/internal/platform/clock"
	"xwiki/internal/platform/id"
)

// Service applies token authorization rules and idempotency.
type Service struct {
	store *Store
	clock clock.Clock
}

// NewService wires the agent service to its store.
func NewService(db *sql.DB, clk clock.Clock) *Service {
	return &Service{store: NewStore(db), clock: clk}
}

// CreateInput is the user-supplied token creation request.
type CreateInput struct {
	Name       string
	Scope      string // "read" | "write"
	ProjectIDs []string
}

// CreatedToken carries the one-time secret.
type CreatedToken struct {
	Token  *Token `json:"token"`
	Secret string `json:"secret"`
}

// Create validates and persists a new token, returning its secret once.
func (s *Service) Create(ctx context.Context, input CreateInput) (*CreatedToken, error) {
	if strings.TrimSpace(input.Name) == "" || len(input.Name) > 64 {
		return nil, ErrInvalid
	}
	if input.Scope != "read" && input.Scope != "write" {
		return nil, ErrInvalid
	}
	if len(input.ProjectIDs) == 0 {
		return nil, ErrInvalid // tokens must be bound explicitly
	}
	now := s.clock.Now().UTC()
	t := &Token{
		ID:         id.New("tok"),
		Name:       strings.TrimSpace(input.Name),
		Scope:      input.Scope,
		ProjectIDs: input.ProjectIDs,
		CreatedAt:  now,
	}
	secret, err := s.store.CreateToken(ctx, t)
	if err != nil {
		return nil, err
	}
	return &CreatedToken{Token: t, Secret: secret}, nil
}

// List returns all tokens.
func (s *Service) List(ctx context.Context) ([]*Token, error) {
	return s.store.ListTokens(ctx)
}

// Revoke marks a token revoked (idempotent).
func (s *Service) Revoke(ctx context.Context, id string) error {
	return s.store.RevokeToken(ctx, id, s.clock.Now().UTC())
}

// Authorize validates a raw secret against the requested access. projectID
// may be empty for global checks (only scope is verified then). write=true
// enforces the token's write scope; project access is always project-scoped.
func (s *Service) Authorize(ctx context.Context, secret, projectID string, write bool) (*Token, error) {
	t, err := s.store.GetBySecret(ctx, secret)
	if err != nil {
		if errors.Is(err, ErrInvalid) {
			return nil, ErrNotFound // malformed secrets resolve as unknown
		}
		return nil, err
	}
	if !t.RevokedAt.IsZero() {
		return nil, ErrTokenRevoked
	}
	if write && t.Scope != "write" {
		return nil, ErrForbidden
	}
	if projectID != "" {
		if !contains(t.ProjectIDs, projectID) {
			return nil, ErrForbidden
		}
	}
	_ = s.store.TouchToken(ctx, t.ID, s.clock.Now().UTC())
	return t, nil
}

// RequestHash hashes the canonical request body for idempotency comparison.
func RequestHash(body []byte) string {
	sum := sha256.Sum256(body)
	return hex.EncodeToString(sum[:])
}

// ApplyIdempotent runs fn once per (key, projectID): a replay returns the
// stored result; a different payload with the same key conflicts.
func (s *Service) ApplyIdempotent(ctx context.Context, key, projectID, requestHash string, run func() (string, error)) (string, bool, error) {
	if key == "" {
		result, err := run()
		return result, false, err
	}
	if existing, err := s.store.GetIdempotency(ctx, key, projectID); err == nil {
		if existing.RequestHash != requestHash {
			return "", false, ErrIdempotencyConflict
		}
		return existing.ResultJSON, true, nil
	}
	result, err := run()
	if err != nil {
		return "", false, err
	}
	if err := s.store.PutIdempotency(ctx, &IdempotencyRecord{
		Key: key, ProjectID: projectID, RequestHash: requestHash,
		ResultJSON: result, CreatedAt: s.clock.Now().UTC(),
	}); err != nil {
		return "", false, err
	}
	return result, false, nil
}

// Audit writes an audit entry.
func (s *Service) Audit(ctx context.Context, actorType, actorID, projectID, action, path, detail, requestID string) error {
	return s.store.AppendAudit(ctx, &AuditEntry{
		ID:        id.New("aud"),
		ActorType: actorType,
		ActorID:   actorID,
		ProjectID: projectID,
		Action:    action,
		Path:      path,
		Detail:    detail,
		RequestID: requestID,
		CreatedAt: s.clock.Now().UTC(),
	})
}

// StoreRecent exposes the audit store for handlers.
func (s *Service) StoreRecent(ctx context.Context, projectID string, limit, offset int) ([]AuditEntry, bool, error) {
	return s.store.RecentAudit(ctx, projectID, limit, offset)
}

// MarshalResult serializes an idempotency result for storage.
func MarshalResult(v any) (string, error) {
	b, err := json.Marshal(v)
	return string(b), err
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}
