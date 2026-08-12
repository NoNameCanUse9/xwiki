package search

import (
	"context"
	"database/sql"
	"fmt"
	"regexp"
	"strings"
	"time"

	"agentdocs/internal/platform/clock"
	"agentdocs/internal/project"
)

// wikiLinkPattern matches [[path]] and [[path|label]] inside markdown sources.
var wikiLinkPattern = regexp.MustCompile(`\[\[([^\]|]+)(?:\|[^\]]+)?\]\]`)

// ReindexStats reports what a reindex pass changed.
type ReindexStats struct {
	Indexed int `json:"indexed"`
	Removed int `json:"removed"`
}

// ExtractTitle returns the first Markdown heading ("# ...") from content,
// or an empty string if none is found.
func ExtractTitle(md string) string {
	for _, line := range strings.Split(md, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "# ") {
			return strings.TrimPrefix(line, "# ")
		}
	}
	return ""
}

// Service coordinates FTS indexing with the project Git trees.
type Service struct {
	store    *Store
	projects *project.Service
	clock    clock.Clock
	maxBlob  int
}

// NewService wires the search service.
func NewService(db *sql.DB, projects *project.Service) *Service {
	return &Service{
		store:    NewStore(db),
		projects: projects,
		clock:    clock.Real{},
		maxBlob:  project.MaxDocBlobBytes,
	}
}

// Search runs a project-scoped FTS query.
func (s *Service) Search(ctx context.Context, projectID, query string, limit int) ([]Result, error) {
	q := strings.TrimSpace(query)
	if q == "" || len(q) > 200 {
		return nil, fmt.Errorf("invalid query")
	}
	// For FTS, use the raw query (no spacing) so trigram can match substrings.
	// Queries with any short (<3 rune) word fall back to LIKE inside the store.
	return s.store.Query(ctx, projectID, BuildMatchExprRaw(q), q, limit)
}

// DeleteProject removes every index entry for a project (used when the
// project itself is deleted).
func (s *Service) DeleteProject(ctx context.Context, projectID string) error {
	return s.store.DeleteProject(ctx, projectID)
}

// MarkDirty records that a project requires a complete reindex.
func (s *Service) MarkDirty(ctx context.Context, projectID string) error {
	return s.store.SetProjectState(ctx, ProjectIndexState{
		ProjectID: projectID, Status: "dirty", UpdatedAt: s.clock.Now().UTC().Format(time.RFC3339),
	})
}

// IndexState returns the persisted health of a project's search snapshot.
func (s *Service) IndexState(ctx context.Context, projectID string) (*ProjectIndexState, error) {
	return s.store.ProjectState(ctx, projectID)
}

// ReindexProject incrementally syncs the index with the project's current
// Git tree: changed blobs are upserted, vanished paths are removed.
func (s *Service) ReindexProject(ctx context.Context, projectID string) (stats *ReindexStats, retErr error) {
	_ = s.MarkDirty(ctx, projectID)
	defer func() {
		if retErr != nil {
			_ = s.store.SetProjectState(context.Background(), ProjectIndexState{
				ProjectID: projectID, Status: "error", LastError: retErr.Error(),
				UpdatedAt: s.clock.Now().UTC().Format(time.RFC3339),
			})
		}
	}()
	repo, err := s.projects.OpenRepo(ctx, projectID)
	if err != nil {
		return nil, err
	}
	revision, err := repo.Revision(ctx)
	if err != nil {
		return nil, err
	}
	stats = &ReindexStats{}
	seen := map[string]bool{}

	var walk func(dir string) error
	walk = func(dir string) error {
		entries, err := repo.ListTree(ctx, revision, dir)
		if err != nil {
			return err
		}
		for _, e := range entries {
			if e.Type == "tree" {
				if err := walk(e.Path); err != nil {
					return err
				}
				continue
			}
			seen[e.Path] = true
			// The tree already carries the blob sha: unchanged files are
			// skipped without reading or hashing anything.
			if e.SHA != "" {
				ok, err := s.store.HasSHA(ctx, projectID, e.Path, e.SHA)
				if err != nil {
					return err
				}
				if ok {
					continue
				}
			}
			blob, err := repo.ReadBlob(ctx, revision, e.Path)
			if err != nil {
				continue // unreadable blobs are skipped
			}
			if len(blob) > s.maxBlob || strings.ContainsRune(string(blob), '\x00') {
				continue // binary or oversized: not indexed
			}
			content := string(blob)
			changed, err := s.store.Upsert(ctx, &StateEntry{
				ProjectID: projectID, Path: e.Path, BlobSHA: e.SHA,
				Title: ExtractTitle(content), Content: content,
			})
			if err != nil {
				return err
			}
			if changed {
				stats.Indexed++
			}
		}
		return nil
	}
	if err := walk(""); err != nil {
		return nil, err
	}
	// Rebuild the wiki-link index (backlinks) from every indexed markdown file.
	if err := s.rebuildLinks(ctx, repo, revision, projectID); err != nil {
		return nil, err
	}
	// Remove indexed paths that no longer exist in the tree.
	existing, err := s.indexedPaths(ctx, projectID)
	if err != nil {
		return nil, err
	}
	for _, p := range existing {
		if !seen[p] {
			if err := s.store.Delete(ctx, projectID, p); err != nil {
				return nil, err
			}
			stats.Removed++
		}
	}
	if err := s.store.SetProjectState(ctx, ProjectIndexState{
		ProjectID: projectID, Revision: revision, Status: "clean",
		UpdatedAt: s.clock.Now().UTC().Format(time.RFC3339),
	}); err != nil {
		return nil, err
	}
	return stats, nil
}

// ReindexAll rebuilds every project (used by the CLI).
func (s *Service) ReindexAll(ctx context.Context) (map[string]*ReindexStats, error) {
	projects, err := s.projects.List(ctx)
	if err != nil {
		return nil, err
	}
	out := map[string]*ReindexStats{}
	for _, p := range projects {
		stats, err := s.ReindexProject(ctx, p.ID)
		if err != nil {
			return nil, fmt.Errorf("%s: %w", p.ID, err)
		}
		out[p.ID] = stats
	}
	return out, nil
}

// rebuildLinks scans all markdown sources for [[wiki links]] and stores the
// (source -> targets) mapping used by the backlinks endpoint.
func (s *Service) rebuildLinks(ctx context.Context, repo *project.Repo, branch, projectID string) error {
	links := map[string][]string{}
	var walk func(dir string) error
	walk = func(dir string) error {
		entries, err := repo.ListTree(ctx, branch, dir)
		if err != nil {
			return err
		}
		for _, e := range entries {
			if e.Type == "tree" {
				if err := walk(e.Path); err != nil {
					return err
				}
				continue
			}
			if !strings.HasSuffix(e.Path, ".md") && !strings.HasSuffix(e.Path, ".markdown") {
				continue
			}
			blob, err := repo.ReadBlob(ctx, branch, e.Path)
			if err != nil || len(blob) > s.maxBlob {
				continue
			}
			content := string(blob)
			seen := map[string]bool{}
			for _, m := range wikiLinkPattern.FindAllStringSubmatch(content, -1) {
				target := m[1]
				if !seen[target] {
					seen[target] = true
					links[e.Path] = append(links[e.Path], target)
				}
			}
		}
		return nil
	}
	if err := walk(""); err != nil {
		return err
	}
	return s.store.ReplaceLinks(ctx, projectID, links)
}

// Backlinks returns pages that link to targetPath within the project.
func (s *Service) Backlinks(ctx context.Context, projectID, targetPath string) ([]Backlink, error) {
	return s.store.Backlinks(ctx, projectID, targetPath)
}

func (s *Service) indexedPaths(ctx context.Context, projectID string) ([]string, error) {
	rows, err := s.store.db.QueryContext(ctx,
		`SELECT path FROM doc_index_state WHERE project_id = ?`, projectID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []string
	for rows.Next() {
		var p string
		if err := rows.Scan(&p); err != nil {
			return nil, err
		}
		out = append(out, p)
	}
	return out, rows.Err()
}
