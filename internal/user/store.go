package user

import (
	"context"
	"database/sql"
	"errors"
	"time"
)

// ErrNotFound covers unknown users.
var ErrNotFound = errors.New("user not found")

// ErrDisabled reports login attempts by disabled accounts.
var ErrDisabled = errors.New("user is disabled")

// User is an account record.
type User struct {
	ID           string
	Username     string
	DisplayName  string
	PasswordHash string
	IsAdmin      bool
	DisabledAt   time.Time
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

// Disabled reports whether the account is disabled.
func (u *User) Disabled() bool { return !u.DisabledAt.IsZero() }

// Store persists users.
type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) *Store {
	return &Store{db: db}
}

const userColumns = `id, username, password_hash, display_name, is_admin, disabled_at, created_at, updated_at`

func (s *Store) Create(ctx context.Context, u *User) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO users (id, username, password_hash, display_name, is_admin, disabled_at, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, NULL, ?, ?)`,
		u.ID, u.Username, u.PasswordHash, u.DisplayName, u.IsAdmin,
		u.CreatedAt.UTC().Format(time.RFC3339), u.UpdatedAt.UTC().Format(time.RFC3339))
	return err
}

func (s *Store) GetByUsername(ctx context.Context, username string) (*User, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT `+userColumns+` FROM users WHERE username = ?`, username)
	return scanUser(row)
}

func (s *Store) GetByID(ctx context.Context, id string) (*User, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT `+userColumns+` FROM users WHERE id = ?`, id)
	return scanUser(row)
}

// List returns all users ordered by creation time, newest first.
func (s *Store) List(ctx context.Context) ([]*User, error) {
	rows, err := s.db.QueryContext(ctx, `
		SELECT `+userColumns+` FROM users ORDER BY created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []*User
	for rows.Next() {
		u, err := scanUserRow(rows)
		if err != nil {
			return nil, err
		}
		out = append(out, u)
	}
	return out, rows.Err()
}

func (s *Store) UpdatePassword(ctx context.Context, id, passwordHash string) error {
	res, err := s.db.ExecContext(ctx, `
		UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?`,
		passwordHash, time.Now().UTC().Format(time.RFC3339), id)
	if err != nil {
		return err
	}
	n, err := res.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return ErrNotFound
	}
	return nil
}

// SetDisabled toggles the disabled_at marker (idempotent).
func (s *Store) SetDisabled(ctx context.Context, id string, disabled bool, at time.Time) error {
	var res sql.Result
	var err error
	if disabled {
		res, err = s.db.ExecContext(ctx, `
			UPDATE users SET disabled_at = COALESCE(disabled_at, ?), updated_at = ? WHERE id = ?`,
			at.UTC().Format(time.RFC3339), at.UTC().Format(time.RFC3339), id)
	} else {
		res, err = s.db.ExecContext(ctx, `
			UPDATE users SET disabled_at = NULL, updated_at = ? WHERE id = ?`,
			at.UTC().Format(time.RFC3339), id)
	}
	if err != nil {
		return err
	}
	n, err := res.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return ErrNotFound
	}
	return nil
}

// Delete removes a user; sessions and password resets cascade.
func (s *Store) Delete(ctx context.Context, id string) error {
	res, err := s.db.ExecContext(ctx, `DELETE FROM users WHERE id = ?`, id)
	if err != nil {
		return err
	}
	n, err := res.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return ErrNotFound
	}
	return nil
}

// DeleteSessionsForUser invalidates every session of a user (used on password reset).
func (s *Store) DeleteSessionsForUser(ctx context.Context, id string) error {
	_, err := s.db.ExecContext(ctx, `DELETE FROM sessions WHERE user_id = ?`, id)
	return err
}

type rowScanner interface {
	Scan(dest ...any) error
}

func scanUser(row rowScanner) (*User, error) {
	u := &User{}
	var isAdmin int
	var createdAt, updatedAt string
	var disabledAt sql.NullString
	if err := row.Scan(&u.ID, &u.Username, &u.PasswordHash, &u.DisplayName,
		&isAdmin, &disabledAt, &createdAt, &updatedAt); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}
	u.IsAdmin = isAdmin != 0
	if disabledAt.Valid {
		u.DisabledAt, _ = time.Parse(time.RFC3339, disabledAt.String)
	}
	u.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	u.UpdatedAt, _ = time.Parse(time.RFC3339, updatedAt)
	return u, nil
}

func scanUserRow(rows *sql.Rows) (*User, error) {
	return scanUser(rowAdapter{rows})
}

// rowAdapter adapts *sql.Rows to the rowScanner interface.
type rowAdapter struct{ rows *sql.Rows }

func (a rowAdapter) Scan(dest ...any) error { return a.rows.Scan(dest...) }
