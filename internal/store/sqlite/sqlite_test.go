package sqlite

import (
	"path/filepath"
	"testing"
)

func TestOpenRunsMigrations(t *testing.T) {
	dir := t.TempDir()
	db, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	for _, name := range []string{"users", "sessions", "schema_migrations"} {
		var n int
		if err := db.QueryRow(
			`SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?`, name,
		).Scan(&n); err != nil {
			t.Fatalf("query %s: %v", name, err)
		}
		if n != 1 {
			t.Fatalf("table %s missing (n=%d)", name, n)
		}
	}
	db.Close()

	// Reopening the same directory must be idempotent.
	db2, err := Open(dir)
	if err != nil {
		t.Fatalf("second Open: %v", err)
	}
	db2.Close()
}

func TestOpenCreatesNestedDataDir(t *testing.T) {
	dir := filepath.Join(t.TempDir(), "nested", "data")
	db, err := Open(dir)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	db.Close()
}
