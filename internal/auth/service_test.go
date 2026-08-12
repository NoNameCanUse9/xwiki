package auth

import (
	"context"
	"testing"
	"time"

	"xwiki/internal/store/sqlite"
	"xwiki/internal/user"
)

type fakeClock struct{ now time.Time }

func (f *fakeClock) Now() time.Time { return f.now }

func newService(t *testing.T) (*Service, *user.Store, *fakeClock) {
	t.Helper()
	db, err := sqlite.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	clk := &fakeClock{now: time.Date(2026, 8, 2, 12, 0, 0, 0, time.UTC)}
	users := user.NewStore(db)
	return NewService(db, clk, time.Hour), users, clk
}

func createUser(t *testing.T, users *user.Store, id, username string) {
	t.Helper()
	now := time.Now().UTC()
	hash, err := HashPassword("secret123")
	if err != nil {
		t.Fatal(err)
	}
	u := &user.User{ID: id, Username: username, DisplayName: username,
		PasswordHash: hash, IsAdmin: true, CreatedAt: now, UpdatedAt: now}
	if err := users.Create(context.Background(), u); err != nil {
		t.Fatal(err)
	}
}

func TestLoginSuccess(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	got, token, err := svc.Login(context.Background(), users, "admin", "secret123")
	if err != nil {
		t.Fatalf("Login: %v", err)
	}
	if got.ID != "usr_1" || token == "" {
		t.Fatalf("bad login result: %+v token=%q", got, token)
	}
}

func TestLoginWrongPassword(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	if _, _, err := svc.Login(context.Background(), users, "admin", "wrong"); err != ErrInvalidCredentials {
		t.Fatalf("want ErrInvalidCredentials, got %v", err)
	}
	if _, _, err := svc.Login(context.Background(), users, "nobody", "secret123"); err != ErrInvalidCredentials {
		t.Fatalf("want ErrInvalidCredentials, got %v", err)
	}
}

func TestResolveSessionValidAndExpired(t *testing.T) {
	svc, users, clk := newService(t)
	createUser(t, users, "usr_1", "admin")
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}
	ses, u, err := svc.ResolveSession(context.Background(), token)
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}
	if ses.UserID != "usr_1" || u.Username != "admin" {
		t.Fatalf("session mismatch: %+v %+v", ses, u)
	}

	// Advance past TTL: session must be rejected and removed.
	clk.now = clk.now.Add(2 * time.Hour)
	if _, _, err := svc.ResolveSession(context.Background(), token); err != ErrSessionNotFound {
		t.Fatalf("expired session: want ErrSessionNotFound, got %v", err)
	}
}

func TestResolveSessionUnknownToken(t *testing.T) {
	svc, _, _ := newService(t)
	if _, _, err := svc.ResolveSession(context.Background(), "garbage"); err != ErrSessionNotFound {
		t.Fatalf("want ErrSessionNotFound, got %v", err)
	}
}

func TestDeleteSessionByToken(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}
	if err := svc.DeleteSessionByToken(context.Background(), token); err != nil {
		t.Fatal(err)
	}
	if _, _, err := svc.ResolveSession(context.Background(), token); err != ErrSessionNotFound {
		t.Fatalf("want ErrSessionNotFound, got %v", err)
	}
}

func TestCreateResolveConsumePasswordReset(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")

	token, err := svc.CreatePasswordReset(context.Background(), "usr_1", 30*time.Minute)
	if err != nil {
		t.Fatal(err)
	}
	if token == "" {
		t.Fatal("empty reset token")
	}

	// Valid token resolves to the user.
	userID, err := svc.ResolvePasswordReset(context.Background(), token)
	if err != nil {
		t.Fatalf("resolve: %v", err)
	}
	if userID != "usr_1" {
		t.Fatalf("want usr_1, got %s", userID)
	}

	// Consuming makes the token single-use.
	if err := svc.ConsumePasswordReset(context.Background(), token); err != nil {
		t.Fatal(err)
	}
	if _, err := svc.ResolvePasswordReset(context.Background(), token); err != ErrInvalidResetToken {
		t.Fatalf("replay: want ErrInvalidResetToken, got %v", err)
	}
}

func TestPasswordResetExpires(t *testing.T) {
	svc, users, clk := newService(t)
	createUser(t, users, "usr_1", "admin")

	token, err := svc.CreatePasswordReset(context.Background(), "usr_1", 30*time.Minute)
	if err != nil {
		t.Fatal(err)
	}
	clk.now = clk.now.Add(31 * time.Minute)
	if _, err := svc.ResolvePasswordReset(context.Background(), token); err != ErrInvalidResetToken {
		t.Fatalf("expired: want ErrInvalidResetToken, got %v", err)
	}
}

func TestPasswordResetUnknownToken(t *testing.T) {
	svc, _, _ := newService(t)
	if _, err := svc.ResolvePasswordReset(context.Background(), "garbage"); err != ErrInvalidResetToken {
		t.Fatalf("want ErrInvalidResetToken, got %v", err)
	}
}

func TestDeleteSessionsByUser(t *testing.T) {
	svc, users, _ := newService(t)
	createUser(t, users, "usr_1", "admin")
	token, err := svc.CreateSession(context.Background(), "usr_1")
	if err != nil {
		t.Fatal(err)
	}
	if err := svc.DeleteSessionsByUser(context.Background(), "usr_1"); err != nil {
		t.Fatal(err)
	}
	if _, _, err := svc.ResolveSession(context.Background(), token); err != ErrSessionNotFound {
		t.Fatalf("want ErrSessionNotFound, got %v", err)
	}
}
