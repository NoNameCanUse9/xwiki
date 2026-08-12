package project

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/base64"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// walkEntries collects (path, blob bytes) for the whole tree.
func walkEntries(t *testing.T, repo *Repo) map[string][]byte {
	t.Helper()
	out := map[string][]byte{}
	var walk func(dir string) error
	walk = func(dir string) error {
		entries, err := repo.ListTree(context.Background(), "main", dir)
		if err != nil {
			return err
		}
		for _, e := range entries {
			if e.Type == "tree" {
				if err := walk(e.Path); err != nil {
					return err
				}
				continue
			}
			blob, err := repo.ReadBlob(context.Background(), "main", e.Path)
			if err != nil {
				return err
			}
			out[e.Path] = blob
		}
		return nil
	}
	if err := walk(""); err != nil {
		t.Fatal(err)
	}
	return out
}

func TestZipExportImportRoundTrip(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)
	// Write a text doc + a binary-ish file (non-UTF8 bytes).
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base,
		Message:      "add files",
		Changes: []Change{
			{Op: "create", Path: "docs/guide.md", Content: "# Guide\n"},
			{Op: "create", Path: "docs/data.bin", Content: string([]byte{0x00, 0x01, 0x02, 'x'})},
		},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	zipData, err := svc.ExportZip(context.Background(), pid)
	if err != nil {
		t.Fatalf("ExportZip: %v", err)
	}
	if len(zipData) == 0 {
		t.Fatal("empty zip")
	}
	// Import into a fresh project and compare trees.
	svc2, pid2, _ := newServiceWithRepo(t)
	res, err := svc2.ImportZip(context.Background(), pid2, ImportZipInput{
		BaseRevision: headOf(t, mustRepo(t, svc2, pid2)),
		Message:      "import",
		Files:        zipEntries(t, zipData),
	})
	if err != nil {
		t.Fatalf("ImportZip: %v", err)
	}
	if res.Imported < 3 { // README + 2 new files
		t.Fatalf("imported %d, want >= 3", res.Imported)
	}
	repo2, _ := svc2.OpenRepo(context.Background(), pid2)
	got := walkEntries(t, repo2)
	want := walkEntries(t, repo)
	for p, b := range want {
		if !bytes.Equal(got[p], b) {
			t.Fatalf("path %s differs after round trip", p)
		}
	}
}

func mustRepo(t *testing.T, svc *Service, pid string) *Repo {
	t.Helper()
	repo, err := svc.OpenRepo(context.Background(), pid)
	if err != nil {
		t.Fatal(err)
	}
	return repo
}

func TestBundleExportImportPreservesHistory(t *testing.T) {
	svc, pid, repo := newServiceWithRepo(t)
	base := headOf(t, repo)
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base, Message: "commit one",
		Changes: []Change{{Op: "create", Path: "a.md", Content: "a"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	base = headOf(t, repo)
	if _, err := svc.ApplyChangeset(context.Background(), pid, ChangesetInput{
		BaseRevision: base, Message: "commit two",
		Changes: []Change{{Op: "create", Path: "b.md", Content: "b"}},
	}, CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	bundle, err := svc.ExportBundle(context.Background(), pid)
	if err != nil || len(bundle) == 0 {
		t.Fatalf("ExportBundle: %v len=%d", err, len(bundle))
	}
	origCount := commitCount(t, repo)

	svc2, _, _ := newServiceWithRepo(t)
	imported, err := svc2.ImportBundle(context.Background(), ImportBundleInput{
		Name:   "bundle-project",
		Bundle: bundle,
	})
	if err != nil {
		t.Fatalf("ImportBundle: %v", err)
	}
	if imported.Project.Name != "bundle-project" {
		t.Fatalf("project name: %+v", imported.Project)
	}
	repo2 := mustRepo(t, svc2, imported.Project.ID)
	if commitCount(t, repo2) != origCount {
		t.Fatalf("commit count: %d vs %d", commitCount(t, repo2), origCount)
	}
	// Same HEAD.
	h1, _ := repo.Revision(context.Background())
	h2, _ := repo2.Revision(context.Background())
	if h1 != h2 {
		t.Fatalf("HEAD differs: %s vs %s", h1, h2)
	}
	// Invalid bundle rejected.
	if _, err := svc2.ImportBundle(context.Background(), ImportBundleInput{
		Name: "bad-bundle", Bundle: []byte("not a bundle"),
	}); err == nil {
		t.Fatal("invalid bundle accepted")
	}
}

func commitCount(t *testing.T, repo *Repo) int {
	t.Helper()
	out, err := gitOutput(context.Background(), repo.Dir, "rev-list", "--count", "HEAD")
	if err != nil {
		t.Fatal(err)
	}
	var n int
	_, _ = sscan(out, &n)
	return n
}

func sscan(s string, n *int) (int, error) {
	return 1, sscanInt(s, n)
}

func sscanInt(s string, n *int) error {
	v := 0
	for _, c := range s {
		if c < '0' || c > '9' {
			break
		}
		v = v*10 + int(c-'0')
	}
	*n = v
	return nil
}

var _ = os.RemoveAll
var _ = filepath.Join
var _ = strings.TrimSpace
var _ = time.Now
var _ = errors.Is

// zipEntries converts exported zip bytes into the import payload shape.
func zipEntries(t *testing.T, data []byte) []ZipFile {
	t.Helper()
	zr, err := zip.NewReader(bytes.NewReader(data), int64(len(data)))
	if err != nil {
		t.Fatal(err)
	}
	var out []ZipFile
	for _, f := range zr.File {
		rc, err := f.Open()
		if err != nil {
			t.Fatal(err)
		}
		var buf bytes.Buffer
		if _, err := buf.ReadFrom(rc); err != nil {
			t.Fatal(err)
		}
		_ = rc.Close()
		out = append(out, ZipFile{Path: f.Name, Content: base64.StdEncoding.EncodeToString(buf.Bytes())})
	}
	return out
}
