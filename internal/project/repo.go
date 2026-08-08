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

// RewriteReadmeTitle rewrites the first line of README.md to the project's
// new name, committing the change on the default branch. If the repository
// has no README, it writes a fresh one.
func (r *Repo) RewriteReadmeTitle(ctx context.Context, name string) error {
	branch, err := r.DefaultBranch(ctx)
	if err != nil {
		return fmt.Errorf("resolve branch: %w", err)
	}
	wtDir, err := os.MkdirTemp("", "agentdocs-wt-*")
	if err != nil {
		return fmt.Errorf("create worktree dir: %w", err)
	}
	cleanup := func() {
		_, _ = gitOutput(context.Background(), r.Dir, "worktree", "remove", "--force", wtDir)
		_ = os.RemoveAll(wtDir)
	}
	// Attach the worktree to the default branch (not --detach) so the commit
	// advances the branch ref in the bare repository.
	if _, err := gitOutput(ctx, r.Dir, "worktree", "add", wtDir, branch); err != nil {
		cleanup()
		return fmt.Errorf("add worktree: %w", err)
	}
	readmePath := filepath.Join(wtDir, "README.md")
	var readme string
	if b, err := os.ReadFile(readmePath); err == nil {
		lines := strings.Split(string(b), "\n")
		if len(lines) > 0 && strings.HasPrefix(strings.TrimSpace(lines[0]), "#") {
			lines[0] = "# " + name
		}
		readme = strings.Join(lines, "\n")
	} else {
		readme = fmt.Sprintf("# %s\n\n---\n\nAgentDocs 项目\n", name)
	}
	if err := os.WriteFile(readmePath, []byte(readme), 0o644); err != nil {
		cleanup()
		return fmt.Errorf("write readme: %w", err)
	}
	if _, err := gitOutputIn(ctx, wtDir, "add", "README.md"); err != nil {
		cleanup()
		return fmt.Errorf("git add readme: %w", err)
	}
	if _, err := gitOutputIn(ctx, wtDir, "commit", "-m", "Rename project to "+name); err != nil {
		cleanup()
		return fmt.Errorf("git commit readme: %w", err)
	}
	cleanup()
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

// gitOutputAs runs a git command against a repo with the given commit author identity.
func gitOutputAs(ctx context.Context, repoDir string, author CommitAuthor, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git", "--git-dir", repoDir)
	cmd.Args = append(cmd.Args, args...)
	cmd.Env = append(os.Environ(),
		"GIT_AUTHOR_NAME="+author.Name,
		"GIT_AUTHOR_EMAIL="+author.Email,
		"GIT_COMMITTER_NAME="+author.Name,
		"GIT_COMMITTER_EMAIL="+author.Email,
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

// PurgePaths removes the given paths from the entire history using
// filter-branch, then expires reflogs and runs a full gc so the blobs are
// gone from the object store. This is the "hard delete" mode: irreversible
// and destructive. paths are repository-relative, either files or
// directories (a directory prefix is removed recursively).
func (r *Repo) PurgePaths(ctx context.Context, paths []string) error {
	if len(paths) == 0 {
		return fmt.Errorf("no paths to purge")
	}
	for _, p := range paths {
		if !validateDocPathInternal(p) {
			return fmt.Errorf("invalid path %q", p)
		}
	}
	branch, err := r.DefaultBranch(ctx)
	if err != nil {
		return err
	}
	// filter-branch runs an index filter that removes each path from every
	// commit (git rm -r --cached handles files and directories).
	var filter strings.Builder
	for _, p := range paths {
		filter.WriteString("git rm -r --cached --ignore-unmatch -- ")
		filter.WriteString(shellQuote(p))
		filter.WriteString("; ")
	}
	if _, err := gitOutput(ctx, r.Dir,
		"filter-branch", "--force",
		"--index-filter", filter.String(),
		"--prune-empty", "--", branch); err != nil {
		return fmt.Errorf("filter-branch: %w", err)
	}
	// Drop reflogs and garbage-collect so the removed objects are physically
	// gone from the object store.
	if _, err := gitOutput(ctx, r.Dir, "reflog", "expire", "--expire=now", "--all"); err != nil {
		return fmt.Errorf("reflog expire: %w", err)
	}
	if _, err := gitOutput(ctx, r.Dir, "gc", "--prune=now", "--aggressive"); err != nil {
		return fmt.Errorf("git gc: %w", err)
	}
	return nil
}

// shellQuote wraps a path in single quotes for embedding in the
// filter-branch index filter shell snippet.
func shellQuote(s string) string {
	return "'" + strings.ReplaceAll(s, "'", "'\\''") + "'"
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

// ReadBlobAt returns the raw content of a file as of a specific commit.
func (r *Repo) ReadBlobAt(ctx context.Context, rev, path string) ([]byte, error) {
	out, err := gitOutput(ctx, r.Dir, "cat-file", "blob", rev+":"+path)
	if err != nil {
		return nil, fmt.Errorf("cat-file %s@%s: %w", path, rev, err)
	}
	return []byte(out), nil
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

// gitOutputPlain runs git without --git-dir (repo-less commands like
// `git bundle verify`).
func gitOutputPlain(ctx context.Context, args ...string) (string, error) {
	cmd := exec.CommandContext(ctx, "git")
	cmd.Args = append(cmd.Args, args...)
	cmd.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1")
	var out, errBuf bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, strings.TrimSpace(errBuf.String()))
	}
	return strings.TrimSpace(out.String()), nil
}
