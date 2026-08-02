package project

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// CommitSummary is one row of the commit log.
type CommitSummary struct {
	SHA     string `json:"sha"`
	Message string `json:"message"`
	Author  string `json:"author"`
	Date    string `json:"date"`
}

// CommitDetail is a commit with its changed file list.
type CommitDetail struct {
	SHA     string         `json:"sha"`
	Message string         `json:"message"`
	Author  string         `json:"author"`
	Date    string         `json:"date"`
	Files   []FileChange   `json:"files"`
}

// FileChange is one changed path inside a commit.
type FileChange struct {
	Status string `json:"status"` // A | M | D | R
	Path   string `json:"path"`
}

// DiffStat is the machine-readable numstat for one path.
type DiffStat struct {
	Path    string `json:"path"`
	Added   int    `json:"added"`
	Deleted int    `json:"deleted"`
}

// CommitDiff is the machine-readable diff of one commit.
type CommitDiff struct {
	SHA   string     `json:"sha"`
	Stats []DiffStat `json:"stats"`
	Patch string     `json:"patch,omitempty"`
}

// maxDiffBytes caps rendered patch output.
const maxDiffBytes = 1 << 20 // 1 MiB

// ListCommits returns commit summaries, newest first.
func (s *Service) ListCommits(ctx context.Context, projectID string, limit, offset int) ([]CommitSummary, error) {
	if limit <= 0 {
		limit = 20
	}
	if limit > 100 {
		limit = 100
	}
	repo, err := s.openRepoChecked(ctx, projectID)
	if err != nil {
		return nil, err
	}
	branch, err := repo.DefaultBranch(ctx)
	if err != nil {
		return nil, err
	}
	out, err := gitOutput(ctx, repo.Dir, "log",
		"--format=%H%x1f%s%x1f%an%x1f%aI",
		"-n", fmt.Sprintf("%d", limit),
		"--skip", fmt.Sprintf("%d", offset),
		branch)
	if err != nil {
		return nil, fmt.Errorf("log: %w", err)
	}
	return parseCommitSummaries(out), nil
}

func parseCommitSummaries(out string) []CommitSummary {
	var commits []CommitSummary
	for _, line := range strings.Split(out, "\n") {
		if line == "" {
			continue
		}
		parts := strings.Split(line, "\x1f")
		if len(parts) < 4 {
			continue
		}
		commits = append(commits, CommitSummary{
			SHA: parts[0], Message: parts[1], Author: parts[2], Date: parts[3],
		})
	}
	return commits
}

// GetCommit returns one commit with its changed files.
func (s *Service) GetCommit(ctx context.Context, projectID, sha string) (*CommitDetail, error) {
	repo, err := s.openRepoChecked(ctx, projectID)
	if err != nil {
		return nil, err
	}
	out, err := gitOutput(ctx, repo.Dir, "show",
		"--format=%H%x1f%s%x1f%an%x1f%aI", "--name-status", "--no-renames", sha)
	if err != nil {
		return nil, ErrNotFound
	}
	lines := strings.Split(out, "\n")
	if len(lines) == 0 {
		return nil, ErrNotFound
	}
	meta := strings.Split(lines[0], "\x1f")
	if len(meta) < 4 {
		return nil, fmt.Errorf("unexpected show output")
	}
	detail := &CommitDetail{SHA: meta[0], Message: meta[1], Author: meta[2], Date: meta[3]}
	for _, line := range lines[1:] {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) < 2 {
			continue
		}
		status := fields[0]
		p := strings.Join(fields[1:], " ")
		// Rename entries "old => new" keep the final path for display.
		if strings.Contains(p, " => ") {
			p = p[strings.LastIndex(p, " => ")+4:]
		}
		detail.Files = append(detail.Files, FileChange{Status: status, Path: p})
	}
	return detail, nil
}

// FileHistory returns commits that touched a path, newest first.
func (s *Service) FileHistory(ctx context.Context, projectID, filePath string) ([]CommitSummary, error) {
	if !validateDocPathInternal(filePath) {
		return nil, ErrInvalid
	}
	repo, err := s.openRepoChecked(ctx, projectID)
	if err != nil {
		return nil, err
	}
	out, err := gitOutput(ctx, repo.Dir, "log",
		"--format=%H%x1f%s%x1f%an%x1f%aI", "--follow", "--", filePath)
	if err != nil {
		return nil, fmt.Errorf("log --follow: %w", err)
	}
	return parseCommitSummaries(out), nil
}

// CommitDiff returns numstat stats and optionally the full patch of a commit.
func (s *Service) CommitDiff(ctx context.Context, projectID, sha, format string) (*CommitDiff, error) {
	if format != "numstat" && format != "patch" {
		return nil, ErrInvalid
	}
	repo, err := s.openRepoChecked(ctx, projectID)
	if err != nil {
		return nil, err
	}
	numstat, err := gitOutput(ctx, repo.Dir, "show", "--format=", "--numstat", sha)
	if err != nil {
		return nil, ErrNotFound
	}
	diff := &CommitDiff{SHA: sha}
	for _, line := range strings.Split(numstat, "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		fields := strings.Fields(line)
		if len(fields) < 3 {
			continue
		}
		var added, deleted int
		fmt.Sscanf(fields[0], "%d", &added)
		fmt.Sscanf(fields[1], "%d", &deleted)
		diff.Stats = append(diff.Stats, DiffStat{Path: strings.Join(fields[2:], " "), Added: added, Deleted: deleted})
	}
	if format == "patch" {
		patch, err := gitOutput(ctx, repo.Dir, "show", "--format=", "--no-color", "--find-renames", sha)
		if err != nil {
			return nil, ErrNotFound
		}
		if len(patch) > maxDiffBytes {
			return nil, fmt.Errorf("diff too large")
		}
		diff.Patch = patch
	}
	return diff, nil
}

// RevertCommit creates a new commit that reverts the given one. The original
// history is preserved; the revert is appended. Conflicts fail with
// ErrConflict and nothing is written.
func (s *Service) RevertCommit(ctx context.Context, projectID, sha string, message string) (*CommitSummary, error) {
	p, err := s.store.GetByID(ctx, projectID)
	if err != nil {
		return nil, err
	}
	if p.IsArchived() {
		return nil, ErrArchived
	}
	mu, _ := projectLocks.LoadOrStore(p.ID, &sync.Mutex{})
	mu.(*sync.Mutex).Lock()
	defer mu.(*sync.Mutex).Unlock()

	repo := &Repo{Dir: filepath.Join(s.reposRoot, p.ID, "repo.git")}
	branch, err := repo.DefaultBranch(ctx)
	if err != nil {
		return nil, err
	}
	current, err := repo.Revision(ctx)
	if err != nil {
		return nil, err
	}
	// Verify the target commit exists.
	if _, err := gitOutput(ctx, repo.Dir, "cat-file", "-e", sha+"^{commit}"); err != nil {
		return nil, ErrNotFound
	}

	wtDir, err := os.MkdirTemp("", "agentdocs-wt-*")
	if err != nil {
		return nil, err
	}
	cleanup := func() {
		_, _ = gitOutput(context.Background(), repo.Dir, "worktree", "remove", "--force", wtDir)
		_ = os.RemoveAll(wtDir)
	}
	if _, err := gitOutput(ctx, repo.Dir, "worktree", "add", "--detach", wtDir, branch); err != nil {
		cleanup()
		return nil, fmt.Errorf("add worktree: %w", err)
	}

	// Generate the reverse patch and preflight it. Raw output is required:
	// trimming would corrupt patch whitespace semantics.
	patch, err := gitOutputRaw(ctx, repo.Dir, "show", "--format=", "--no-color", "--find-renames", "--binary", sha)
	if err != nil {
		cleanup()
		return nil, ErrNotFound
	}
	// The patch file lives OUTSIDE the worktree so `git add -A` never commits it.
	patchDir, err := os.MkdirTemp("", "agentdocs-patch-*")
	if err != nil {
		cleanup()
		return nil, err
	}
	defer os.RemoveAll(patchDir)
	patchFile := filepath.Join(patchDir, "revert.patch")
	if err := os.WriteFile(patchFile, []byte(patch), 0o600); err != nil {
		cleanup()
		return nil, err
	}
	if _, err := gitOutputIn(ctx, wtDir, "apply", "--reverse", "--check", patchFile); err != nil {
		cleanup()
		return nil, fmt.Errorf("apply check: %w", err)
	}
	if _, err := gitOutputIn(ctx, wtDir, "apply", "--reverse", patchFile); err != nil {
		cleanup()
		return nil, ErrConflict
	}
	if _, err := gitOutputIn(ctx, wtDir, "add", "-A"); err != nil {
		cleanup()
		return nil, fmt.Errorf("git add: %w", err)
	}
	tree, err := gitOutputIn(ctx, wtDir, "write-tree")
	if err != nil {
		cleanup()
		return nil, fmt.Errorf("write-tree: %w", err)
	}
	if message == "" {
		message = fmt.Sprintf("Revert %q", shortSHA(sha))
	}
	commit, err := gitOutput(ctx, repo.Dir, "commit-tree", tree, "-p", current, "-m", message)
	if err != nil {
		cleanup()
		return nil, fmt.Errorf("commit-tree: %w", err)
	}
	if _, err := gitOutput(ctx, repo.Dir, "update-ref", "refs/heads/"+branch, commit, current); err != nil {
		cleanup()
		return nil, ErrConflict
	}
	cleanup()
	return &CommitSummary{SHA: commit, Message: message, Author: "AgentDocs"}, nil
}

func (s *Service) openRepoChecked(ctx context.Context, projectID string) (*Repo, error) {
	repo, err := s.OpenRepo(ctx, projectID)
	if err != nil {
		return nil, err
	}
	return repo, nil
}

func shortSHA(sha string) string {
	if len(sha) > 8 {
		return sha[:8]
	}
	return sha
}
