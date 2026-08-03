package project

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"os"
	"os/exec"
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

// ImportRepo clones a remote repository URL into a new project (bare).
func (s *Service) ImportRepo(ctx context.Context, name, url string) (*BundleImportResult, error) {
	if err := ValidateName(name); err != nil {
		return nil, ErrInvalid
	}
	if !validRepoURL(url) {
		return nil, ErrInvalid
	}
	projectID := id.New("prj")
	now := s.clock.Now().UTC()
	repoDir := filepath.Join(s.reposRoot, projectID, "repo.git")
	if err := os.MkdirAll(filepath.Dir(repoDir), 0o755); err != nil {
		return nil, err
	}
	cleanup := func() { _ = os.RemoveAll(filepath.Dir(repoDir)) }
	cmd := exec.CommandContext(ctx, "git", "clone", "--bare", "--", url, repoDir)
	cmd.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1")
	if out, err := cmd.CombinedOutput(); err != nil {
		cleanup()
		return nil, fmt.Errorf("clone %s: %w: %s", url, err, strings.TrimSpace(string(out)))
	}
	// Normalize the default branch to main when the remote used another name.
	if _, err := gitOutput(ctx, repoDir, "rev-parse", "--verify", "refs/heads/main"); err != nil {
		head, err := gitOutput(ctx, repoDir, "for-each-ref", "--format=%(objectname)", "refs/heads/")
		if err != nil || head == "" {
			cleanup()
			return nil, fmt.Errorf("repository has no branches")
		}
		branchName, _ := gitOutput(ctx, repoDir, "for-each-ref", "--format=%(refname:short)", "refs/heads/")
		first := strings.SplitN(branchName, "\n", 2)[0]
		if _, err := gitOutput(ctx, repoDir, "update-ref", "refs/heads/main", head, first); err != nil {
			cleanup()
			return nil, err
		}
	}
	p := &Project{
		ID: projectID, Name: name,
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

// UploadedFile is one file entry in a folder import payload.
type UploadedFile struct {
	Path    string `json:"path"`
	Content []byte `json:"-"` // raw bytes, not JSON-serializable
}

// ImportFolderInput carries the user-supplied fields for ImportFolder.
type ImportFolderInput struct {
	Name        string
	Description string
	Files       []UploadedFile
}

// ImportFolderResult reports a folder import outcome.
type ImportFolderResult struct {
	Project *Project `json:"project"`
	Commits int      `json:"commits"`
}

// ImportFolder creates a new project from an uploaded folder. If any uploaded
// file has a path under .git/, the original git history is preserved; otherwise
// all files are committed as a fresh initial snapshot.
func (s *Service) ImportFolder(ctx context.Context, input ImportFolderInput) (*ImportFolderResult, error) {
	if err := ValidateName(input.Name); err != nil {
		return nil, err
	}
	if len(input.Files) == 0 {
		return nil, ErrInvalid
	}

	projectID := id.New("prj")
	now := s.clock.Now().UTC()
	repoDir := filepath.Join(s.reposRoot, projectID, "repo.git")
	if err := os.MkdirAll(filepath.Dir(repoDir), 0o755); err != nil {
		return nil, err
	}
	cleanup := func() { _ = os.RemoveAll(filepath.Dir(repoDir)) }

	// Detect whether uploaded files include .git internals.
	hasGit := false
	for _, f := range input.Files {
		if hasGitSegment(f.Path) {
			hasGit = true
			break
		}
	}

	// Build a temporary non-bare repository, write all files, then convert
	// to bare and move into the target location.
	tmpDir, err := os.MkdirTemp("", "agentdocs-folder-import-*")
	if err != nil {
		return nil, fmt.Errorf("create temp dir: %w", err)
	}
	defer os.RemoveAll(tmpDir)

	if hasGit {
		// Write all files (including .git/) into the temp directory.
		for _, f := range input.Files {
			if !validateDocPathInternal(f.Path) {
				cleanup()
				return nil, fmt.Errorf("invalid path in uploaded folder: %q", f.Path)
			}
			fp := filepath.Join(tmpDir, filepath.FromSlash(f.Path))
			if err := os.MkdirAll(filepath.Dir(fp), 0o755); err != nil {
				cleanup()
				return nil, err
			}
			if err := os.WriteFile(fp, f.Content, 0o644); err != nil {
				cleanup()
				return nil, err
			}
		}
		// Verify the repo is usable.
		if _, err := gitOutputIn(ctx, tmpDir, "status"); err != nil {
			cleanup()
			return nil, fmt.Errorf("invalid git repository in uploaded folder: %w", err)
		}
	} else {
		// No .git — init, write, commit.
		if _, err := gitOutputPlain(ctx, "init", "--initial-branch=main", tmpDir); err != nil {
			cleanup()
			return nil, fmt.Errorf("git init: %w", err)
		}
		for _, f := range input.Files {
			if !validateDocPathInternal(f.Path) {
				cleanup()
				return nil, fmt.Errorf("invalid path in uploaded folder: %q", f.Path)
			}
			fp := filepath.Join(tmpDir, filepath.FromSlash(f.Path))
			if err := os.MkdirAll(filepath.Dir(fp), 0o755); err != nil {
				cleanup()
				return nil, err
			}
			if err := os.WriteFile(fp, f.Content, 0o644); err != nil {
				cleanup()
				return nil, err
			}
		}
		if _, err := gitOutputIn(ctx, tmpDir, "add", "-A"); err != nil {
			cleanup()
			return nil, fmt.Errorf("git add: %w", err)
		}
		if _, err := gitOutputIn(ctx, tmpDir, "commit", "-m", "Initial import from folder"); err != nil {
			cleanup()
			return nil, fmt.Errorf("git commit: %w", err)
		}
	}

	// Convert the temp repo to a bare repo at the target location.
	if _, err := gitOutput(ctx, repoDir, "init", "--bare", "--initial-branch=main", repoDir); err != nil {
		cleanup()
		return nil, fmt.Errorf("init bare: %w", err)
	}
	if _, err := gitOutput(ctx, repoDir, "fetch", tmpDir, "refs/heads/*:refs/heads/*"); err != nil {
		cleanup()
		return nil, fmt.Errorf("fetch into bare: %w", err)
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

	commits := 0
	if out, err := gitOutput(ctx, repoDir, "rev-list", "--count", "HEAD"); err == nil {
		fmt.Sscanf(out, "%d", &commits)
	}
	return &ImportFolderResult{Project: p, Commits: commits}, nil
}

// hasGitSegment reports whether a slash-separated path contains a .git
// directory segment (either at the root or nested).
func hasGitSegment(path string) bool {
	for _, seg := range strings.Split(path, "/") {
		if seg == ".git" {
			return true
		}
	}
	return false
}

// validRepoURL restricts import sources to git-capable protocols.
func validRepoURL(u string) bool {
	for _, prefix := range []string{"http://", "https://", "git://", "ssh://", "file://"} {
		if strings.HasPrefix(u, prefix) {
			return true
		}
	}
	// scp-like syntax: user@host:path
	if strings.Contains(u, "@") && strings.Contains(u, ":") && !strings.HasPrefix(u, "/") {
		return true
	}
	return false
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
