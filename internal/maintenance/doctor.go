package maintenance

import (
	"context"
	"database/sql"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	_ "modernc.org/sqlite"
)

type CheckStatus string

const (
	CheckOK      CheckStatus = "ok"
	CheckWarning CheckStatus = "warning"
	CheckError   CheckStatus = "error"
)

type DoctorCheck struct {
	Name   string      `json:"name"`
	Status CheckStatus `json:"status"`
	Detail string      `json:"detail,omitempty"`
}

type DoctorReport struct {
	Healthy bool          `json:"healthy"`
	Checks  []DoctorCheck `json:"checks"`
}

func (r DoctorReport) Check(name string) *DoctorCheck {
	for i := range r.Checks {
		if r.Checks[i].Name == name {
			return &r.Checks[i]
		}
	}
	return nil
}

// Doctor diagnoses an offline data directory without modifying it.
func Doctor(ctx context.Context, dataDir string) (DoctorReport, error) {
	lock, err := AcquireDataLock(dataDir)
	if err != nil {
		return DoctorReport{}, err
	}
	defer lock.Close()
	report := DoctorReport{Healthy: true}
	add := func(name string, status CheckStatus, detail string) {
		report.Checks = append(report.Checks, DoctorCheck{Name: name, Status: status, Detail: detail})
		if status == CheckError {
			report.Healthy = false
		}
	}

	gitVersion, err := exec.CommandContext(ctx, "git", "--version").CombinedOutput()
	if err != nil {
		add("git", CheckError, err.Error())
	} else {
		add("git", CheckOK, strings.TrimSpace(string(gitVersion)))
	}
	absData, err := filepath.Abs(dataDir)
	if err != nil {
		return DoctorReport{}, err
	}
	if info, err := os.Stat(absData); err != nil || !info.IsDir() {
		add("data_directory", CheckError, fmt.Sprintf("not accessible: %v", err))
	} else {
		probe, err := os.CreateTemp(absData, ".doctor-write-*")
		if err != nil {
			add("data_directory", CheckError, "not writable: "+err.Error())
		} else {
			name := probe.Name()
			_ = probe.Close()
			_ = os.Remove(name)
			add("data_directory", CheckOK, absData)
		}
	}

	dbPath := filepath.Join(absData, "xwiki.db")
	db, err := sql.Open("sqlite", "file:"+filepath.ToSlash(dbPath)+"?mode=ro")
	if err != nil {
		add("sqlite", CheckError, err.Error())
		add("repositories", CheckError, "database unavailable")
		add("worktrees", CheckError, "database unavailable")
		add("search_index", CheckError, "database unavailable")
		return report, nil
	}
	defer db.Close()
	var integrity string
	if err := db.QueryRowContext(ctx, "PRAGMA integrity_check").Scan(&integrity); err != nil || integrity != "ok" {
		add("sqlite", CheckError, fmt.Sprintf("integrity=%s error=%v", integrity, err))
	} else {
		add("sqlite", CheckOK, "integrity check passed")
	}

	rows, err := db.QueryContext(ctx, "SELECT id, repo_dir FROM projects")
	if err != nil {
		add("repositories", CheckError, err.Error())
	} else {
		var repoErrors []string
		var worktreeErrors []string
		for rows.Next() {
			var id, rel string
			if err := rows.Scan(&id, &rel); err != nil {
				repoErrors = append(repoErrors, err.Error())
				continue
			}
			repo := filepath.Join(absData, filepath.FromSlash(rel))
			if !pathWithin(absData, repo) {
				repoErrors = append(repoErrors, id+": repo path escapes data directory")
				continue
			}
			if info, err := os.Stat(repo); err != nil || !info.IsDir() {
				repoErrors = append(repoErrors, id+": repository missing")
				continue
			}
			if out, err := exec.CommandContext(ctx, "git", "--git-dir", repo, "fsck", "--no-dangling").CombinedOutput(); err != nil {
				repoErrors = append(repoErrors, id+": "+strings.TrimSpace(string(out)))
			}
			if out, err := exec.CommandContext(ctx, "git", "--git-dir", repo, "worktree", "list", "--porcelain").CombinedOutput(); err != nil {
				worktreeErrors = append(worktreeErrors, id+": "+err.Error())
			} else if strings.Count(string(out), "worktree ") > 1 {
				worktreeErrors = append(worktreeErrors, id+": linked worktrees remain")
			}
		}
		_ = rows.Close()
		if len(repoErrors) > 0 {
			add("repositories", CheckError, strings.Join(repoErrors, "; "))
		} else {
			add("repositories", CheckOK, "all registered repositories passed fsck")
		}
		if len(worktreeErrors) > 0 {
			add("worktrees", CheckError, strings.Join(worktreeErrors, "; "))
		} else {
			add("worktrees", CheckOK, "no orphaned linked worktrees")
		}
	}

	var dirty int
	if err := db.QueryRowContext(ctx, `SELECT count(*) FROM sqlite_master WHERE type='table' AND name='project_index_state'`).Scan(&dirty); err != nil {
		add("search_index", CheckError, err.Error())
	} else if dirty == 0 {
		add("search_index", CheckWarning, "index health table is not installed")
	} else if err := db.QueryRowContext(ctx, `
		SELECT count(*)
		FROM projects p
		LEFT JOIN project_index_state s ON s.project_id = p.id
		WHERE p.deleted_at IS NULL AND (s.project_id IS NULL OR s.status != 'clean')`).Scan(&dirty); err != nil {
		add("search_index", CheckError, err.Error())
	} else if dirty > 0 {
		add("search_index", CheckError, fmt.Sprintf("%d project indexes are not clean", dirty))
	} else {
		add("search_index", CheckOK, "all project indexes are clean")
	}
	return report, nil
}
