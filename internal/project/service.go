package project

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"xwiki/internal/platform/clock"
	"xwiki/internal/platform/id"
)

// Service coordinates project metadata (Store) with on-disk bare Git
// repositories (Repo). Every project owns exactly one bare repository.
type Service struct {
	store     *Store
	reposRoot string // absolute path to <dataDir>/repos
	clock     clock.Clock
}

// CreateInput carries the user-supplied fields for Create.
type CreateInput struct {
	Name        string
	Description string
}

// NewService builds the project service. dataDir is the application data
// directory; repositories live under <dataDir>/repos.
func NewService(db *sql.DB, dataDir string, clk clock.Clock) *Service {
	return &Service{
		store:     NewStore(db),
		reposRoot: filepath.Join(dataDir, "repos"),
		clock:     clk,
	}
}

// Create validates the name, initializes the project's bare repository with a
// README root commit, then persists the metadata. On any failure no partial
// record survives: the repository directory is removed when the store insert
// fails, and nothing is inserted when repository setup fails.
func (s *Service) Create(ctx context.Context, input CreateInput) (*Project, error) {
	if err := ValidateName(input.Name); err != nil {
		return nil, err
	}
	projectID := id.New("prj")
	now := s.clock.Now().UTC()

	repoDir := filepath.Join(s.reposRoot, projectID, "repo.git")
	repo, err := InitBare(ctx, s.reposRoot, projectID)
	if err != nil {
		return nil, fmt.Errorf("init repository: %w", err)
	}
	cleanup := func() { _ = os.RemoveAll(filepath.Dir(repoDir)) }

	if err := repo.WriteReadme(ctx, input.Name, input.Description, now); err != nil {
		cleanup()
		return nil, fmt.Errorf("initialize readme: %w", err)
	}

	p := &Project{
		ID:          projectID,
		Name:        input.Name,
		Description: input.Description,
		RepoDir:     filepath.ToSlash(filepath.Join("repos", projectID, "repo.git")),
		CreatedAt:   now,
		UpdatedAt:   now,
	}
	if err := s.store.Create(ctx, p); err != nil {
		cleanup()
		return nil, err
	}
	return p, nil
}

// ReposRoot exposes the absolute repositories root (used by tests).
func (s *Service) ReposRoot() string { return s.reposRoot }

// OpenRepo resolves a project's bare repository by id.
func (s *Service) OpenRepo(ctx context.Context, projectID string) (*Repo, error) {
	p, err := s.store.GetByID(ctx, projectID)
	if err != nil {
		return nil, err
	}
	return &Repo{Dir: filepath.Join(s.reposRoot, p.ID, "repo.git")}, nil
}

// List returns every project, newest first (archived included).
func (s *Service) List(ctx context.Context) ([]*Project, error) {
	return s.store.List(ctx)
}

func (s *Service) ListDeleted(ctx context.Context) ([]*Project, error) {
	return s.store.ListDeleted(ctx)
}

// Get returns one project by id.
func (s *Service) Get(ctx context.Context, projectID string) (*Project, error) {
	return s.store.GetByID(ctx, projectID)
}

// Archive marks a project archived. The operation is idempotent: archiving an
// already archived project succeeds and keeps the original timestamp.
func (s *Service) Archive(ctx context.Context, projectID string) (*Project, error) {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	now := s.clock.Now().UTC()
	if err := s.store.Archive(ctx, projectID, now); err != nil {
		return nil, err
	}
	p, err := s.store.GetByID(ctx, projectID)
	if err != nil {
		if errors.Is(err, ErrNotFound) {
			return nil, ErrNotFound
		}
		return nil, err
	}
	return p, nil
}

// Unarchive restores an archived project (idempotent).
func (s *Service) Unarchive(ctx context.Context, projectID string) (*Project, error) {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	now := s.clock.Now().UTC()
	if err := s.store.Unarchive(ctx, projectID, now); err != nil {
		return nil, err
	}
	return s.store.GetByID(ctx, projectID)
}

// RenameInput carries the user-supplied name for Rename.
type RenameInput struct {
	Name string
}

// Rename updates a project's name in metadata and refreshes the README
// headline in the repository with a new commit, keeping the two in sync.
func (s *Service) Rename(ctx context.Context, projectID string, input RenameInput) (*Project, error) {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	if err := ValidateName(input.Name); err != nil {
		return nil, ErrInvalid
	}
	p, err := s.store.GetByID(ctx, projectID)
	if err != nil {
		return nil, err
	}
	if p.Name == input.Name {
		return p, nil
	}
	now := s.clock.Now().UTC()
	// Write Git first while holding the shared project mutation lock. Metadata
	// is changed only after Git succeeds, so callers never observe split state.
	repo := &Repo{Dir: filepath.Join(s.reposRoot, p.ID, "repo.git")}
	if err := repo.RewriteReadmeTitle(ctx, input.Name); err != nil {
		return nil, fmt.Errorf("rewrite readme title: %w", err)
	}
	if err := s.store.Rename(ctx, projectID, input.Name, now); err != nil {
		// Best-effort compensation restores the public repository title.
		_ = repo.RewriteReadmeTitle(context.Background(), p.Name)
		return nil, err
	}
	return s.store.GetByID(ctx, projectID)
}

// Delete moves a project to the recoverable deleted-project view. The
// metadata row is soft-deleted (deleted_at set) and the repository stays in
// place, so RestoreDeleted can bring the project back without disk moves.
func (s *Service) Delete(ctx context.Context, projectID string) error {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	return s.store.SoftDelete(ctx, projectID, s.clock.Now().UTC())
}

func (s *Service) RestoreDeleted(ctx context.Context, projectID string) (*Project, error) {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	if _, err := s.store.GetDeletedByID(ctx, projectID); err != nil {
		return nil, err
	}
	if err := s.store.RestoreDeleted(ctx, projectID, s.clock.Now().UTC()); err != nil {
		return nil, err
	}
	return s.store.GetByID(ctx, projectID)
}

// PurgeDeleted permanently removes a soft-deleted project: metadata row and
// the on-disk repository. The repository is staged in a trash directory
// first so a failed metadata delete can roll the repo back.
func (s *Service) PurgeDeleted(ctx context.Context, projectID string) error {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	p, err := s.store.GetDeletedByID(ctx, projectID)
	if errors.Is(err, ErrNotFound) {
		if _, activeErr := s.store.GetByID(ctx, projectID); activeErr == nil {
			return ErrNotDeleted
		}
		return ErrNotFound
	}
	if err != nil {
		return err
	}
	projectDir := filepath.Join(s.reposRoot, p.ID)
	trashRoot := filepath.Join(filepath.Dir(s.reposRoot), "trash")
	if err := os.MkdirAll(trashRoot, 0o755); err != nil {
		return err
	}
	staged := filepath.Join(trashRoot, p.ID+"-purging")
	if err := os.Rename(projectDir, staged); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("stage repository purge: %w", err)
	}
	if err := s.store.Delete(ctx, projectID); err != nil {
		_ = os.Rename(staged, projectDir)
		return err
	}
	if err := os.RemoveAll(staged); err != nil {
		return fmt.Errorf("remove staged repository: %w", err)
	}
	return nil
}

// Purge rewrites the project's git history to remove the given paths
// completely (hard delete, irreversible). It requires an existing project.
func (s *Service) Purge(ctx context.Context, projectID string, paths []string, message string) error {
	unlock := LockProjectWrite(projectID)
	defer unlock()
	p, err := s.store.GetByID(ctx, projectID)
	if err != nil {
		return err
	}
	if p.IsArchived() {
		return ErrArchived
	}
	repo := &Repo{Dir: filepath.Join(s.reposRoot, p.ID, "repo.git")}
	return repo.PurgePaths(ctx, paths)
}
