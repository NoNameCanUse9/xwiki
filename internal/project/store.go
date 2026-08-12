package project

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"time"
)

// Store persists project metadata in SQLite.
type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) *Store {
	return &Store{db: db}
}

const projectColumns = `id, name, description, repo_dir, archived_at, deleted_at, created_at, updated_at`

func scanProject(row *sql.Row) (*Project, error) {
	var p Project
	var archivedAt sql.NullString
	var deletedAt sql.NullString
	var createdAt, updatedAt string
	err := row.Scan(&p.ID, &p.Name, &p.Description, &p.RepoDir, &archivedAt, &deletedAt, &createdAt, &updatedAt)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	if archivedAt.Valid {
		t, err := time.Parse(time.RFC3339, archivedAt.String)
		if err != nil {
			return nil, fmt.Errorf("parse archived_at: %w", err)
		}
		p.ArchivedAt = &t
	}
	p.Archived = p.IsArchived()
	if deletedAt.Valid {
		t, err := time.Parse(time.RFC3339, deletedAt.String)
		if err != nil {
			return nil, fmt.Errorf("parse deleted_at: %w", err)
		}
		p.DeletedAt = &t
	}
	p.Deleted = p.IsDeleted()
	if p.CreatedAt, err = time.Parse(time.RFC3339, createdAt); err != nil {
		return nil, fmt.Errorf("parse created_at: %w", err)
	}
	if p.UpdatedAt, err = time.Parse(time.RFC3339, updatedAt); err != nil {
		return nil, fmt.Errorf("parse updated_at: %w", err)
	}
	return &p, nil
}

// Create inserts a project; a duplicate name yields ErrConflict.
func (s *Store) Create(ctx context.Context, p *Project) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO projects (id, name, description, repo_dir, archived_at, created_at, updated_at)
		VALUES (?, ?, ?, ?, NULL, ?, ?)`,
		p.ID, p.Name, p.Description, p.RepoDir,
		p.CreatedAt.UTC().Format(time.RFC3339), p.UpdatedAt.UTC().Format(time.RFC3339))
	if err != nil && isUniqueViolation(err) {
		return ErrConflict
	}
	return err
}

func (s *Store) GetByID(ctx context.Context, id string) (*Project, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT `+projectColumns+` FROM projects WHERE id = ? AND deleted_at IS NULL`, id)
	return scanProject(row)
}

// GetByName resolves a project by its unique name.
func (s *Store) GetByName(ctx context.Context, name string) (*Project, error) {
	row := s.db.QueryRowContext(ctx, `
		SELECT `+projectColumns+` FROM projects WHERE name = ? AND deleted_at IS NULL`, name)
	return scanProject(row)
}

// List returns all projects ordered by creation time, newest first.
func (s *Store) List(ctx context.Context) ([]*Project, error) {
	rows, err := s.db.QueryContext(ctx, `
		SELECT `+projectColumns+` FROM projects WHERE deleted_at IS NULL ORDER BY created_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []*Project
	for rows.Next() {
		var p Project
		var archivedAt sql.NullString
		var deletedAt sql.NullString
		var createdAt, updatedAt string
		if err := rows.Scan(&p.ID, &p.Name, &p.Description, &p.RepoDir, &archivedAt, &deletedAt, &createdAt, &updatedAt); err != nil {
			return nil, err
		}
		if archivedAt.Valid {
			t, err := time.Parse(time.RFC3339, archivedAt.String)
			if err != nil {
				return nil, fmt.Errorf("parse archived_at: %w", err)
			}
			p.ArchivedAt = &t
		}
		p.Archived = p.IsArchived()
		if deletedAt.Valid {
			t, err := time.Parse(time.RFC3339, deletedAt.String)
			if err != nil {
				return nil, fmt.Errorf("parse deleted_at: %w", err)
			}
			p.DeletedAt = &t
		}
		p.Deleted = p.IsDeleted()
		if p.CreatedAt, err = time.Parse(time.RFC3339, createdAt); err != nil {
			return nil, fmt.Errorf("parse created_at: %w", err)
		}
		if p.UpdatedAt, err = time.Parse(time.RFC3339, updatedAt); err != nil {
			return nil, fmt.Errorf("parse updated_at: %w", err)
		}
		out = append(out, &p)
	}
	return out, rows.Err()
}

func (s *Store) GetDeletedByID(ctx context.Context, id string) (*Project, error) {
	row := s.db.QueryRowContext(ctx, `SELECT `+projectColumns+` FROM projects WHERE id = ? AND deleted_at IS NOT NULL`, id)
	return scanProject(row)
}

func (s *Store) ListDeleted(ctx context.Context) ([]*Project, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT `+projectColumns+` FROM projects WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []*Project
	for rows.Next() {
		var p Project
		var archivedAt, deletedAt sql.NullString
		var createdAt, updatedAt string
		if err := rows.Scan(&p.ID, &p.Name, &p.Description, &p.RepoDir, &archivedAt, &deletedAt, &createdAt, &updatedAt); err != nil {
			return nil, err
		}
		if archivedAt.Valid {
			t, err := time.Parse(time.RFC3339, archivedAt.String)
			if err != nil {
				return nil, err
			}
			p.ArchivedAt = &t
		}
		if deletedAt.Valid {
			t, err := time.Parse(time.RFC3339, deletedAt.String)
			if err != nil {
				return nil, err
			}
			p.DeletedAt = &t
		}
		p.Archived, p.Deleted = p.IsArchived(), p.IsDeleted()
		p.CreatedAt, err = time.Parse(time.RFC3339, createdAt)
		if err != nil {
			return nil, err
		}
		p.UpdatedAt, err = time.Parse(time.RFC3339, updatedAt)
		if err != nil {
			return nil, err
		}
		out = append(out, &p)
	}
	return out, rows.Err()
}

func (s *Store) SoftDelete(ctx context.Context, id string, at time.Time) error {
	res, err := s.db.ExecContext(ctx, `UPDATE projects SET deleted_at = COALESCE(deleted_at, ?), updated_at = ? WHERE id = ? AND deleted_at IS NULL`, at.UTC().Format(time.RFC3339), at.UTC().Format(time.RFC3339), id)
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

func (s *Store) RestoreDeleted(ctx context.Context, id string, at time.Time) error {
	res, err := s.db.ExecContext(ctx, `UPDATE projects SET deleted_at = NULL, updated_at = ? WHERE id = ? AND deleted_at IS NOT NULL`, at.UTC().Format(time.RFC3339), id)
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

// Unarchive clears the archived flag (idempotent).
func (s *Store) Unarchive(ctx context.Context, id string, at time.Time) error {
	res, err := s.db.ExecContext(ctx, `
		UPDATE projects
		SET archived_at = NULL, updated_at = ?
		WHERE id = ?`,
		at.UTC().Format(time.RFC3339), id)
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

// Archive marks a project archived. The first call records the timestamp;
// later calls are idempotent and never overwrite it.
func (s *Store) Archive(ctx context.Context, id string, at time.Time) error {
	res, err := s.db.ExecContext(ctx, `
		UPDATE projects
		SET archived_at = COALESCE(archived_at, ?), updated_at = ?
		WHERE id = ?`,
		at.UTC().Format(time.RFC3339), at.UTC().Format(time.RFC3339), id)
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

// Rename updates a project's name and bump the updated timestamp. A
// duplicate target name yields ErrConflict; a missing row yields
// ErrNotFound.
func (s *Store) Rename(ctx context.Context, id, name string, at time.Time) error {
	res, err := s.db.ExecContext(ctx, `
		UPDATE projects
		SET name = ?, updated_at = ?
		WHERE id = ?`,
		name, at.UTC().Format(time.RFC3339), id)
	if err != nil {
		if isUniqueViolation(err) {
			return ErrConflict
		}
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

// Delete removes a project's metadata row. A missing row yields
// ErrNotFound.
func (s *Store) Delete(ctx context.Context, id string) error {
	res, err := s.db.ExecContext(ctx, `DELETE FROM projects WHERE id = ?`, id)
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

func isUniqueViolation(err error) bool {
	var sqliteErr interface{ Error() string }
	return errors.As(err, &sqliteErr) && contains(sqliteErr.Error(), "UNIQUE constraint failed")
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && (s == sub || len(sub) == 0 || indexOf(s, sub) >= 0)
}

func indexOf(s, sub string) int {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return i
		}
	}
	return -1
}
