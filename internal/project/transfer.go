package project

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"agentdocs/internal/platform/id"
)

// ZipFile is one entry of an import payload.
type ZipFile struct {
	Path    string `json:"path"`
	Content string `json:"content"` // base64
}

// ImportZipInput mirrors a ZIP import request.
type ImportZipInput struct {
	BaseRevision string    `json:"base_revision"`
	Message      string    `json:"message"`
	Files        []ZipFile `json:"files"`
}

// ImportResult reports an import outcome.
type ImportResult struct {
	Commit   string `json:"commit"`
	Revision string `json:"revision"`
	Imported int    `json:"imported"`
}

// MaxImportFileBytes caps a single imported/attached file (5 MiB).
const MaxImportFileBytes = 5 << 20

// ExportZip snapshots the whole project tree into a ZIP in memory.
func (s *Service) ExportZip(ctx context.Context, projectID string) ([]byte, error) {
	repo, err := s.OpenRepo(ctx, projectID)
	if err != nil {
		return nil, err
	}
	branch, err := repo.DefaultBranch(ctx)
	if err != nil {
		return nil, err
	}
	var buf bytes.Buffer
	zw := zip.NewWriter(&buf)
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
			blob, err := repo.ReadBlob(ctx, branch, e.Path)
			if err != nil {
				return err
			}
			fw, err := zw.Create(e.Path)
			if err != nil {
				return err
			}
			if _, err := fw.Write(blob); err != nil {
				return err
			}
		}
		return nil
	}
	if err := walk(""); err != nil {
		_ = zw.Close()
		return nil, err
	}
	if err := zw.Close(); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// ImportZip applies an import payload as one changeset commit.
func (s *Service) ImportZip(ctx context.Context, projectID string, input ImportZipInput) (*ImportResult, error) {
	if len(input.Files) == 0 || len(input.Files) > 500 {
		return nil, ErrInvalid
	}
	changes := make([]Change, 0, len(input.Files))
	for _, f := range input.Files {
		if !validateDocPathInternal(f.Path) {
			return nil, ErrInvalid
		}
		raw, err := base64.StdEncoding.DecodeString(f.Content)
		if err != nil {
			return nil, ErrInvalid
		}
		if len(raw) > MaxImportFileBytes {
			return nil, ErrInvalid
		}
		changes = append(changes, Change{
			Op: "create", Path: f.Path, Content: string(raw),
		})
	}
	message := input.Message
	if message == "" {
		message = "Import zip snapshot"
	}
	res, err := s.ApplyChangeset(ctx, projectID, ChangesetInput{
		BaseRevision: input.BaseRevision,
		Message:      message,
		Changes:      changes,
	}, CommitAuthor{Name: "import", Email: "import@agentdocs.local"})
	if err != nil {
		return nil, err
	}
	return &ImportResult{Commit: res.Commit, Revision: res.Revision, Imported: len(changes)}, nil
}

// ExportBundle serializes the whole repository as a git bundle.
func (s *Service) ExportBundle(ctx context.Context, projectID string) ([]byte, error) {
	repo, err := s.OpenRepo(ctx, projectID)
	if err != nil {
		return nil, err
	}
	tmp, err := os.CreateTemp("", "agentdocs-bundle-*.bundle")
	if err != nil {
		return nil, err
	}
	tmpPath := tmp.Name()
	defer os.Remove(tmpPath)
	if err := tmp.Close(); err != nil {
		return nil, err
	}
	if _, err := gitOutput(ctx, repo.Dir, "bundle", "create", tmpPath, "--all"); err != nil {
		return nil, fmt.Errorf("bundle create: %w", err)
	}
	return os.ReadFile(tmpPath)
}

// BundleImportResult reports a bundle import.
type BundleImportResult struct {
	Project *Project `json:"project"`
	Commits int      `json:"commits"`
}

// ImportBundleInput carries the bundle bytes and target project name.
type ImportBundleInput struct {
	Name   string
	Bundle []byte
}

// ImportBundle creates a new project whose repository is seeded from a git
// bundle (full history preserved).
func (s *Service) ImportBundle(ctx context.Context, input ImportBundleInput) (*BundleImportResult, error) {
	if err := ValidateName(input.Name); err != nil {
		return nil, err
	}
	if len(input.Bundle) == 0 {
		return nil, ErrInvalid
	}
	bundleFile, err := os.CreateTemp("", "agentdocs-in-*.bundle")
	if err != nil {
		return nil, err
	}
	bundlePath := bundleFile.Name()
	defer os.Remove(bundlePath)
	if _, err := bundleFile.Write(input.Bundle); err != nil {
		_ = bundleFile.Close()
		return nil, err
	}
	if err := bundleFile.Close(); err != nil {
		return nil, err
	}
	if _, err := gitOutputPlain(context.Background(), "bundle", "verify", bundlePath); err != nil {
		return nil, ErrInvalid // not a valid bundle
	}

	projectID := id.New("prj")
	now := s.clock.Now().UTC()
	repoDir := filepath.Join(s.reposRoot, projectID, "repo.git")
	if err := os.MkdirAll(filepath.Dir(repoDir), 0o755); err != nil {
		return nil, err
	}
	if _, err := gitOutput(ctx, repoDir, "init", "--bare", "--initial-branch=main", repoDir); err != nil {
		return nil, fmt.Errorf("init: %w", err)
	}
	cleanup := func() { _ = os.RemoveAll(filepath.Dir(repoDir)) }
	// fetch creates refs (unbundle alone only writes objects).
	if _, err := gitOutput(ctx, repoDir, "fetch", bundlePath, "refs/heads/*:refs/heads/*"); err != nil {
		cleanup()
		return nil, fmt.Errorf("fetch bundle: %w", err)
	}
	head, err := gitOutput(ctx, repoDir, "rev-parse", "--verify", "refs/heads/main")
	if err != nil {
		// Bundle may use a different branch name; normalize to main.
		head, err = gitOutput(ctx, repoDir, "for-each-ref", "--format=%(objectname)", "refs/heads/")
		if err != nil || head == "" {
			cleanup()
			return nil, fmt.Errorf("bundle has no branches")
		}
		branchName, _ := gitOutput(ctx, repoDir, "for-each-ref", "--format=%(refname:short)", "refs/heads/")
		first := strings.SplitN(branchName, "\n", 2)[0]
		if _, err := gitOutput(ctx, repoDir, "update-ref", "refs/heads/main", head, first); err != nil {
			cleanup()
			return nil, err
		}
	}
	_ = head
	p := &Project{
		ID: projectID, Name: input.Name,
		RepoDir:   filepath.ToSlash(filepath.Join("repos", projectID, "repo.git")),
		CreatedAt: now, UpdatedAt: now,
	}
	if err := s.store.Create(ctx, p); err != nil {
		cleanup()
		return nil, err
	}
	commits := 0
	if out, err := gitOutput(ctx, repoDir, "rev-list", "--count", "HEAD"); err == nil {
		fmt.Sscanf(out, "%d", &commits)
	}
	return &BundleImportResult{Project: p, Commits: commits}, nil
}
