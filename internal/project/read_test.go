package project

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"time"
)

// commitExtraFile appends a commit that adds a nested docs/guide.md plus a
// second root file (guide-root.md) to the repo, preserving existing content.
func commitExtraFile(t *testing.T, r *Repo, branch string, now time.Time) {
	t.Helper()
	guide := "# Guide\n\nsteps\n"
	rootNote := "# Root note\n"
	blobGuide := hashObjectStdin(t, r, guide)
	blobRoot := hashObjectStdin(t, r, rootNote)

	docsTree := mktreeStdin(t, r, fmt.Sprintf("100644 blob %s\tguide.md\n", blobGuide))
	// Preserve existing root entries and add the new ones.
	existing := gitCmd(t, r.Dir, "ls-tree", branch)
	var rootInput strings.Builder
	for _, line := range strings.Split(existing, "\n") {
		if line != "" {
			rootInput.WriteString(line)
			rootInput.WriteString("\n")
		}
	}
	rootInput.WriteString(fmt.Sprintf("100644 blob %s\tguide-root.md\n040000 tree %s\tdocs\n", blobRoot, docsTree))
	rootTree := mktreeStdin(t, r, rootInput.String())
	parent := gitCmd(t, r.Dir, "rev-parse", "HEAD")
	commit, err := gitOutputOk(t, r, "commit-tree", rootTree, "-p", parent, "-m", "add docs")
	if err != nil {
		t.Fatalf("commit-tree: %v", err)
	}
	gitCmd(t, r.Dir, "update-ref", "refs/heads/"+branch, commit)
}

func hashObjectStdin(t *testing.T, r *Repo, content string) string {
	t.Helper()
	out, err := gitWithStdin(context.Background(), r.Dir, content, "hash-object", "-w", "--stdin")
	if err != nil {
		t.Fatalf("hash-object: %v", err)
	}
	return out
}

func mktreeStdin(t *testing.T, r *Repo, input string) string {
	t.Helper()
	out, err := gitWithStdin(context.Background(), r.Dir, input, "mktree")
	if err != nil {
		t.Fatalf("mktree: %v", err)
	}
	return out
}

func gitOutputOk(_ *testing.T, r *Repo, args ...string) (string, error) {
	return gitOutput(context.Background(), r.Dir, args...)
}

func newRepoWithDocs(t *testing.T) *Repo {
	t.Helper()
	r, err := InitBare(context.Background(), t.TempDir(), "prj_1")
	if err != nil {
		t.Fatal(err)
	}
	now := time.Now().UTC()
	if err := r.WriteReadme(context.Background(), "docs-site", "产品文档", now); err != nil {
		t.Fatal(err)
	}
	commitExtraFile(t, r, "main", now)
	return r
}

func TestDefaultBranch(t *testing.T) {
	r := newRepoWithDocs(t)
	branch, err := r.DefaultBranch(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if branch != "main" {
		t.Fatalf("want main, got %q", branch)
	}
}

func TestListTreeSingleLevel(t *testing.T) {
	r := newRepoWithDocs(t)
	entries, err := r.ListTree(context.Background(), "main", "")
	if err != nil {
		t.Fatal(err)
	}
	names := map[string]string{}
	for _, e := range entries {
		names[e.Name] = e.Type
	}
	if names["README.md"] != "blob" || names["guide-root.md"] != "blob" || names["docs"] != "tree" {
		t.Fatalf("root entries wrong: %v", names)
	}
	sub, err := r.ListTree(context.Background(), "main", "docs")
	if err != nil {
		t.Fatal(err)
	}
	if len(sub) != 1 || sub[0].Name != "guide.md" || sub[0].Type != "blob" {
		t.Fatalf("docs entries wrong: %+v", sub)
	}
}

func TestReadBlobAndResolveTree(t *testing.T) {
	r := newRepoWithDocs(t)
	content, err := r.ReadBlob(context.Background(), "main", "docs/guide.md")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(content), "# Guide") {
		t.Fatalf("unexpected guide content: %q", content)
	}
	treeSHA, err := r.ResolveTree(context.Background(), "main", "docs")
	if err != nil {
		t.Fatal(err)
	}
	if len(treeSHA) != 40 {
		t.Fatalf("bad tree sha: %q", treeSHA)
	}
	// Resolving a blob path as a tree must fail.
	if _, err := r.ResolveTree(context.Background(), "main", "README.md"); err == nil {
		t.Fatal("README.md resolved as a tree")
	}
}

func TestReadFileRejectsTraversal(t *testing.T) {
	r := newRepoWithDocs(t)
	for _, bad := range []string{"../README.md", "/etc/passwd", "a/../../b", "docs/../../README.md"} {
		if _, err := r.ReadBlob(context.Background(), "main", bad); err == nil {
			t.Fatalf("traversal path %q was accepted", bad)
		}
	}
	if _, err := r.ReadBlob(context.Background(), "main", "missing.md"); err == nil {
		t.Fatal("missing blob accepted")
	}
}
