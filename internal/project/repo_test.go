package project

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func gitCmd(t *testing.T, repoDir string, args ...string) string {
	t.Helper()
	out, err := gitOutput(context.Background(), repoDir, args...)
	if err != nil {
		t.Fatalf("git %v: %v (out=%q)", args, err, out)
	}
	return out
}

func gitFail(t *testing.T, repoDir string, args ...string) {
	t.Helper()
	if _, err := gitOutput(context.Background(), repoDir, args...); err == nil {
		t.Fatalf("git %v unexpectedly succeeded", args)
	}
}

func TestInitBare(t *testing.T) {
	root := t.TempDir()
	r, err := InitBare(context.Background(), root, "prj_abc")
	if err != nil {
		t.Fatalf("InitBare: %v", err)
	}
	if got := gitCmd(t, r.Dir, "rev-parse", "--is-bare-repository"); strings.TrimSpace(got) != "true" {
		t.Fatalf("want bare repo, got %q", got)
	}
	if _, err := os.Stat(filepath.Join(r.Dir, "HEAD")); err != nil {
		t.Fatalf("bare repo lacks HEAD: %v", err)
	}
}

func TestWriteReadmeCreatesRootCommit(t *testing.T) {
	root := t.TempDir()
	r, err := InitBare(context.Background(), root, "prj_abc")
	if err != nil {
		t.Fatal(err)
	}
	now := time.Date(2026, 8, 2, 12, 0, 0, 0, time.UTC)
	if err := r.WriteReadme(context.Background(), "docs-site", "产品文档", now); err != nil {
		t.Fatalf("WriteReadme: %v", err)
	}
	head := strings.TrimSpace(gitCmd(t, r.Dir, "rev-parse", "HEAD"))
	if head == "" {
		t.Fatal("HEAD empty after WriteReadme")
	}
	if got := strings.TrimSpace(gitCmd(t, r.Dir, "rev-list", "--count", "HEAD")); got != "1" {
		t.Fatalf("want exactly 1 commit, got %s", got)
	}
	tree := strings.TrimSpace(gitCmd(t, r.Dir, "rev-parse", "HEAD^{tree}"))
	if !strings.Contains(gitCmd(t, r.Dir, "ls-tree", "--name-only", tree), "README.md") {
		t.Fatal("root tree lacks README.md")
	}
	blob := strings.TrimSpace(gitCmd(t, r.Dir, "ls-tree", tree, "README.md"))
	sha := strings.Fields(blob)[2]
	content := gitCmd(t, r.Dir, "cat-file", "blob", sha)
	if !strings.Contains(content, "docs-site") || !strings.Contains(content, "产品文档") {
		t.Fatalf("README content wrong: %q", content)
	}
}

func TestReposAreIsolated(t *testing.T) {
	root := t.TempDir()
	ra, err := InitBare(context.Background(), root, "prj_a")
	if err != nil {
		t.Fatal(err)
	}
	rb, err := InitBare(context.Background(), root, "prj_b")
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now().UTC()
	if err := ra.WriteReadme(context.Background(), "alpha", "a", now); err != nil {
		t.Fatal(err)
	}
	if err := rb.WriteReadme(context.Background(), "beta", "b", now); err != nil {
		t.Fatal(err)
	}
	headA := strings.TrimSpace(gitCmd(t, ra.Dir, "rev-parse", "HEAD"))
	headB := strings.TrimSpace(gitCmd(t, rb.Dir, "rev-parse", "HEAD"))
	if headA == headB {
		t.Fatal("two projects share the same commit — history is not isolated")
	}
	// B's commit object must not exist in A's object store.
	gitFail(t, ra.Dir, "cat-file", "-e", headB)
	gitFail(t, rb.Dir, "cat-file", "-e", headA)
}
