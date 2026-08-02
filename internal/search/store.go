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

// ReplaceLinks rebuilds the wiki-link index for one project (called during reindex).
func (s *Store) ReplaceLinks(ctx context.Context, projectID string, links map[string][]string) error {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err := tx.ExecContext(ctx, `DELETE FROM page_links WHERE project_id = ?`, projectID); err != nil {
		return err
	}
	for source, targets := range links {
		for _, t := range targets {
			if _, err := tx.ExecContext(ctx,
				`INSERT OR IGNORE INTO page_links (project_id, source_path, target_path) VALUES (?, ?, ?)`,
				projectID, source, t); err != nil {
				return err
			}
		}
	}
	return tx.Commit()
}

// Backlinks returns pages linking to targetPath, with a snippet of the
// linking line (best-effort: first 80 chars around the link).
func (s *Store) Backlinks(ctx context.Context, projectID, targetPath string) ([]Backlink, error) {
	rows, err := s.db.QueryContext(ctx, `
		SELECT pl.source_path, d.content
		FROM page_links pl
		LEFT JOIN doc_index_state d
		  ON d.project_id = pl.project_id AND d.path = pl.source_path
		WHERE pl.project_id = ? AND pl.target_path = ?
		ORDER BY pl.source_path`, projectID, targetPath)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []Backlink
	for rows.Next() {
		var source string
		var content sql.NullString
		if err := rows.Scan(&source, &content); err != nil {
			return nil, err
		}
		out = append(out, Backlink{Source: source, Snippet: linkSnippet(content.String, targetPath)})
	}
	return out, rows.Err()
}

// linkSnippet extracts a short window around the wiki link in the source.
func linkSnippet(content, target string) string {
	idx := strings.Index(content, "[["+target)
	if idx < 0 {
		idx = strings.Index(content, target)
	}
	if idx < 0 {
		if len(content) > 80 {
			return content[:80]
		}
		return content
	}
	start := idx - 30
	if start < 0 {
		start = 0
	}
	end := idx + len(target) + 40
	if end > len(content) {
		end = len(content)
	}
	snip := strings.ReplaceAll(content[start:end], "\n", " ")
	return strings.TrimSpace(snip)
}

// Backlink is one inbound reference.
type Backlink struct {
	Source  string `json:"source"`
	Snippet string `json:"snippet"`
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
