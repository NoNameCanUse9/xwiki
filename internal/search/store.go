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
	Title     string
	Content   string
}

// ProjectIndexState records whether a project's searchable snapshot is in
// sync with a specific immutable Git revision.
type ProjectIndexState struct {
	ProjectID string `json:"project_id"`
	Revision  string `json:"revision"`
	Status    string `json:"status"`
	UpdatedAt string `json:"updated_at"`
	LastError string `json:"last_error,omitempty"`
}

func (s *Store) SetProjectState(ctx context.Context, state ProjectIndexState) error {
	_, err := s.db.ExecContext(ctx, `
		INSERT INTO project_index_state (project_id, revision, status, updated_at, last_error)
		VALUES (?, ?, ?, ?, ?)
		ON CONFLICT(project_id) DO UPDATE SET
			revision = excluded.revision,
			status = excluded.status,
			updated_at = excluded.updated_at,
			last_error = excluded.last_error`,
		state.ProjectID, state.Revision, state.Status, state.UpdatedAt, state.LastError)
	return err
}

func (s *Store) ProjectState(ctx context.Context, projectID string) (*ProjectIndexState, error) {
	var state ProjectIndexState
	err := s.db.QueryRowContext(ctx, `
		SELECT project_id, revision, status, updated_at, last_error
		FROM project_index_state WHERE project_id = ?`, projectID).
		Scan(&state.ProjectID, &state.Revision, &state.Status, &state.UpdatedAt, &state.LastError)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, ErrNotFound
	}
	return &state, err
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
		INSERT INTO doc_index_state (project_id, path, blob_sha, title, content, updated_at)
		VALUES (?, ?, ?, ?, ?, ?)
		ON CONFLICT(project_id, path) DO UPDATE SET
			blob_sha = excluded.blob_sha,
			title = excluded.title,
			content = excluded.content,
			updated_at = excluded.updated_at`,
		e.ProjectID, e.Path, e.BlobSHA, e.Title, e.Content,
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
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	for _, query := range []string{
		`DELETE FROM page_links WHERE project_id = ?`,
		`DELETE FROM doc_index_state WHERE project_id = ?`,
		`DELETE FROM project_index_state WHERE project_id = ?`,
	} {
		if _, err := tx.ExecContext(ctx, query, projectID); err != nil {
			return err
		}
	}
	return tx.Commit()
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

// Query searches for documents matching the query.
// Uses FTS5 trigram unless any query term is shorter than 3 runes, in which
// case it falls back to LIKE (trigram can't match short terms).
func (s *Store) Query(ctx context.Context, projectID, matchExpr, rawQuery string, limit int) ([]Result, error) {
	if limit <= 0 || limit > 50 {
		limit = 20
	}

	// FTS5 trigram requires at least 3 characters to form a trigram; a short
	// term silently matches nothing, so fall back to LIKE (AND across terms).
	terms := strings.Fields(strings.TrimSpace(rawQuery))
	for _, t := range terms {
		if len([]rune(t)) < 3 {
			return s.queryLike(ctx, projectID, terms, limit)
		}
	}

	rows, err := s.db.QueryContext(ctx, `
		SELECT d.path, d.content
		FROM doc_search
		JOIN doc_index_state d ON d.id = doc_search.rowid
		WHERE doc_search MATCH ? AND d.project_id = ?
		ORDER BY rank
		LIMIT ?`, matchExpr, projectID, limit)
	if err != nil {
		return nil, fmt.Errorf("fts query: %w", err)
	}
	defer rows.Close()
	var out []Result
	for rows.Next() {
		var r Result
		var content string
		if err := rows.Scan(&r.Path, &content); err != nil {
			return nil, err
		}
		r.Snippet = contentSnippet(content, rawQuery)
		out = append(out, r)
	}
	return out, rows.Err()
}

// queryLike performs a LIKE-based search for queries whose short terms can't
// be matched by FTS trigram. Terms are ANDed; LIKE wildcards are escaped so
// user input matches literally.
func (s *Store) queryLike(ctx context.Context, projectID string, terms []string, limit int) ([]Result, error) {
	q := `SELECT path, content FROM doc_index_state WHERE project_id = ?`
	args := []any{projectID}
	for _, t := range terms {
		q += ` AND content LIKE '%' || ? || '%' ESCAPE '\'`
		args = append(args, escapeLike(t))
	}
	q += ` ORDER BY rowid LIMIT ?`
	args = append(args, limit)
	rows, err := s.db.QueryContext(ctx, q, args...)
	if err != nil {
		return nil, fmt.Errorf("like query: %w", err)
	}
	defer rows.Close()
	var out []Result
	for rows.Next() {
		var r Result
		var content string
		if err := rows.Scan(&r.Path, &content); err != nil {
			return nil, err
		}
		r.Snippet = contentSnippet(content, strings.Join(terms, " "))
		out = append(out, r)
	}
	return out, rows.Err()
}

// escapeLike neutralizes LIKE wildcards so user input matches literally.
func escapeLike(s string) string {
	s = strings.ReplaceAll(s, `\`, `\\`)
	s = strings.ReplaceAll(s, `%`, `\%`)
	s = strings.ReplaceAll(s, `_`, `\_`)
	return s
}

// HasSHA reports whether the indexed snapshot for path already matches sha.
func (s *Store) HasSHA(ctx context.Context, projectID, path, sha string) (bool, error) {
	var one int
	err := s.db.QueryRowContext(ctx,
		`SELECT 1 FROM doc_index_state WHERE project_id = ? AND path = ? AND blob_sha = ?`,
		projectID, path, sha).Scan(&one)
	if err == nil {
		return true, nil
	}
	if errors.Is(err, sql.ErrNoRows) {
		return false, nil
	}
	return false, err
}

// contentSnippet extracts a short window around the first matching term.
func contentSnippet(content, query string) string {
	lower := strings.ToLower(content)
	terms := strings.Fields(query)
	if len(terms) == 0 {
		if len(content) > 120 {
			return content[:120] + "…"
		}
		return content
	}
	q := strings.ToLower(strings.Trim(terms[0], `"`))
	idx := strings.Index(lower, q)
	if idx < 0 {
		if len(content) > 120 {
			return content[:120] + "…"
		}
		return content
	}
	start := idx - 40
	if start < 0 {
		start = 0
	}
	end := idx + len(q) + 80
	if end > len(content) {
		end = len(content)
	}
	snip := strings.ReplaceAll(content[start:end], "\n", " ")
	return "…" + strings.TrimSpace(snip) + "…"
}

// BuildMatchExprRaw converts user words into a safe FTS5 query: every term is
// quoted so user input can't inject FTS syntax.
func BuildMatchExprRaw(q string) string {
	return buildMatchExpr(q)
}

func buildMatchExpr(q string) string {
	fields := strings.Fields(q)
	quoted := make([]string, 0, len(fields))
	for _, f := range fields {
		escaped := strings.ReplaceAll(f, `"`, `""`)
		quoted = append(quoted, `"`+escaped+`"`)
	}
	return strings.Join(quoted, " ")
}
