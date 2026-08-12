package project

import (
	"context"
	"errors"
	"strings"
	"testing"
)

func seedHistory(t *testing.T) (*Service, string) {
	t.Helper()
	svc, pid, _ := newServiceWithRepo(t)
	// Two changesets: create docs/a.md; then update it and create b.md.
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: mustHead(t, svc, pid),
		Message:      "add a",
		Changes:      []Change{{Op: "create", Path: "docs/a.md", Content: "# A\n"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@agentdocs.local"}); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: mustHead(t, svc, pid),
		Message:      "update a and add b",
		Changes: []Change{
			{Op: "update", Path: "docs/a.md", Content: "# A updated\n"},
			{Op: "create", Path: "docs/b.md", Content: "# B\n"},
		},
	}, CommitAuthor{Name: "Test Author", Email: "test@agentdocs.local"}); err != nil {
		t.Fatal(err)
	}
	return svc, pid
}

func mustHead(t *testing.T, svc *Service, pid string) string {
	t.Helper()
	repo, err := svc.OpenRepo(context.Background(), pid)
	if err != nil {
		t.Fatal(err)
	}
	head, err := repo.Revision(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	return head
}

func TestListCommitsIncludesAllWrites(t *testing.T) {
	svc, pid := seedHistory(t)
	page, err := svc.SearchCommits(context.Background(), pid, CommitQuery{Limit: 10})
	if err != nil {
		t.Fatal(err)
	}
	commits := page.Commits
	// root README commit + 2 changesets = 3.
	if len(commits) != 3 {
		t.Fatalf("want 3 commits, got %d: %+v", len(commits), commits)
	}
	if commits[0].Message != "update a and add b" || commits[2].Message != "Initialize project docs-site" {
		t.Fatalf("ordering wrong: %+v", commits)
	}
	if len(commits[0].SHA) != 40 {
		t.Fatalf("bad sha %q", commits[0].SHA)
	}
	// Pagination.
	two, err := svc.SearchCommits(context.Background(), pid, CommitQuery{Limit: 2})
	if err != nil || len(two.Commits) != 2 || !two.HasMore {
		t.Fatalf("limit failed: %+v %v", two, err)
	}
	rest, err := svc.SearchCommits(context.Background(), pid, CommitQuery{Limit: 10, Offset: 2})
	if err != nil || len(rest.Commits) != 1 || rest.HasMore {
		t.Fatalf("offset failed: %+v %v", rest, err)
	}
}

func TestSearchCommitsMatchesAllRefsAndFields(t *testing.T) {
	svc, pid := seedHistory(t)
	repo, err := svc.OpenRepo(context.Background(), pid)
	if err != nil {
		t.Fatal(err)
	}
	tree, err := gitOutput(context.Background(), repo.Dir, "show", "-s", "--format=%T", "HEAD")
	if err != nil {
		t.Fatal(err)
	}
	orphan, err := gitOutput(context.Background(), repo.Dir, "commit-tree", tree, "-m", "orphan release marker")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := gitOutput(context.Background(), repo.Dir, "update-ref", "refs/heads/release", orphan); err != nil {
		t.Fatal(err)
	}

	for _, tc := range []struct {
		name  string
		query string
		want  string
	}{
		{name: "message on another ref", query: "release marker", want: orphan},
		{name: "author", query: "test author", want: mustHead(t, svc, pid)},
		{name: "full sha", query: orphan, want: orphan},
		{name: "short sha", query: orphan[:8], want: orphan},
	} {
		t.Run(tc.name, func(t *testing.T) {
			page, err := svc.SearchCommits(context.Background(), pid, CommitQuery{Query: tc.query, Limit: 10})
			if err != nil {
				t.Fatal(err)
			}
			found := false
			for _, commit := range page.Commits {
				if commit.SHA == tc.want {
					found = true
				}
			}
			if !found {
				t.Fatalf("query %q did not find %s in %+v", tc.query, tc.want, page.Commits)
			}
		})
	}
}

func TestGetCommitDetail(t *testing.T) {
	svc, pid := seedHistory(t)
	head := mustHead(t, svc, pid)
	detail, err := svc.GetCommit(context.Background(), pid, head)
	if err != nil {
		t.Fatal(err)
	}
	if detail.Message != "update a and add b" {
		t.Fatalf("message: %q", detail.Message)
	}
	if len(detail.Files) != 2 {
		t.Fatalf("want 2 files, got %+v", detail.Files)
	}
	if _, err := svc.GetCommit(context.Background(), pid, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("unknown sha: want ErrNotFound, got %v", err)
	}
}

func TestFileHistory(t *testing.T) {
	svc, pid := seedHistory(t)
	commits, err := svc.FileHistory(context.Background(), pid, "docs/a.md")
	if err != nil {
		t.Fatal(err)
	}
	// a.md touched by both changesets.
	if len(commits) != 2 {
		t.Fatalf("want 2 commits for a.md, got %+v", commits)
	}
	bCommits, err := svc.FileHistory(context.Background(), pid, "docs/b.md")
	if err != nil {
		t.Fatal(err)
	}
	if len(bCommits) != 1 {
		t.Fatalf("want 1 commit for b.md, got %+v", bCommits)
	}
}

func TestCommitDiffNumstatAndPatch(t *testing.T) {
	svc, pid := seedHistory(t)
	head := mustHead(t, svc, pid)
	numstat, err := svc.CommitDiff(context.Background(), pid, head, "numstat")
	if err != nil {
		t.Fatal(err)
	}
	if len(numstat.Stats) != 2 {
		t.Fatalf("numstat: want 2 files, got %+v", numstat.Stats)
	}
	found := false
	for _, st := range numstat.Stats {
		if st.Path == "docs/a.md" && st.Added > 0 {
			found = true
		}
	}
	if !found {
		t.Fatalf("numstat missing a.md: %+v", numstat.Stats)
	}
	patch, err := svc.CommitDiff(context.Background(), pid, head, "patch")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(patch.Patch, "diff --git") || !strings.Contains(patch.Patch, "docs/a.md") {
		t.Fatalf("patch content wrong: %.200s", patch.Patch)
	}
	if _, err := svc.CommitDiff(context.Background(), pid, head, "bogus"); !errors.Is(err, ErrInvalid) {
		t.Fatalf("bad format: want ErrInvalid, got %v", err)
	}
}

func TestRevertCreatesNewCommit(t *testing.T) {
	svc, pid := seedHistory(t)
	head := mustHead(t, svc, pid)

	reverted, err := svc.RevertCommit(context.Background(), pid, head, "", CommitAuthor{Name: "Test Author", Email: "test@agentdocs.local"})
	if err != nil {
		t.Fatalf("revert: %v", err)
	}
	if reverted.SHA == "" {
		t.Fatal("revert returned no sha")
	}
	// Count advanced by 1, original commit still present.
	page, _ := svc.SearchCommits(context.Background(), pid, CommitQuery{Limit: 10})
	commits := page.Commits
	if len(commits) != 4 {
		t.Fatalf("want 4 commits after revert, got %d", len(commits))
	}
	if commits[0].SHA == head {
		t.Fatal("revert must create a new commit, not rewrite")
	}
	if _, err := svc.GetCommit(context.Background(), pid, head); err != nil {
		t.Fatalf("original commit vanished: %v", err)
	}
	// The revert commit must contain no internal patch file.
	revertDetail, err := svc.GetCommit(context.Background(), pid, reverted.SHA)
	if err != nil {
		t.Fatal(err)
	}
	for _, f := range revertDetail.Files {
		if strings.Contains(f.Path, "revert.patch") {
			t.Fatalf("revert commit leaked patch file: %+v", revertDetail.Files)
		}
	}
	// Content reverted: a.md back to "# A\n" (create state), b.md gone.
	repo, _ := svc.OpenRepo(context.Background(), pid)
	content, err := repo.ReadBlob(context.Background(), "main", "docs/a.md")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), "# A") || strings.Contains(string(content), "updated") {
		t.Fatalf("a.md not reverted: %q", content)
	}
	if _, err := repo.ReadBlob(context.Background(), "main", "docs/b.md"); err == nil {
		t.Fatal("b.md still exists after revert")
	}
	// Unknown sha -> ErrNotFound.
	if _, err := svc.RevertCommit(context.Background(), pid, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "", CommitAuthor{Name: "Test Author", Email: "test@agentdocs.local"}); !errors.Is(err, ErrNotFound) {
		t.Fatalf("unknown sha: want ErrNotFound, got %v", err)
	}
}
