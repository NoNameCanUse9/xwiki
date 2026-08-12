package main

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"agentdocs/internal/maintenance"
	storesqlite "agentdocs/internal/store/sqlite"
)

func TestMaintenanceCommandsBackupAndRestoreData(t *testing.T) {
	source := filepath.Join(t.TempDir(), "source")
	db, err := storesqlite.Open(source)
	if err != nil {
		t.Fatal(err)
	}
	if err := db.Close(); err != nil {
		t.Fatal(err)
	}
	archive := filepath.Join(t.TempDir(), "backup.tar.gz")
	if err := run([]string{"backup", "--data-dir", source, "--output", archive}); err != nil {
		t.Fatalf("backup command: %v", err)
	}
	target := filepath.Join(t.TempDir(), "restored")
	if err := run([]string{"restore", "--data-dir", target, "--replace", archive}); err != nil {
		t.Fatalf("restore command: %v", err)
	}
	if _, err := os.Stat(filepath.Join(target, "agentdocs.db")); err != nil {
		t.Fatalf("restored database: %v", err)
	}
	report, err := maintenance.Doctor(context.Background(), target)
	if err != nil || !report.Healthy {
		t.Fatalf("restored data doctor: healthy=%v err=%v checks=%+v", report.Healthy, err, report.Checks)
	}
}

func TestUsageIncludesMaintenanceCommands(t *testing.T) {
	for _, command := range []string{"backup", "restore", "doctor", "reindex"} {
		if !strings.Contains(usageText, "agentdocs "+command) {
			t.Fatalf("usage missing %q", command)
		}
	}
}
