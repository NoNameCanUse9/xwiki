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

// TreeEntry is one entry of a directory listing.
type TreeEntry struct {
	Name string `json:"name"`
	Type string `json:"type"` // "blob" | "tree"
	Path string `json:"path"`
}

// Revision returns the current HEAD commit id (the write base revision).
func (r *Repo) Revision(ctx context.Context) (string, error) {
	out, err := gitOutput(ctx, r.Dir, "rev-parse", "HEAD")
	if err != nil {
		return "", fmt.Errorf("rev-parse HEAD: %w", err)
	}
	return out, nil
}

// DefaultBranch resolves the repository's current branch name.
func (r *Repo) DefaultBranch(ctx context.Context) (string, error) {
	out, err := gitOutput(ctx, r.Dir, "rev-parse", "--abbrev-ref", "HEAD")
	if err != nil {
		return "", err
	}
	if out == "HEAD" {
		return "", fmt.Errorf("repository has no branch yet")
	}
	return out, nil
}

// ListTree lists one directory level of the branch. path is repository
// relative ("" for the root, "docs" or "docs/sub" for subdirectories).
func (r *Repo) ListTree(ctx context.Context, branch, path string) ([]TreeEntry, error) {
	treeISH := branch
	if path != "" {
		treeISH = branch + ":" + strings.TrimSuffix(path, "/")
	}
	out, err := gitOutput(ctx, r.Dir, "ls-tree", treeISH)
	if err != nil {
		return nil, fmt.Errorf("ls-tree %s: %w", path, err)
	}
	var entries []TreeEntry
	for _, line := range strings.Split(out, "\n") {
		if line == "" {
			continue
		}
		meta, name, ok := strings.Cut(line, "\t")
		if !ok {
			continue
		}
		fields := strings.Fields(meta)
		if len(fields) < 3 {
			continue
		}
		entryPath := name
		if path != "" {
			entryPath = strings.TrimSuffix(path, "/") + "/" + name
		}
		entries = append(entries, TreeEntry{Name: name, Type: fields[1], Path: entryPath})
	}
	return entries, nil
}

// ReadBlob returns the raw content of a file at the given repository path.
func (r *Repo) ReadBlob(ctx context.Context, branch, path string) ([]byte, error) {
	out, err := gitOutput(ctx, r.Dir, "cat-file", "blob", branch+":"+path)
	if err != nil {
		return nil, fmt.Errorf("cat-file %s: %w", path, err)
	}
	return []byte(out), nil
}

// ResolveTree resolves a directory path to its tree object id, failing when
// the path is not a tree.
func (r *Repo) ResolveTree(ctx context.Context, branch, path string) (string, error) {
	path = strings.TrimSuffix(path, "/")
	treeISH := branch
	if path == "" {
		treeISH = branch + "^{tree}"
	} else {
		treeISH = branch + ":" + path
	}
	out, err := gitOutput(ctx, r.Dir, "rev-parse", treeISH)
	if err != nil {
		return "", fmt.Errorf("resolve tree %s: %w", path, err)
	}
	typ, err := gitOutput(ctx, r.Dir, "cat-file", "-t", out)
	if err != nil {
		return "", fmt.Errorf("resolve tree %s: %w", path, err)
	}
	if typ != "tree" {
		return "", fmt.Errorf("%s is not a directory", path)
	}
	return out, nil
}

// gitWithStdin runs a git command feeding content on stdin, returning stdout.
func gitWithStdin(ctx context.Context, repoDir string, stdin string, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "--git-dir", repoDir)
	cmd.Args = append(cmd.Args, args...)
	cmd.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1")
	cmd.Stdin = strings.NewReader(stdin)
	var out bytes.Buffer
	cmd.Stdout = &out
	var errBuf bytes.Buffer
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, strings.TrimSpace(errBuf.String()))
	}
	return strings.TrimSpace(out.String()), nil
}

// gitOutputIn runs git with -C dir (worktree-aware command execution).
func gitOutputIn(ctx context.Context, dir string, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "-C", dir)
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

// gitOutputRaw runs a git command and returns stdout without trimming.
func gitOutputRaw(ctx context.Context, repoDir string, args ...string) ([]byte, error) {
	cmd := exec.CommandContext(ctx, "git", "--git-dir", repoDir)
	cmd.Args = append(cmd.Args, args...)
	cmd.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1")
	var out, errBuf bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		return nil, fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, strings.TrimSpace(errBuf.String()))
	}
	return out.Bytes(), nil
}

// HashBlob writes a blob into the repository object store and returns its id.
func (r *Repo) HashBlob(ctx context.Context, content []byte) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "--git-dir", r.Dir, "hash-object", "-w", "--stdin")
	cmd.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1")
	cmd.Stdin = bytes.NewReader(content)
	var out, errBuf bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("hash-object: %w: %s", err, strings.TrimSpace(errBuf.String()))
	}
	return strings.TrimSpace(out.String()), nil
}
