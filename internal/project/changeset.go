package project

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"hash/fnv"
	"os"
	"path"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

// CommitAuthor is the identity recorded on commits (set by the handler from
// the authenticated actor, never from client input).
type CommitAuthor struct {
	Name  string
	Email string
}

// Change is one file operation inside a changeset.
type Change struct {
	Op       string `json:"op"` // create | update | delete | move
	Path     string `json:"path"`
	Content  string `json:"content,omitempty"`
	Encoding string `json:"encoding,omitempty"` // "" | "base64"
	NewPath  string `json:"new_path,omitempty"`
}

// ChangesetInput is the full write request.
type ChangesetInput struct {
	BaseRevision string   `json:"base_revision"`
	Message      string   `json:"message"`
	Changes      []Change `json:"changes"`
	DryRun       bool     `json:"dry_run,omitempty"`
}

// ChangesetResult is the outcome of an applied changeset.
type ChangesetResult struct {
	Commit   string            `json:"commit,omitempty"`
	Revision string            `json:"revision"`
	Preview  *ChangesetPreview `json:"preview,omitempty"`
}

// ChangesetPreview describes a dry-run without writing any ref.
type ChangesetPreview struct {
	Tree    string          `json:"tree"`
	Changes []ChangeOutcome `json:"changes"`
}

// ChangeOutcome reports the applied status of one change.
type ChangeOutcome struct {
	Op     string `json:"op"`
	Path   string `json:"path"`
	Status string `json:"status"` // created | updated | deleted | moved
}

// MaxChangesetFiles caps the number of changes per request.
const MaxChangesetFiles = 100

// MaxDocBlobBytes caps the size of a single document (read and write).
const MaxDocBlobBytes = 2 << 20 // 2 MiB

// mutationCoordinator serializes project mutations within this process. A
// striped pool instead of a grow-only map keeps memory bounded no matter how
// many projects are created and deleted over the process lifetime.
type mutationCoordinator struct {
	locks [64]sync.Mutex
}

func (c *mutationCoordinator) lockFor(projectID string) *sync.Mutex {
	h := fnv.New32a()
	h.Write([]byte(projectID))
	return &c.locks[h.Sum32()%uint32(len(c.locks))]
}

// LockProjectWrite serializes all operations that can move a project's Git
// refs. Callers must defer the returned unlock function.
func LockProjectWrite(projectID string) func() {
	mu := projectMutationCoordinator.lockFor(projectID)
	mu.Lock()
	return mu.Unlock
}

var projectMutationCoordinator mutationCoordinator

// ErrArchived reports writes to an archived project.
var ErrArchived = errors.New("project is archived")

// ErrPathExists reports a move whose target path is already occupied. The
// server never overwrites an existing path, so clients can preflight moves
// with a dry run and surface the conflict before submitting.
var ErrPathExists = errors.New("target path already exists")

// ErrSourceMissing reports a move whose source path does not exist.
var ErrSourceMissing = errors.New("source path does not exist")

// ApplyChangeset applies a set of file changes in one atomic commit. A dry
// run returns a preview without touching any ref. Concurrent stale writes are
// rejected with ErrConflict by Git's compare-and-swap update-ref.
func (s *Service) ApplyChangeset(ctx context.Context, projectID string, input ChangesetInput, author CommitAuthor) (*ChangesetResult, error) {
	if err := validateChangeset(input); err != nil {
		return nil, err
	}
	if author.Name == "" {
		author.Name = "anonymous"
	}
	if author.Email == "" {
		author.Email = "anonymous@xwiki.local"
	}
	if strings.TrimSpace(input.Message) == "" {
		input.Message = defaultMessage(s.clock.Now(), author, input.Changes)
	}
	p, err := s.store.GetByID(ctx, projectID)
	if err != nil {
		return nil, err
	}
	if p.IsArchived() {
		return nil, ErrArchived
	}

	// Route every project Git mutation through the shared coordinator. This
	// keeps changesets serialized with rename, purge, push, and revert.
	unlock := LockProjectWrite(p.ID)
	defer unlock()

	repo := &Repo{Dir: filepath.Join(s.reposRoot, p.ID, "repo.git")}
	branch, err := repo.DefaultBranch(ctx)
	if err != nil {
		return nil, fmt.Errorf("resolve branch: %w", err)
	}
	current, err := gitOutput(ctx, repo.Dir, "rev-parse", "HEAD")
	if err != nil {
		return nil, fmt.Errorf("resolve head: %w", err)
	}
	if current != input.BaseRevision {
		return nil, ErrConflict
	}

	// Temporary worktree on a detached HEAD.
	wtDir, err := os.MkdirTemp("", "xwiki-wt-*")
	if err != nil {
		return nil, fmt.Errorf("create worktree dir: %w", err)
	}
	cleanup := func() {
		_, _ = gitOutput(context.Background(), repo.Dir, "worktree", "remove", "--force", wtDir)
		_ = os.RemoveAll(wtDir)
	}
	if _, err := gitOutput(ctx, repo.Dir, "worktree", "add", "--detach", wtDir, branch); err != nil {
		cleanup()
		return nil, fmt.Errorf("add worktree: %w", err)
	}

	// Decode base64 payloads in place before applying.
	for i := range input.Changes {
		c := &input.Changes[i]
		if c.Encoding == "base64" {
			decoded, err := base64.StdEncoding.DecodeString(c.Content)
			if err != nil {
				cleanup()
				return nil, fmt.Errorf("invalid base64 content for %q", c.Path)
			}
			c.Content = string(decoded)
			c.Encoding = ""
		}
	}

	// Apply changes inside the worktree.
	outcomes := make([]ChangeOutcome, 0, len(input.Changes))
	applyErr := func() error {
		for _, c := range input.Changes {
			outcome, err := applyOne(wtDir, c)
			if err != nil {
				return err
			}
			outcomes = append(outcomes, outcome)
		}
		return nil
	}()
	if applyErr != nil {
		cleanup()
		return nil, applyErr
	}

	// Stage everything and build the commit (run inside the worktree so git
	// discovers its own .git file rather than the bare repo).
	if _, err := gitOutputIn(ctx, wtDir, "add", "-A"); err != nil {
		cleanup()
		return nil, fmt.Errorf("git add: %w", err)
	}
	tree, err := gitOutputIn(ctx, wtDir, "write-tree")
	if err != nil {
		cleanup()
		return nil, fmt.Errorf("write-tree: %w", err)
	}

	if input.DryRun {
		cleanup()
		return &ChangesetResult{
			Revision: current,
			Preview:  &ChangesetPreview{Tree: tree, Changes: outcomes},
		}, nil
	}

	commit, err := gitOutputAs(ctx, repo.Dir, author, "commit-tree", tree, "-p", current, "-m", input.Message)
	if err != nil {
		cleanup()
		return nil, fmt.Errorf("commit-tree: %w", err)
	}
	// Atomic compare-and-swap: fails when the branch moved since we started.
	if _, err := gitOutput(ctx, repo.Dir, "update-ref", "refs/heads/"+branch, commit, current); err != nil {
		cleanup()
		return nil, ErrConflict
	}
	cleanup()
	return &ChangesetResult{Commit: commit, Revision: commit}, nil
}

// defaultMessage builds the fallback commit message when none is supplied:
// "<time> <author> 修改 [<firstPath>]" (e.g. 2026-08-02 18:30 admin 修改 docs/a.md).
func defaultMessage(now time.Time, author CommitAuthor, changes []Change) string {
	msg := fmt.Sprintf("%s %s 修改", now.UTC().Format("2006-01-02 15:04"), author.Name)
	if len(changes) > 0 && changes[0].Path != "" {
		msg += " " + changes[0].Path
	}
	return msg
}

// validateChangeset checks structure, sizes and path safety up front.
// Message may be empty: the service fills a default (time + author).
func validateChangeset(input ChangesetInput) error {
	if len(input.Changes) == 0 || len(input.Changes) > MaxChangesetFiles {
		return fmt.Errorf("changeset must contain 1-%d changes", MaxChangesetFiles)
	}
	for _, c := range input.Changes {
		if !validateDocPathInternal(c.Path) {
			return fmt.Errorf("invalid path %q", c.Path)
		}
		switch c.Op {
		case "create", "update":
			if c.Encoding != "" && c.Encoding != "base64" {
				return fmt.Errorf("unsupported encoding %q", c.Encoding)
			}
			size := len(c.Content)
			if c.Encoding == "base64" {
				if _, err := base64.StdEncoding.DecodeString(c.Content); err != nil {
					return fmt.Errorf("invalid base64 content for %q", c.Path)
				}
				size = base64.StdEncoding.DecodedLen(len(c.Content))
			}
			if size > MaxImportFileBytes {
				return fmt.Errorf("content of %q exceeds size limit", c.Path)
			}
		case "delete":
			// no content expected
		case "move":
			if c.NewPath == "" || !validateDocPathInternal(c.NewPath) {
				return fmt.Errorf("invalid new_path %q", c.NewPath)
			}
			if path.Clean(c.Path) == path.Clean(c.NewPath) {
				return fmt.Errorf("new_path must differ from path")
			}
		default:
			return fmt.Errorf("unsupported op %q", c.Op)
		}
	}
	return nil
}

func validateDocPathInternal(p string) bool {
	if p == "" || strings.HasPrefix(p, "/") || strings.Contains(p, "\\") {
		return false
	}
	clean := path.Clean(p)
	return clean != ".." && !strings.HasPrefix(clean, "../")
}

// applyOne performs a single file operation inside the worktree.
func applyOne(wtDir string, c Change) (ChangeOutcome, error) {
	abs := filepath.Join(wtDir, filepath.FromSlash(c.Path))
	switch c.Op {
	case "create", "update":
		if err := os.MkdirAll(filepath.Dir(abs), 0o755); err != nil {
			return ChangeOutcome{}, err
		}
		if err := os.WriteFile(abs, []byte(c.Content), 0o644); err != nil {
			return ChangeOutcome{}, err
		}
		status := "updated"
		if c.Op == "create" {
			status = "created"
		}
		return ChangeOutcome{Op: c.Op, Path: c.Path, Status: status}, nil
	case "delete":
		if err := os.RemoveAll(abs); err != nil {
			return ChangeOutcome{}, fmt.Errorf("delete %s: %w", c.Path, err)
		}
		return ChangeOutcome{Op: c.Op, Path: c.Path, Status: "deleted"}, nil
	case "move":
		if _, err := os.Lstat(abs); err != nil {
			if os.IsNotExist(err) {
				return ChangeOutcome{}, ErrSourceMissing
			}
			return ChangeOutcome{}, fmt.Errorf("stat %s: %w", c.Path, err)
		}
		newAbs := filepath.Join(wtDir, filepath.FromSlash(c.NewPath))
		if err := os.MkdirAll(filepath.Dir(newAbs), 0o755); err != nil {
			return ChangeOutcome{}, err
		}
		// Never overwrite: os.Rename would silently replace an existing file
		// (and an existing empty directory), which is data loss. Reject with
		// a typed error so the API maps it to a distinct conflict code.
		if _, err := os.Lstat(newAbs); err == nil {
			return ChangeOutcome{}, ErrPathExists
		}
		if err := os.Rename(abs, newAbs); err != nil {
			return ChangeOutcome{}, fmt.Errorf("move %s: %w", c.Path, err)
		}
		return ChangeOutcome{Op: c.Op, Path: c.Path, Status: "moved"}, nil
	}
	return ChangeOutcome{}, fmt.Errorf("unsupported op %q", c.Op)
}
