package project

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// Repo is one project's bare Git repository on disk.
type Repo struct {
	Dir string // absolute path to the bare repo
}

// InitBare creates `<root>/<projectID>/repo.git` as a bare repository and
// returns the Repo handle. The parent directory is created on demand.
func InitBare(ctx context.Context, root, projectID string) (*Repo, error) {
	dir := filepath.Join(root, projectID, "repo.git")
	if err := os.MkdirAll(filepath.Dir(dir), 0o755); err != nil {
		return nil, fmt.Errorf("create repo parent: %w", err)
	}
	if _, err := gitOutput(ctx, dir, "init", "--bare", "--initial-branch=main", dir); err != nil {
		return nil, fmt.Errorf("git init --bare: %w", err)
	}
	return &Repo{Dir: dir}, nil
}

// WriteReadme creates the repository's root commit containing a README.md
// generated from the project metadata. It uses plumbing commands only, so no
// working tree or checkout is required.
func (r *Repo) WriteReadme(ctx context.Context, name, description string, now time.Time) error {
	readme := fmt.Sprintf("# %s\n\n%s\n\n---\n\nAgentDocs 项目 · %s\n",
		name, description, now.UTC().Format(time.RFC3339))

	tree, err := r.mkTree(ctx, readme)
	if err != nil {
		return fmt.Errorf("mktree: %w", err)
	}
	commit, err := gitOutput(ctx, r.Dir, "commit-tree", tree, "-m", "Initialize project "+name)
	if err != nil {
		return fmt.Errorf("commit-tree: %w", err)
	}
	if _, err := gitOutput(ctx, r.Dir, "update-ref", "refs/heads/main", strings.TrimSpace(commit)); err != nil {
		return fmt.Errorf("update-ref: %w", err)
	}
	return nil
}

// mkTree writes the README blob and builds the root tree via mktree.
func (r *Repo) mkTree(ctx context.Context, readme string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "--git-dir", r.Dir, "hash-object", "-w", "--stdin")
	cmd.Stdin = strings.NewReader(readme)
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &out
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("%v: %w", cmd.Args, err)
	}
	blob := strings.TrimSpace(out.String())
	if blob == "" {
		return "", fmt.Errorf("hash-object returned empty blob id")
	}

	out.Reset()
	cmd = exec.CommandContext(ctx, "git", "--git-dir", r.Dir, "mktree")
	cmd.Stdin = strings.NewReader(fmt.Sprintf("100644 blob %s\tREADME.md\n", blob))
	cmd.Stdout = &out
	cmd.Stderr = &out
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("%v: %w", cmd.Args, err)
	}
	return strings.TrimSpace(out.String()), nil
}

// gitOutput runs a git command against a repo with deterministic identity
// environment and returns trimmed stdout. With --git-dir set, commands that
// need a working tree (rare here) are run against the bare repo itself.
func gitOutput(ctx context.Context, repoDir string, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "--git-dir", repoDir)
	cmd.Args = append(cmd.Args, args...)
	cmd.Env = append(os.Environ(),
		"GIT_AUTHOR_NAME=AgentDocs",
		"GIT_AUTHOR_EMAIL=agentdocs@local",
		"GIT_COMMITTER_NAME=AgentDocs",
		"GIT_COMMITTER_EMAIL=agentdocs@local",
		"GIT_CONFIG_NOSYSTEM=1",
	)
	var out, errBuf bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, strings.TrimSpace(errBuf.String()))
	}
	return strings.TrimSpace(out.String()), nil
}
