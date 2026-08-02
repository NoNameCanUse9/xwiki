package project

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"agentdocs/internal/platform/clock"
	"agentdocs/internal/platform/id"
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

// Get returns one project by id.
func (s *Service) Get(ctx context.Context, projectID string) (*Project, error) {
	return s.store.GetByID(ctx, projectID)
}

// Archive marks a project archived. The operation is idempotent: archiving an
// already archived project succeeds and keeps the original timestamp.
func (s *Service) Archive(ctx context.Context, projectID string) (*Project, error) {
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
	now := s.clock.Now().UTC()
	if err := s.store.Unarchive(ctx, projectID, now); err != nil {
		return nil, err
	}
	return s.store.GetByID(ctx, projectID)
}
