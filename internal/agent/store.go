package agent

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"strings"
	"time"
)

// ErrNotFound covers missing tokens and unknown actors.
var ErrNotFound = errors.New("agent token not found")

// ErrTokenRevoked reports use of a revoked token.
var ErrTokenRevoked = errors.New("agent token revoked")

// ErrForbidden reports scope/project authorization failures.
var ErrForbidden = errors.New("agent forbidden")

// ErrIdempotencyConflict reports a reused key with a different payload.
var ErrIdempotencyConflict = errors.New("idempotency key reused with different request")

// ErrInvalid reports malformed inputs (empty project list, bad token format).
var ErrInvalid = errors.New("invalid agent input")

// Token is the stored metadata of an agent token (never the secret).
type Token struct {
	ID         string    `json:"id"`
	Name       string    `json:"name"`
	Scope      string    `json:"scope"`
	ProjectIDs []string  `json:"project_ids"`
	CreatedAt  time.Time `json:"created_at"`
	LastUsedAt time.Time `json:"last_used_at,omitempty"`
	RevokedAt  time.Time `json:"revoked_at,omitempty"`
}

// AuditEntry is one audit log row.
type AuditEntry struct {
	ID        string    `json:"id"`
	ActorType string    `json:"actor_type"`
	ActorID   string    `json:"actor_id"`
	ProjectID string    `json:"project_id,omitempty"`
	Action    string    `json:"action"`
	Path      string    `json:"path,omitempty"`
	Detail    string    `json:"detail,omitempty"`
	RequestID string    `json:"request_id,omitempty"`
	CreatedAt time.Time `json:"created_at"`
}

// Store persists agent tokens, idempotency keys and audit entries.
type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) *Store {
	return &Store{db: db}
}

// NewSecret generates a fresh token secret: ad_<32 hex>.
func NewSecret() (string, error) {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		return "", err
	}
	return "ad_" + hex.EncodeToString(b[:]), nil
}

func hashSecret(secret string) string {
	sum := sha256.Sum256([]byte(secret))
	return hex.EncodeToString(sum[:])
}

// CreateToken inserts a token and returns its secret (shown exactly once).
func (s *Store) CreateToken(ctx context.Context, t *Token) (string, error) {
	secret, err := NewSecret()
	if err != nil {
		return "", err
	}
	projectJSON, err := json.Marshal(t.ProjectIDs)
	if err != nil {
		return "", err
	}
	_, err = s.db.ExecContext(ctx, `
		INSERT INTO agent_tokens (id, name, token_hash, scope, project_ids, path_prefixes, created_at)
		VALUES (?, ?, ?, ?, ?, '[]', ?)`,
		t.ID, t.Name, hashSecret(secret), t.Scope,
		string(projectJSON), t.CreatedAt.UTC().Format(time.RFC3339))
	if err != nil {
		return "", err
	}
	return secret, nil
}

func scanToken(row *sql.Row) (*Token, error) {
	var t Token
	var projects, createdAt string
	var lastUsed, revoked sql.NullString
	err := row.Scan(&t.ID, &t.Name, &t.Scope, &projects,
		&createdAt, &lastUsed, &revoked)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	_ = json.Unmarshal([]byte(projects), &t.ProjectIDs)
	t.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	if lastUsed.Valid {
		t.LastUsedAt, _ = time.Parse(time.RFC3339, lastUsed.String)
	}
	if revoked.Valid {
		t.RevokedAt, _ = time.Parse(time.RFC3339, revoked.String)
	}
	return &t, nil
}

// GetBySecret resolves a token by its raw secret.
func (s *Store) GetBySecret(ctx context.Context, secret string) (*Token, error) {
	if len(secret) < 8 || secret[:3] != "ad_" {
		return nil, ErrInvalid
	}
	row := s.db.QueryRowContext(ctx, `
		SELECT id, name, scope, project_ids, created_at, last_used_at, revoked_at
		FROM agent_tokens WHERE token_hash = ?`, hashSecret(secret))
	return scanToken(row)
}

// ListTokens returns all tokens, newest first.
func (s *Store) ListTokens(ctx context.Context) ([]*Token, error) {
	rows, err := s.db.QueryContext(ctx, `
		SELECT id, name, scope, project_ids, created_at, last_used_at, revoked_at
		FROM agent_tokens ORDER BY created_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := make([]*Token, 0)
	for rows.Next() {
		var t Token
		var projects, createdAt string
		var lastUsed, revoked sql.NullString
		if err := rows.Scan(&t.ID, &t.Name, &t.Scope, &projects,
			&createdAt, &lastUsed, &revoked); err != nil {
			return nil, err
		}
		_ = json.Unmarshal([]byte(projects), &t.ProjectIDs)
		t.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
		if lastUsed.Valid {
			t.LastUsedAt, _ = time.Parse(time.RFC3339, lastUsed.String)
		}
		if revoked.Valid {
			t.RevokedAt, _ = time.Parse(time.RFC3339, revoked.String)
		}
		out = append(out, &t)
	}
	return out, rows.Err()
}

// RevokeToken marks a token revoked (idempotent).
func (s *Store) RevokeToken(ctx context.Context, id string, at time.Time) error {
	res, err := s.db.ExecContext(ctx, `
		UPDATE agent_tokens SET revoked_at = COALESCE(revoked_at, ?) WHERE id = ?`,
		at.UTC().Format(time.RFC3339), id)
	if err != nil {
		return err
	}
	n, _ := res.RowsAffected()
	if n == 0 {
		return ErrNotFound
	}
	return nil
}

// TouchToken updates last_used_at.
func (s *Store) TouchToken(ctx context.Context, id string, at time.Time) error {
	_, err := s.db.ExecContext(ctx,
		`UPDATE agent_tokens SET last_used_at = ? WHERE id = ?`,
		at.UTC().Format(time.RFC3339), id)
	return err
}

// IdempotencyRecord is a stored idempotent result.
type IdempotencyRecord struct {
	Key         string
	ProjectID   string
	RequestHash string
	ResultJSON  string
	CreatedAt   time.Time
}

// GetIdempotency loads a stored record; returns ErrNotFound when absent.
func (s *Store) GetIdempotency(ctx context.Context, key, projectID string) (*IdempotencyRecord, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT key, project_id, request_hash, result_json, created_at
		FROM idempotency_keys WHERE key = ? AND project_id = ?`, key, projectID)
	var r IdempotencyRecord
	var createdAt string
	err := row.Scan(&r.Key, &r.ProjectID, &r.RequestHash, &r.ResultJSON, &createdAt)
	r.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	return &r, nil
}

// PutIdempotency stores a result; a duplicate key+project returns
// ErrIdempotencyConflict when the request hash differs.
func (s *Store) PutIdempotency(ctx context.Context, rec *IdempotencyRecord) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO idempotency_keys (key, project_id, request_hash, result_json, created_at)
		VALUES (?, ?, ?, ?, ?)`,
		rec.Key, rec.ProjectID, rec.RequestHash, rec.ResultJSON,
		rec.CreatedAt.UTC().Format(time.RFC3339))
	if err != nil && isUniqueViolation(err) {
		existing, gerr := s.GetIdempotency(ctx, rec.Key, rec.ProjectID)
		if gerr != nil {
			return gerr
		}
		if existing.RequestHash != rec.RequestHash {
			return ErrIdempotencyConflict
		}
		return nil // same request replayed: fine
	}
	return err
}

// AppendAudit writes one audit entry.
func (s *Store) AppendAudit(ctx context.Context, e *AuditEntry) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO audit_logs (id, actor_type, actor_id, project_id, action, path, detail, request_id, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		e.ID, e.ActorType, e.ActorID, nullable(e.ProjectID), e.Action, nullable(e.Path),
		nullable(e.Detail), nullable(e.RequestID), e.CreatedAt.UTC().Format(time.RFC3339))
	return err
}

// RecentAudit returns the latest audit entries (project-scoped when set),
// newest first, plus whether more entries exist beyond this page.
func (s *Store) RecentAudit(ctx context.Context, projectID string, limit, offset int) ([]AuditEntry, bool, error) {
	if limit <= 0 || limit > 100 {
		limit = 20
	}
	if offset < 0 {
		offset = 0
	}
	q := `SELECT id, actor_type, actor_id, project_id, action, path, detail, request_id, created_at
	      FROM audit_logs`
	var args []any
	if projectID != "" {
		q += ` WHERE project_id = ?`
		args = append(args, projectID)
	}
	q += ` ORDER BY created_at DESC LIMIT ? OFFSET ?`
	args = append(args, limit+1, offset)
	rows, err := s.db.QueryContext(ctx, q, args...)
	if err != nil {
		return nil, false, err
	}
	defer rows.Close()
	out := make([]AuditEntry, 0, limit)
	for rows.Next() {
		var e AuditEntry
		var projectID, path, detail, requestID sql.NullString
		var createdAt string
		if err := rows.Scan(&e.ID, &e.ActorType, &e.ActorID, &projectID, &e.Action,
			&path, &detail, &requestID, &createdAt); err != nil {
			return nil, false, err
		}
		e.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
		e.ProjectID = projectID.String
		e.Path = path.String
		e.Detail = detail.String
		e.RequestID = requestID.String
		out = append(out, e)
	}
	if err := rows.Err(); err != nil {
		return nil, false, err
	}
	hasMore := len(out) > limit
	if hasMore {
		out = out[:limit]
	}
	return out, hasMore, nil
}

func nullable(s string) any {
	if s == "" {
		return nil
	}
	return s
}

func isUniqueViolation(err error) bool {
	return err != nil && strings.Contains(err.Error(), "UNIQUE constraint failed")
}
