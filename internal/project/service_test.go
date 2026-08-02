package project

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"agentdocs/internal/store/sqlite"
)

type fakeClock struct{ now time.Time }

func (f fakeClock) Now() time.Time { return f.now }

func newService(t *testing.T) (*Service, *Store) {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	dataDir := t.TempDir()
	svc := NewService(db, dataDir, fakeClock{now: time.Date(2026, 8, 2, 12, 0, 0, 0, time.UTC)})
	return svc, NewStore(db)
}

func TestServiceCreateInitializesRepoAndStore(t *testing.T) {
	svc, store := newService(t)
	p, err := svc.Create(context.Background(), CreateInput{Name: "docs-site", Description: "产品文档"})
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	if p.ID == "" || p.Name != "docs-site" || p.Description != "产品文档" {
		t.Fatalf("unexpected project: %+v", p)
	}
	if p.IsArchived() {
		t.Fatal("new project must not be archived")
	}
	// Store has the record with a relative repo dir.
	got, err := store.GetByID(context.Background(), p.ID)
	if err != nil {
		t.Fatalf("store lookup: %v", err)
	}
	if got.RepoDir != "repos/"+p.ID+"/repo.git" {
		t.Fatalf("unexpected repo_dir %q", got.RepoDir)
	}
	// The bare repo exists on disk with a README root commit.
	abs := filepath.Join(svc.reposRoot, p.ID, "repo.git")
	if _, err := os.Stat(filepath.Join(abs, "HEAD")); err != nil {
		t.Fatalf("repo missing: %v", err)
	}
	head, err := gitOutput(context.Background(), abs, "rev-parse", "HEAD")
	if err != nil || head == "" {
		t.Fatalf("no HEAD commit: %v %q", err, head)
	}
}

func TestServiceCreateInvalidNameLeavesNoRepo(t *testing.T) {
	svc, _ := newService(t)
	_, err := svc.Create(context.Background(), CreateInput{Name: "Bad Name!"})
	if !errors.Is(err, ErrInvalid) {
		t.Fatalf("want ErrInvalid, got %v", err)
	}
	entries, err := os.ReadDir(svc.reposRoot)
	if errors.Is(err, os.ErrNotExist) {
		return // no repos dir at all — nothing leaked
	}
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 0 {
		t.Fatalf("invalid create left repo dirs: %v", entries)
	}
}

func TestServiceCreateDuplicateName(t *testing.T) {
	svc, _ := newService(t)
	if _, err := svc.Create(context.Background(), CreateInput{Name: "docs-site"}); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.Create(context.Background(), CreateInput{Name: "docs-site"}); !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
}

func TestServiceListAndArchive(t *testing.T) {
	svc, _ := newService(t)
	a, err := svc.Create(context.Background(), CreateInput{Name: "alpha"})
	if err != nil {
		t.Fatal(err)
	}
	b, err := svc.Create(context.Background(), CreateInput{Name: "beta"})
	if err != nil {
		t.Fatal(err)
	}
	all, err := svc.List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(all) != 2 {
		t.Fatalf("want 2 projects, got %d", len(all))
	}

	archived, err := svc.Archive(context.Background(), a.ID)
	if err != nil {
		t.Fatalf("Archive: %v", err)
	}
	if !archived.IsArchived() {
		t.Fatal("archived project must report archived")
	}
	// Idempotent second archive.
	again, err := svc.Archive(context.Background(), a.ID)
	if err != nil {
		t.Fatalf("second Archive: %v", err)
	}
	if *again.ArchivedAt != *archived.ArchivedAt {
		t.Fatal("second archive must not change timestamp")
	}
	// Archived projects remain visible in the list and via Get.
	if got, err := svc.Get(context.Background(), a.ID); err != nil || !got.IsArchived() {
		t.Fatalf("Get archived: %v %+v", err, got)
	}
	if _, err := svc.Archive(context.Background(), "prj_missing"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
	if _, err := svc.Get(context.Background(), "prj_missing"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
	_ = b
}

func TestServiceCreateFailsWhenStoreFailsAndCleansRepo(t *testing.T) {
	svc, _ := newService(t)
	// A duplicate name passes repo init but fails store insert (UNIQUE),
	// so the repo directory must be cleaned up.
	if _, err := svc.Create(context.Background(), CreateInput{Name: "dup"}); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.Create(context.Background(), CreateInput{Name: "dup"}); !errors.Is(err, ErrConflict) {
		t.Fatalf("want ErrConflict, got %v", err)
	}
	entries, err := os.ReadDir(svc.reposRoot)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 {
		t.Fatalf("want 1 repo dir after failed duplicate create, got %d", len(entries))
	}
}
