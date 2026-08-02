package user

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

func TestCreateAndGetByUsername(t *testing.T) {
	s := newStore(t)
	now := time.Now().UTC()
	u := &User{ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "hash", IsAdmin: true, CreatedAt: now, UpdatedAt: now}
	if err := s.Create(context.Background(), u); err != nil {
		t.Fatalf("Create: %v", err)
	}
	got, err := s.GetByUsername(context.Background(), "admin")
	if err != nil {
		t.Fatalf("GetByUsername: %v", err)
	}
	if got.ID != "usr_1" || got.Username != "admin" || !got.IsAdmin {
		t.Fatalf("unexpected user: %+v", got)
	}
	if _, err := s.GetByUsername(context.Background(), "nobody"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}

func TestCreateDuplicateUsername(t *testing.T) {
	s := newStore(t)
	now := time.Now().UTC()
	u := &User{ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "hash", CreatedAt: now, UpdatedAt: now}
	if err := s.Create(context.Background(), u); err != nil {
		t.Fatal(err)
	}
	u2 := *u
	u2.ID = "usr_2"
	if err := s.Create(context.Background(), &u2); err == nil {
		t.Fatal("duplicate username allowed")
	}
}

func TestUpdatePassword(t *testing.T) {
	s := newStore(t)
	now := time.Now().UTC()
	u := &User{ID: "usr_1", Username: "admin", DisplayName: "Admin",
		PasswordHash: "old", CreatedAt: now, UpdatedAt: now}
	if err := s.Create(context.Background(), u); err != nil {
		t.Fatal(err)
	}
	if err := s.UpdatePassword(context.Background(), "usr_1", "new"); err != nil {
		t.Fatalf("UpdatePassword: %v", err)
	}
	got, err := s.GetByUsername(context.Background(), "admin")
	if err != nil {
		t.Fatal(err)
	}
	if got.PasswordHash != "new" {
		t.Fatalf("password hash not updated: %q", got.PasswordHash)
	}
	if err := s.UpdatePassword(context.Background(), "usr_404", "x"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}
