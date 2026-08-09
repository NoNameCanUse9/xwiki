package agent

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"

	"agentdocs/internal/store/sqlite"
)

type fakeClock struct{ now time.Time }

func (f fakeClock) Now() time.Time { return f.now }

func newService(t *testing.T) *Service {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return NewService(db, fakeClock{now: time.Date(2026, 8, 2, 12, 0, 0, 0, time.UTC)})
}

func sampleInput() CreateInput {
	return CreateInput{
		Name:       "ci-bot",
		Scope:      "write",
		ProjectIDs: []string{"prj_1"},
	}
}

func TestCreateTokenSecretShownOnceAndHashed(t *testing.T) {
	svc := newService(t)
	created, err := svc.Create(context.Background(), sampleInput())
	if err != nil {
		t.Fatal(err)
	}
	if created.Secret == "" || len(created.Secret) != 3+32 {
		t.Fatalf("bad secret %q", created.Secret)
	}
	if created.Token.ID == "" || created.Token.Scope != "write" {
		t.Fatalf("bad token: %+v", created.Token)
	}
	// Secret resolves; hash is not stored in plaintext anywhere.
	got, err := svc.Authorize(context.Background(), created.Secret, "prj_1", true)
	if err != nil {
		t.Fatalf("authorize: %v", err)
	}
	if got.ID != created.Token.ID {
		t.Fatalf("wrong token: %+v", got)
	}
	// Unknown secret.
	if _, err := svc.Authorize(context.Background(), "ad_"+strings.Repeat("0", 32), "prj_1", true); !errors.Is(err, ErrNotFound) {
		t.Fatalf("unknown: want ErrNotFound, got %v", err)
	}
}

func TestAuthorizeMatrix(t *testing.T) {
	svc := newService(t)
	created, err := svc.Create(context.Background(), sampleInput())
	if err != nil {
		t.Fatal(err)
	}
	secret := created.Secret

	cases := []struct {
		name    string
		project string
		write   bool
		wantErr error
	}{
		{"read within project", "prj_1", false, nil},
		{"write any path within project", "prj_1", true, nil},
		{"write outside project", "prj_2", true, ErrForbidden},
		{"read outside project", "prj_2", false, ErrForbidden},
	}
	for _, c := range cases {
		_, err := svc.Authorize(context.Background(), secret, c.project, c.write)
		if !errors.Is(err, c.wantErr) {
			t.Fatalf("%s: want %v, got %v", c.name, c.wantErr, err)
		}
	}
}

func TestRevokedTokenRejected(t *testing.T) {
	svc := newService(t)
	created, _ := svc.Create(context.Background(), sampleInput())
	if err := svc.Revoke(context.Background(), created.Token.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.Authorize(context.Background(), created.Secret, "prj_1", true); !errors.Is(err, ErrTokenRevoked) {
		t.Fatalf("want ErrTokenRevoked, got %v", err)
	}
	// List still shows it (with revoked flag) and revoke is idempotent.
	list, err := svc.List(context.Background())
	if err != nil || len(list) != 1 || list[0].RevokedAt.IsZero() {
		t.Fatalf("list after revoke: %v %+v", err, list)
	}
	if err := svc.Revoke(context.Background(), created.Token.ID); err != nil {
		t.Fatalf("second revoke: %v", err)
	}
}

func TestCreateRejectsInvalid(t *testing.T) {
	svc := newService(t)
	cases := []CreateInput{
		{Name: "", Scope: "write", ProjectIDs: []string{"p"}},
		{Name: "x", Scope: "admin", ProjectIDs: []string{"p"}},
		{Name: "x", Scope: "read"}, // no projects

	}
	for i, c := range cases {
		if _, err := svc.Create(context.Background(), c); !errors.Is(err, ErrInvalid) {
			t.Fatalf("case %d: want ErrInvalid, got %v", i, err)
		}
	}
}

func TestIdempotencyReplay(t *testing.T) {
	svc := newService(t)
	hash := RequestHash([]byte(`{"a":1}`))
	calls := 0
	run := func() (string, error) {
		calls++
		return `{"commit":"c1"}`, nil
	}
	res, replayed, err := svc.ApplyIdempotent(context.Background(), "key-1", "prj_1", hash, run)
	if err != nil || replayed || res != `{"commit":"c1"}` {
		t.Fatalf("first: %v %v %q", err, replayed, res)
	}
	res, replayed, err = svc.ApplyIdempotent(context.Background(), "key-1", "prj_1", hash, run)
	if err != nil || !replayed || res != `{"commit":"c1"}` || calls != 1 {
		t.Fatalf("replay: %v %v calls=%d", err, replayed, calls)
	}
	// Different payload, same key -> conflict.
	if _, _, err := svc.ApplyIdempotent(context.Background(), "key-1", "prj_1", RequestHash([]byte(`{"a":2}`)), run); !errors.Is(err, ErrIdempotencyConflict) {
		t.Fatalf("conflict: want ErrIdempotencyConflict, got %v", err)
	}
}

func TestAuditAppendAndList(t *testing.T) {
	svc := newService(t)
	if err := svc.Audit(context.Background(), "token", "tok_1", "prj_1", "change", "docs/a.md", "", "req_1"); err != nil {
		t.Fatal(err)
	}
	entries, err := svc.store.RecentAudit(context.Background(), "prj_1", 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 1 || entries[0].Action != "change" || entries[0].ActorType != "token" {
		t.Fatalf("audit wrong: %+v", entries)
	}
	all, err := svc.store.RecentAudit(context.Background(), "", 10)
	if err != nil || len(all) != 1 {
		t.Fatalf("all audit: %v %d", err, len(all))
	}
}
