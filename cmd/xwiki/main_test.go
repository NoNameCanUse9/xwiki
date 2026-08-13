package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	storesqlite "xwiki/internal/store/sqlite"
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
	if err := run([]string{"backup", "create", "-data-dir", source, "-output", archive}); err != nil {
		t.Fatalf("backup create command: %v", err)
	}
	target := filepath.Join(t.TempDir(), "restored")
	if err := run([]string{"backup", "restore", "-input", archive, "-data-dir", target}); err != nil {
		t.Fatalf("backup restore command: %v", err)
	}
	if _, err := os.Stat(filepath.Join(target, "xwiki.db")); err != nil {
		t.Fatalf("restored database: %v", err)
	}
	if err := run([]string{"doctor", "-data-dir", target}); err != nil {
		t.Fatalf("doctor on restored data: %v", err)
	}
}

func TestUsageIncludesMaintenanceCommands(t *testing.T) {
	for _, command := range []string{"backup", "doctor", "reindex"} {
		if !strings.Contains(usageText, "xwiki "+command) {
			t.Fatalf("usage missing %q", command)
		}
	}
}
