package maintenance

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	storesqlite "agentdocs/internal/store/sqlite"
)

func TestBackupAndRestoreRoundTrip(t *testing.T) {
	source := filepath.Join(t.TempDir(), "source")
	db, err := storesqlite.Open(source)
	if err != nil {
		t.Fatalf("open source: %v", err)
	}
	if _, err := db.Exec(`INSERT INTO projects
		(id, name, description, repo_dir, created_at, updated_at)
		VALUES ('prj_1', 'docs', '', 'repos/prj_1/repo.git', '2026-08-12T00:00:00Z', '2026-08-12T00:00:00Z')`); err != nil {
		t.Fatalf("seed database: %v", err)
	}
	if err := db.Close(); err != nil {
		t.Fatalf("close source: %v", err)
	}
	repoDir := filepath.Join(source, "repos", "prj_1", "repo.git")
	if out, err := exec.Command("git", "init", "--bare", "--initial-branch=main", repoDir).CombinedOutput(); err != nil {
		t.Fatalf("init repo: %v: %s", err, out)
	}

	archive := filepath.Join(t.TempDir(), "agentdocs-backup.tar.gz")
	if err := Backup(context.Background(), source, archive); err != nil {
		t.Fatalf("backup: %v", err)
	}

	target := filepath.Join(t.TempDir(), "target")
	if err := Restore(context.Background(), archive, target, true); err != nil {
		t.Fatalf("restore: %v", err)
	}
	got, err := os.ReadFile(filepath.Join(target, "repos", "prj_1", "repo.git", "HEAD"))
	if err != nil || string(got) != "ref: refs/heads/main\n" {
		t.Fatalf("restored repository marker = %q, %v", got, err)
	}
	restored, err := storesqlite.Open(target)
	if err != nil {
		t.Fatalf("open restored database: %v", err)
	}
	defer restored.Close()
	var name string
	if err := restored.QueryRow(`SELECT name FROM projects WHERE id = 'prj_1'`).Scan(&name); err != nil {
		t.Fatalf("read restored project: %v", err)
	}
	if name != "docs" {
		t.Fatalf("restored project name = %q", name)
	}
}

func TestRestoreRejectsInvalidArchiveBeforeReplacingData(t *testing.T) {
	target := filepath.Join(t.TempDir(), "data")
	if err := os.MkdirAll(target, 0o755); err != nil {
		t.Fatal(err)
	}
	marker := filepath.Join(target, "keep.txt")
	if err := os.WriteFile(marker, []byte("keep"), 0o644); err != nil {
		t.Fatal(err)
	}
	bad := filepath.Join(t.TempDir(), "bad.tar.gz")
	if err := os.WriteFile(bad, []byte("not an archive"), 0o600); err != nil {
		t.Fatal(err)
	}

	if err := Restore(context.Background(), bad, target, true); err == nil {
		t.Fatal("invalid archive restored successfully")
	}
	got, err := os.ReadFile(marker)
	if err != nil || string(got) != "keep" {
		t.Fatalf("existing data changed: %q, %v", got, err)
	}
}
