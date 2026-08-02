package search

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
)

// ErrNotFound covers unknown index entries.
var ErrNotFound = errors.New("search entry not found")

// Result is one search hit.
type Result struct {
	Path    string `json:"path"`
	Snippet string `json:"snippet"`
}

// Store owns FTS5 upserts, deletes and queries.
type Store struct {
	db *sql.DB
}

func NewStore(db *sql.DB) *Store {
	return &Store{db: db}
}

// StateEntry is one indexed document snapshot.
type StateEntry struct {
	ProjectID string
	Path      string
	BlobSHA   string
	Content   string
}

// Upsert indexes one document; unchanged blobs are skipped.
func (s *Store) Upsert(ctx context.Context, e *StateEntry) (bool, error) {
	var existing string
	err := s.db.QueryRowContext(ctx,
		`SELECT blob_sha FROM doc_index_state WHERE project_id = ? AND path = ?`,
		e.ProjectID, e.Path).Scan(&existing)
	if err == nil && existing == e.BlobSHA {
		return false, nil // unchanged
	}
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return false, err
	}
	_, err = s.db.ExecContext(ctx, `
		INSERT INTO doc_index_state (project_id, path, blob_sha, content, updated_at)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(project_id, path) DO UPDATE SET
			blob_sha = excluded.blob_sha,
			content = excluded.content,
			updated_at = excluded.updated_at`,
		e.ProjectID, e.Path, e.BlobSHA, e.Content,
		time.Now().UTC().Format(time.RFC3339))
	if err != nil {
		return false, err
	}
	return true, nil
}

// Delete removes one indexed document (missing entries are fine).
func (s *Store) Delete(ctx context.Context, projectID, path string) error {
	_, err := s.db.ExecContext(ctx,
		`DELETE FROM doc_index_state WHERE project_id = ? AND path = ?`,
		projectID, path)
	return err
}

// DeleteProject removes every entry of a project.
func (s *Store) DeleteProject(ctx context.Context, projectID string) error {
	_, err := s.db.ExecContext(ctx,
		`DELETE FROM doc_index_state WHERE project_id = ?`, projectID)
	return err
}

// Query runs an FTS5 prefix search and returns snippets.
func (s *Store) Query(ctx context.Context, projectID, matchExpr string, limit int) ([]Result, error) {
	if limit <= 0 || limit > 50 {
		limit = 20
	}
	rows, err := s.db.QueryContext(ctx, `
		SELECT d.path,
		       snippet(doc_search, 0, '[', ']', '…', 24) AS snip
		FROM doc_search
		JOIN doc_index_state d ON d.id = doc_search.rowid
		WHERE doc_search MATCH ? AND d.project_id = ?
		ORDER BY rank
		LIMIT ?`, matchExpr, projectID, limit)
	if err != nil {
		// FTS syntax errors surface as query errors.
		return nil, fmt.Errorf("fts query: %w", err)
	}
	defer rows.Close()
	var out []Result
	for rows.Next() {
		var r Result
		var snip sql.NullString
		if err := rows.Scan(&r.Path, &snip); err != nil {
			return nil, err
		}
		r.Snippet = cleanSnippet(snip.String)
		out = append(out, r)
	}
	return out, rows.Err()
}

// BuildMatchExpr converts user words into a safe FTS5 AND-of-prefix expression.
func BuildMatchExpr(q string) string {
	fields := strings.Fields(q)
	quoted := make([]string, 0, len(fields))
	for _, f := range fields {
		escaped := strings.ReplaceAll(f, `"`, `""`)
		quoted = append(quoted, `"`+escaped+`"*`)
	}
	return strings.Join(quoted, " ")
}

// cleanSnippet strips the snippet highlight markers (search display is plain).
func cleanSnippet(s string) string {
	s = strings.ReplaceAll(s, "[", "")
	s = strings.ReplaceAll(s, "]", "")
	return strings.TrimSpace(s)
}
