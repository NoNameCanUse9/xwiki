package maintenance

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"

	storesqlite "xwiki/internal/store/sqlite"
)

func TestDoctorReportsHealthyDataDirectory(t *testing.T) {
	dataDir := t.TempDir()
	db, err := storesqlite.Open(dataDir)
	if err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}

	report, err := Doctor(context.Background(), dataDir)
	if err != nil {
		t.Fatalf("doctor: %v", err)
	}
	if !report.Healthy {
		t.Fatalf("healthy data reported unhealthy: %+v", report.Checks)
	}
	for _, name := range []string{"git", "data_directory", "sqlite", "repositories", "worktrees", "search_index"} {
		if report.Check(name) == nil {
			t.Fatalf("missing %q check: %+v", name, report.Checks)
		}
	}
}

func TestDoctorReportsMissingRepository(t *testing.T) {
	dataDir := t.TempDir()
	db, err := storesqlite.Open(dataDir)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.Exec(`INSERT INTO projects
		(id, name, description, repo_dir, created_at, updated_at)
		VALUES ('prj_missing', 'missing', '', 'repos/prj_missing/repo.git', '2026-08-12T00:00:00Z', '2026-08-12T00:00:00Z')`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}

	report, err := Doctor(context.Background(), dataDir)
	if err != nil {
		t.Fatalf("doctor: %v", err)
	}
	if report.Healthy {
		t.Fatal("missing repository reported healthy")
	}
	check := report.Check("repositories")
	if check == nil || check.Status != CheckError || check.Detail == "" {
		t.Fatalf("repository check = %+v", check)
	}
}

func TestDoctorReportsCorruptRepository(t *testing.T) {
	dataDir := t.TempDir()
	db, err := storesqlite.Open(dataDir)
	if err != nil {
		t.Fatal(err)
	}
	repoDir := filepath.Join(dataDir, "repos", "prj_bad", "repo.git")
	if out, err := exec.Command("git", "init", "--bare", "--initial-branch=main", repoDir).CombinedOutput(); err != nil {
		t.Fatalf("init repo: %v: %s", err, out)
	}
	if _, err := db.Exec(`INSERT INTO projects
		(id, name, description, repo_dir, created_at, updated_at)
		VALUES ('prj_bad', 'bad', '', 'repos/prj_bad/repo.git', '2026-08-12T00:00:00Z', '2026-08-12T00:00:00Z')`); err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	// A bare repository with a missing object is observably corrupt to fsck.
	if err := os.WriteFile(filepath.Join(repoDir, "refs", "heads", "main"), []byte("1111111111111111111111111111111111111111\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	report, err := Doctor(context.Background(), dataDir)
	if err != nil {
		t.Fatalf("doctor: %v", err)
	}
	if report.Healthy || report.Check("repositories").Status != CheckError {
		t.Fatalf("corrupt repository report = %+v", report)
	}
}
