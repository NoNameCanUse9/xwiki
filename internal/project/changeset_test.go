package project

import (
	"context"
	"errors"
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
	})
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
	}); err != nil {
		t.Fatal(err)
	}
	// Second commit with the stale base revision must conflict.
	_, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "second",
		Changes:      []Change{{Op: "create", Path: "b.md", Content: "b"}},
	})
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
	})
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
	if !strings.Contains(wts, repo.Dir) || strings.Count(wts, "worktree ") > 1 {
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
	})
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
	})
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
		{BaseRevision: base, Message: "m", Changes: []Change{{Op: "create", Path: "a.md", Content: strings.Repeat("x", 2<<20+1)}}}, // too large
		{BaseRevision: "deadbeef", Message: "m", Changes: []Change{{Op: "create", Path: "a.md", Content: "x"}}},                    // unknown base
	}
	for i, cs := range cases {
		if _, err := svc.ApplyChangeset(context.Background(), pid, cs); err == nil {
			t.Fatalf("case %d: expected error", i)
		}
	}
	if headOf(t, repo) != base {
		t.Fatal("HEAD moved after invalid changesets")
	}
	_ = time.Now
}
