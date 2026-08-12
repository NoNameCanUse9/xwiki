package search

import (
	"context"
	"testing"
	"time"

	"xwiki/internal/project"
	"xwiki/internal/store/sqlite"
)

func newService(t *testing.T) (*Service, *project.Service) {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	projSvc := project.NewService(db, t.TempDir(), fakeClock{})
	return NewService(db, projSvc), projSvc
}

type fakeClock struct{}

func (fakeClock) Now() time.Time { return time.Now() }

func TestUpsertQueryDelete(t *testing.T) {
	svc, _ := newService(t)
	if _, err := svc.store.Upsert(context.Background(), &StateEntry{
		ProjectID: "prj_1", Path: "docs/guide.md", BlobSHA: "s1",
		Content: "# Guide\n\ninstallation docs for agents\n",
	}); err != nil {
		t.Fatal(err)
	}
	res, err := svc.Search(context.Background(), "prj_1", "agents", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(res) != 1 || res[0].Path != "docs/guide.md" {
		t.Fatalf("search hit wrong: %+v", res)
	}
	// Unchanged upsert is a no-op.
	changed, err := svc.store.Upsert(context.Background(), &StateEntry{
		ProjectID: "prj_1", Path: "docs/guide.md", BlobSHA: "s1", Content: "same",
	})
	if err != nil || changed {
		t.Fatalf("unchanged upsert reported change: %v %v", changed, err)
	}
	// Delete removes it.
	if err := svc.store.Delete(context.Background(), "prj_1", "docs/guide.md"); err != nil {
		t.Fatal(err)
	}
	res, _ = svc.Search(context.Background(), "prj_1", "agents", 10)
	if len(res) != 0 {
		t.Fatalf("search after delete: %+v", res)
	}
}

func TestBuildMatchExprRaw(t *testing.T) {
	// Trigram tokenizer: quoted phrases, no prefix wildcard.
	if got := BuildMatchExprRaw(`say "hello" world`); got != `"say" """hello""" "world"` {
		t.Fatalf("match expr wrong: %q", got)
	}
}

func TestEscapeLike(t *testing.T) {
	if got := escapeLike(`a_b%c\d`); got != `a\_b\%c\\d` {
		t.Fatalf("escapeLike: %q", got)
	}
}

func TestQueryLikeEscapesWildcards(t *testing.T) {
	svc, _ := newService(t)
	if _, err := svc.store.Upsert(context.Background(), &StateEntry{
		ProjectID: "prj_pct", Path: "a.md", BlobSHA: "1", Content: "price is 50% off",
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.store.Upsert(context.Background(), &StateEntry{
		ProjectID: "prj_pct", Path: "b.md", BlobSHA: "2", Content: "fifty 50 percent",
	}); err != nil {
		t.Fatal(err)
	}
	// "%" is a 1-char query, so it hits the LIKE path; the wildcard must be
	// escaped or b.md (contains "50" but no literal %) would match too.
	res, err := svc.Search(context.Background(), "prj_pct", "%", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(res) != 1 || res[0].Path != "a.md" {
		t.Fatalf("query '%%': %+v", res)
	}
}

func TestCJKShortTermCombined(t *testing.T) {
	// Two 2-char CJK words must not kill the query: any short term falls
	// back to LIKE with AND across all terms.
	svc, _ := newService(t)
	if _, err := svc.store.Upsert(context.Background(), &StateEntry{
		ProjectID: "prj_cjk", Path: "docs/install.md", BlobSHA: "s1",
		Content: "安装指南 installation guide",
	}); err != nil {
		t.Fatal(err)
	}
	for _, q := range []string{"安装", "安装指南", "安装 指南", "指南 安装"} {
		res, err := svc.Search(context.Background(), "prj_cjk", q, 10)
		if err != nil {
			t.Fatalf("%q: %v", q, err)
		}
		if len(res) != 1 || res[0].Path != "docs/install.md" {
			t.Fatalf("query %q: got %+v, want docs/install.md", q, res)
		}
	}
}

func TestReindexProjectIncremental(t *testing.T) {
	svc, projSvc := newService(t)
	p, err := projSvc.Create(context.Background(), project.CreateInput{Name: "search-site"})
	if err != nil {
		t.Fatal(err)
	}
	repo, _ := projSvc.OpenRepo(context.Background(), p.ID)

	// Initial reindex: README.md only.
	stats, err := svc.ReindexProject(context.Background(), p.ID)
	if err != nil {
		t.Fatal(err)
	}
	if stats.Indexed != 1 || stats.Removed != 0 {
		t.Fatalf("initial reindex: %+v", stats)
	}
	// Idempotent second pass.
	stats, err = svc.ReindexProject(context.Background(), p.ID)
	if err != nil || stats.Indexed != 0 {
		t.Fatalf("second reindex: %+v %v", stats, err)
	}
	// Write docs via changesets, then reindex picks them up.
	base, _ := repo.Revision(context.Background())
	if _, err := projSvc.ApplyChangeset(context.Background(), p.ID, project.ChangesetInput{
		BaseRevision: base,
		Message:      "add searchable",
		Changes: []project.Change{
			{Op: "create", Path: "docs/findme.md", Content: "# Findme\n\nunique keyword zanzibar\n"},
			{Op: "create", Path: "docs/other.md", Content: "# Other\n\nplain\n"},
		},
	}, project.CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	stats, err = svc.ReindexProject(context.Background(), p.ID)
	if err != nil || stats.Indexed != 2 {
		t.Fatalf("after write reindex: %+v %v", stats, err)
	}
	res, err := svc.Search(context.Background(), p.ID, "zanzibar", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(res) != 1 || res[0].Path != "docs/findme.md" {
		t.Fatalf("zanzibar hit: %+v", res)
	}
	// Delete a file, reindex removes it from search.
	base, _ = repo.Revision(context.Background())
	if _, err := projSvc.ApplyChangeset(context.Background(), p.ID, project.ChangesetInput{
		BaseRevision: base,
		Message:      "remove findme",
		Changes:      []project.Change{{Op: "delete", Path: "docs/findme.md"}},
	}, project.CommitAuthor{Name: "Test Author", Email: "test@xwiki.local"}); err != nil {
		t.Fatal(err)
	}
	stats, err = svc.ReindexProject(context.Background(), p.ID)
	if err != nil || stats.Removed != 1 {
		t.Fatalf("after delete reindex: %+v %v", stats, err)
	}
	res, _ = svc.Search(context.Background(), p.ID, "zanzibar", 10)
	if len(res) != 0 {
		t.Fatalf("deleted doc still searchable: %+v", res)
	}
}
