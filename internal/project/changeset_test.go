package project

import (
	"context"
	"errors"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"
)

func newServiceWithRepo(t *testing.T) (*Service, string, *Repo) {
	t.Helper()
	svc, _ := newService(t)
	p, err := svc.Create(context.Background(), CreateInput{Name: "docs-site", Description: ""})
	if err != nil {
		t.Fatal(err)
	}
	repo, err := svc.OpenRepo(context.Background(), p.ID)
	if err != nil {
		t.Fatal(err)
	}
	return svc, p.ID, repo
}

func TestLockProjectWriteSerializesSameProject(t *testing.T) {
	firstUnlock := LockProjectWrite("prj_same")
	acquired := make(chan struct{})
	go func() {
		secondUnlock := LockProjectWrite("prj_same")
		close(acquired)
		secondUnlock()
	}()

	select {
	case <-acquired:
		t.Fatal("same-project mutation acquired the lock concurrently")
	case <-time.After(25 * time.Millisecond):
	}

	firstUnlock()
	select {
	case <-acquired:
	case <-time.After(time.Second):
		t.Fatal("same-project mutation did not acquire after unlock")
	}
}

func headOf(t *testing.T, r *Repo) string {
	t.Helper()
	head, err := gitOutput(context.Background(), r.Dir, "rev-parse", "HEAD")
	if err != nil {
		t.Fatal(err)
	}
	return head
}

func TestApplyChangesetMultipleFilesOneCommit(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	res, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "add guide and update readme",
		Changes: []Change{
			{Op: "create", Path: "docs/guide.md", Content: "# Guide\n"},
			{Op: "update", Path: "README.md", Content: "# docs-site\n\nupdated\n"},
			{Op: "move", Path: "docs/guide.md", NewPath: "docs/handbook.md"},
		},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if err != nil {
		t.Fatalf("ApplyChangeset: %v", err)
	}
	if res.Commit == "" {
		t.Fatal("expected a commit")
	}
	// Exactly one new commit.
	count, err := gitOutput(context.Background(), repo.Dir, "rev-list", "--count", "HEAD")
	if err != nil {
		t.Fatal(err)
	}
	if count != "2" {
		t.Fatalf("want 2 commits total, got %s", count)
	}
	// Moved file exists, old path gone, README updated.
	if _, err := repo.ReadBlob(context.Background(), "main", "docs/handbook.md"); err != nil {
		t.Fatalf("moved file missing: %v", err)
	}
	if _, err := repo.ReadBlob(context.Background(), "main", "docs/guide.md"); err == nil {
		t.Fatal("old path still exists after move")
	}
	readme, err := repo.ReadBlob(context.Background(), "main", "README.md")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(readme), "updated") {
		t.Fatalf("README not updated: %q", readme)
	}
	if res.Revision != headOf(t, repo) {
		t.Fatal("result revision must match new HEAD")
	}
}

func TestApplyChangesetStaleRevisionConflict(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "first",
		Changes:      []Change{{Op: "create", Path: "a.md", Content: "a"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	// Second commit with the stale base revision must conflict.
	_, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "second",
		Changes:      []Change{{Op: "create", Path: "b.md", Content: "b"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
	// HEAD unchanged (only 2 commits).
	count, _ := gitOutput(context.Background(), repo.Dir, "rev-list", "--count", "HEAD")
	if count != "2" {
		t.Fatalf("conflict must not create a commit, got %s", count)
	}
}

func TestApplyChangesetFailureLeavesNoCommitOrWorktree(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	_, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "bad",
		Changes:      []Change{{Op: "create", Path: "../evil.md", Content: "x"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if err == nil {
		t.Fatal("traversal path accepted")
	}
	if headOf(t, repo) != base {
		t.Fatal("HEAD moved after failed changeset")
	}
	// No worktree residue.
	wts, err := gitOutput(context.Background(), repo.Dir, "worktree", "list", "--porcelain")
	if err != nil {
		t.Fatal(err)
	}
	// git prints forward slashes even on Windows; compare in slash form.
	if !strings.Contains(wts, filepath.ToSlash(repo.Dir)) || strings.Count(wts, "worktree ") > 1 {
		t.Fatalf("worktree residue: %q", wts)
	}
}

func TestApplyChangesetDryRun(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	res, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "preview",
		DryRun:       true,
		Changes:      []Change{{Op: "create", Path: "preview.md", Content: "p"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if err != nil {
		t.Fatal(err)
	}
	if res.Commit != "" || res.Preview == nil || res.Preview.Tree == "" {
		t.Fatalf("dry run result wrong: %+v", res)
	}
	if len(res.Preview.Changes) != 1 || res.Preview.Changes[0].Path != "preview.md" {
		t.Fatalf("preview changes wrong: %+v", res.Preview.Changes)
	}
	if headOf(t, repo) != base {
		t.Fatal("dry run must not move HEAD")
	}
	count, _ := gitOutput(context.Background(), repo.Dir, "rev-list", "--count", "HEAD")
	if count != "1" {
		t.Fatalf("dry run must not create commits, got %s", count)
	}
}

func TestApplyChangesetArchivedProjectRejected(t *testing.T) {
	svc, _ := newService(t)
	p, err := svc.Create(context.Background(), CreateInput{Name: "archived-site"})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := svc.Archive(context.Background(), p.ID); err != nil {
		t.Fatal(err)
	}
	_, err = svc.ApplyChangeset(context.Background(), p.ID, ChangesetInput{
		BaseRevision: "x",
		Message:      "nope",
		Changes:      []Change{{Op: "create", Path: "a.md", Content: "a"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if !errors.Is(err, ErrArchived) {
		t.Fatalf("want ErrArchived, got %v", err)
	}
}

func TestApplyChangesetRejectsInvalidOpsAndPaths(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)
	cases := []ChangesetInput{
		{BaseRevision: base, Message: "m", Changes: []Change{{Op: "explode", Path: "a.md"}}},
		{BaseRevision: base, Message: "m", Changes: []Change{{Op: "create", Path: "/abs.md", Content: "x"}}},
		{BaseRevision: base, Message: "m", Changes: []Change{{Op: "move", Path: "a.md"}}},                                          // no new_path
		{BaseRevision: base, Message: "m", Changes: []Change{{Op: "create", Path: "a.md", Content: strings.Repeat("x", 5<<20+1)}}}, // too large
		{BaseRevision: "deadbeef", Message: "m", Changes: []Change{{Op: "create", Path: "a.md", Content: "x"}}},                    // unknown base
	}
	for i, cs := range cases {
		if _, err := svc.ApplyChangeset(context.Background(), pid, cs, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err == nil {
			t.Fatalf("case %d: expected error", i)
		}
	}
	if headOf(t, repo) != base {
		t.Fatal("HEAD moved after invalid changesets")
	}
	_ = time.Now
}

func TestApplyChangesetMoveRejectsExistingTarget(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)

	mustApply := func(changes ...Change) {
		t.Helper()
		b := headOf(t, repo)
		if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
			BaseRevision: b, Message: "seed", Changes: changes,
		}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
			t.Fatalf("seed changeset: %v", err)
		}
	}
	mustApply(
		Change{Op: "create", Path: "a.md", Content: "a"},
		Change{Op: "create", Path: "b.md", Content: "B"},
		Change{Op: "create", Path: "dir/x.md", Content: "x"},
		Change{Op: "create", Path: "dir/sub/y.md", Content: "y"},
		Change{Op: "create", Path: "dir3/occupied.md", Content: "keep"},
	)

	cases := []struct {
		name string
		from string
		to   string
	}{
		{"file over file", "a.md", "b.md"},
		{"dir over non-empty dir", "dir", "dir3"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			head := headOf(t, repo)
			_, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
				BaseRevision: head, Message: "move",
				Changes: []Change{{Op: "move", Path: tc.from, NewPath: tc.to}},
			}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
			if !errors.Is(err, ErrPathExists) {
				t.Fatalf("want ErrPathExists, got %v", err)
			}
			if headOf(t, repo) != head {
				t.Fatal("rejected move must not move HEAD")
			}
		})
	}
	// The rejected moves must not have clobbered anything: b.md keeps its
	// content, dir/sub/y.md and the empty dir2 are intact.
	blob, err := repo.ReadBlob(context.Background(), "main", "b.md")
	if err != nil {
		t.Fatal(err)
	}
	if string(blob) != "B" {
		t.Fatalf("target file was clobbered: %q", blob)
	}
	if _, err := repo.ReadBlob(context.Background(), "main", "dir/sub/y.md"); err != nil {
		t.Fatalf("dir subtree damaged: %v", err)
	}
	if _, err := repo.ReadBlob(context.Background(), "main", "dir3/occupied.md"); err != nil {
		t.Fatalf("non-empty dir damaged: %v", err)
	}
	if _, err := repo.ReadBlob(context.Background(), "main", "dir/x.md"); err != nil {
		t.Fatalf("source dir damaged: %v", err)
	}
}

func TestApplyChangesetMoveSourceMissing(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	_, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base, Message: "move ghost",
		Changes: []Change{{Op: "move", Path: "ghost.md", NewPath: "gone.md"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if !errors.Is(err, ErrSourceMissing) {
		t.Fatalf("want ErrSourceMissing, got %v", err)
	}
	if headOf(t, repo) != base {
		t.Fatal("rejected move must not move HEAD")
	}
}

func TestApplyChangesetMoveSamePathRejected(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	cases := []Change{
		{Op: "move", Path: "a.md", NewPath: "a.md"},
		{Op: "move", Path: "docs/a.md", NewPath: "docs/../docs/a.md"},
	}
	for i, c := range cases {
		if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
			BaseRevision: base, Message: "same path", Changes: []Change{c},
		}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err == nil {
			t.Fatalf("case %d: same-path move accepted", i)
		}
	}
	if headOf(t, repo) != base {
		t.Fatal("rejected move must not move HEAD")
	}
}

func TestApplyChangesetMoveDryRunPreflight(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base, Message: "seed",
		Changes: []Change{
			{Op: "create", Path: "a.md", Content: "a"},
			{Op: "create", Path: "b.md", Content: "B"},
		},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	head := headOf(t, repo)

	// A dry run must catch the conflict without writing anything.
	_, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: head, Message: "preflight", DryRun: true,
		Changes: []Change{{Op: "move", Path: "a.md", NewPath: "b.md"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if !errors.Is(err, ErrPathExists) {
		t.Fatalf("dry run must reject existing target, got %v", err)
	}
	if headOf(t, repo) != head {
		t.Fatal("failed dry run must not move HEAD")
	}

	// A dry run to a free target succeeds with a preview and writes nothing.
	res, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: head, Message: "preflight ok", DryRun: true,
		Changes: []Change{{Op: "move", Path: "a.md", NewPath: "c.md"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"})
	if err != nil {
		t.Fatalf("dry run to free target: %v", err)
	}
	if res.Commit != "" || res.Preview == nil {
		t.Fatalf("dry run result wrong: %+v", res)
	}
	if len(res.Preview.Changes) != 1 || res.Preview.Changes[0].Status != "moved" {
		t.Fatalf("preview changes wrong: %+v", res.Preview.Changes)
	}
	if headOf(t, repo) != head {
		t.Fatal("successful dry run must not move HEAD")
	}
}

func TestDefaultMessageAndAuthorIdentity(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)

	// Empty message -> default "<time> <author> 修改 <path>".
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Changes:      []Change{{Op: "create", Path: "docs/auto.md", Content: "# Auto\n"}},
	}, CommitAuthor{Name: "Carol Chen", Email: "carol@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	out, err := gitOutput(context.Background(), repo.Dir, "log", "-1", "--format=%s%x1f%an%x1f%ae")
	if err != nil {
		t.Fatal(err)
	}
	parts := strings.Split(out, "\x1f")
	if len(parts) != 3 {
		t.Fatalf("log parts: %v", parts)
	}
	// message: "<time> Carol Chen 修改 docs/auto.md"
	if !strings.Contains(parts[0], "Carol Chen 修改") || !strings.Contains(parts[0], "docs/auto.md") {
		t.Fatalf("default message wrong: %q", parts[0])
	}
	if !regexp.MustCompile(`^\d{4}-\d{2}-\d{2} \d{2}:\d{2} `).MatchString(parts[0]) {
		t.Fatalf("message missing time prefix: %q", parts[0])
	}
	if parts[1] != "Carol Chen" || parts[2] != "carol@xwiki.local" {
		t.Fatalf("author identity wrong: %v", parts[1:])
	}
}
