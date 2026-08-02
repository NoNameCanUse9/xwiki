package project

import (
	"context"
	"errors"
	"testing"
	"time"

	"agentdocs/internal/store/sqlite"
)

func newStore(t *testing.T) *Store {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return NewStore(db)
}

func sampleProject(id, name string) *Project {
	now := time.Now().UTC().Truncate(time.Second)
	return &Project{
		ID: id, Name: name, Description: "desc", RepoDir: "repos/" + id + "/repo.git",
		CreatedAt: now, UpdatedAt: now,
	}
}

func TestCreateAndGetByID(t *testing.T) {
	s := newStore(t)
	p := sampleProject("prj_1", "docs-site")
	if err := s.Create(context.Background(), p); err != nil {
		t.Fatalf("Create: %v", err)
	}
	got, err := s.GetByID(context.Background(), "prj_1")
	if err != nil {
		t.Fatalf("GetByID: %v", err)
	}
	if got.ID != "prj_1" || got.Name != "docs-site" || got.RepoDir != "repos/prj_1/repo.git" {
		t.Fatalf("unexpected project: %+v", got)
	}
	if got.IsArchived() {
		t.Fatal("fresh project should not be archived")
	}
	if _, err := s.GetByID(context.Background(), "missing"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

func TestCreateDuplicateName(t *testing.T) {
	s := newStore(t)
	if err := s.Create(context.Background(), sampleProject("prj_1", "docs-site")); err != nil {
		t.Fatal(err)
	}
	dup := sampleProject("prj_2", "docs-site")
	if err := s.Create(context.Background(), dup); err == nil {
		t.Fatal("duplicate name allowed")
	}
}

func TestListOrderedByCreatedAtDesc(t *testing.T) {
	s := newStore(t)
	base := time.Now().UTC().Truncate(time.Second)
	a := sampleProject("prj_a", "alpha")
	a.CreatedAt = base.Add(-2 * time.Hour)
	b := sampleProject("prj_b", "beta")
	b.CreatedAt = base.Add(-1 * time.Hour)
	if err := s.Create(context.Background(), a); err != nil {
		t.Fatal(err)
	}
	if err := s.Create(context.Background(), b); err != nil {
		t.Fatal(err)
	}
	got, err := s.List(context.Background())
	if err != nil {
		t.Fatalf("List: %v", err)
	}
	if len(got) != 2 {
		t.Fatalf("want 2 projects, got %d", len(got))
	}
	if got[0].Name != "beta" || got[1].Name != "alpha" {
		t.Fatalf("want beta first (newest), got %q then %q", got[0].Name, got[1].Name)
	}
}

func TestArchiveIsIdempotent(t *testing.T) {
	s := newStore(t)
	p := sampleProject("prj_1", "docs-site")
	if err := s.Create(context.Background(), p); err != nil {
		t.Fatal(err)
	}
	if err := s.Archive(context.Background(), "prj_1", time.Now().UTC()); err != nil {
		t.Fatalf("Archive: %v", err)
	}
	got, err := s.GetByID(context.Background(), "prj_1")
	if err != nil {
		t.Fatal(err)
	}
	if !got.IsArchived() {
		t.Fatal("project should be archived")
	}
	// Second archive keeps the original timestamp (idempotent).
	if err := s.Archive(context.Background(), "prj_1", time.Now().UTC().Add(time.Hour)); err != nil {
		t.Fatalf("second Archive: %v", err)
	}
	got2, err := s.GetByID(context.Background(), "prj_1")
	if err != nil {
		t.Fatal(err)
	}
	if !got2.ArchivedAt.Equal(got.ArchivedAt) {
		t.Fatal("second archive must not overwrite archived_at")
	}
	if err := s.Archive(context.Background(), "missing", time.Now().UTC()); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}
