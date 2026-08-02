package user

import (
	"context"
	"database/sql"
	"errors"
	"time"
)

var ErrNotFound = errors.New("user not found")

type User struct {
	ID           string
	Username     string
	DisplayName  string
	PasswordHash string
	IsAdmin      bool
	CreatedAt    time.Time
	UpdatedAt    time.Time
}

type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) *Store {
	return &Store{db: db}
}

func (s *Store) Create(ctx context.Context, u *User) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO users (id, username, password_hash, display_name, is_admin, created_at, updated_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)`,
		u.ID, u.Username, u.PasswordHash, u.DisplayName, u.IsAdmin,
		u.CreatedAt.UTC().Format(time.RFC3339), u.UpdatedAt.UTC().Format(time.RFC3339))
	return err
}

func (s *Store) GetByUsername(ctx context.Context, username string) (*User, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT id, username, password_hash, display_name, is_admin, created_at, updated_at
		FROM users WHERE username = ?`, username)
	return scanUser(row)
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

func scanUser(row *sql.Row) (*User, error) {
	u := &User{}
	var isAdmin int
	var createdAt, updatedAt string
	if err := row.Scan(&u.ID, &u.Username, &u.PasswordHash, &u.DisplayName,
		&isAdmin, &createdAt, &updatedAt); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, ErrNotFound
		}
		return nil, err
	}
	u.IsAdmin = isAdmin != 0
	u.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	u.UpdatedAt, _ = time.Parse(time.RFC3339, updatedAt)
	return u, nil
}
