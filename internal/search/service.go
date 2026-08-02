package search

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"agentdocs/internal/platform/clock"
	"agentdocs/internal/project"
)

// ReindexStats reports what a reindex pass changed.
type ReindexStats struct {
	Indexed int `json:"indexed"`
	Removed int `json:"removed"`
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
	return s.store.Query(ctx, projectID, BuildMatchExpr(q), limit)
}

// ReindexProject incrementally syncs the index with the project's current
// Git tree: changed blobs are upserted, vanished paths are removed.
func (s *Service) ReindexProject(ctx context.Context, projectID string) (*ReindexStats, error) {
	repo, err := s.projects.OpenRepo(ctx, projectID)
	if err != nil {
		return nil, err
	}
	branch, err := repo.DefaultBranch(ctx)
	if err != nil {
		return nil, err
	}
	stats := &ReindexStats{}
	seen := map[string]bool{}

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
			seen[e.Path] = true
			blob, err := repo.ReadBlob(ctx, branch, e.Path)
			if err != nil {
				continue // unreadable blobs are skipped
			}
			if len(blob) > s.maxBlob || strings.ContainsRune(string(blob), '\x00') {
				continue // binary or oversized: not indexed
			}
			sha, err := blobSHA(repo, e.Path, blob)
			if err != nil {
				return err
			}
			changed, err := s.store.Upsert(ctx, &StateEntry{
				ProjectID: projectID, Path: e.Path, BlobSHA: sha, Content: string(blob),
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

func blobSHA(repo *project.Repo, path string, content []byte) (string, error) {
	// Reuse the repo's object store: write the blob and read its id.
	sha, err := repo.HashBlob(context.Background(), content)
	if err != nil {
		return "", err
	}
	_ = path
	return sha, nil
}
