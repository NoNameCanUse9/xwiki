package maintenance

import (
	"archive/tar"
	"compress/gzip"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	storesqlite "agentdocs/internal/store/sqlite"
)

const backupFormatVersion = 1

type backupManifest struct {
	FormatVersion int       `json:"format_version"`
	CreatedAt     time.Time `json:"created_at"`
}

// Backup writes a complete, offline snapshot of an AgentDocs data directory.
func Backup(ctx context.Context, dataDir, output string) error {
	lock, err := AcquireDataLock(dataDir)
	if err != nil {
		return err
	}
	defer lock.Close()

	absData, err := filepath.Abs(dataDir)
	if err != nil {
		return fmt.Errorf("resolve data directory: %w", err)
	}
	if _, err := os.Stat(filepath.Join(absData, "agentdocs.db")); err != nil {
		return fmt.Errorf("data directory is not initialized: %w", err)
	}
	db, err := storesqlite.Open(absData)
	if err != nil {
		return fmt.Errorf("open database for backup: %w", err)
	}
	if _, err := db.ExecContext(ctx, "PRAGMA wal_checkpoint(TRUNCATE)"); err != nil {
		_ = db.Close()
		return fmt.Errorf("checkpoint database: %w", err)
	}
	if err := db.Close(); err != nil {
		return fmt.Errorf("close database before backup: %w", err)
	}

	absOutput, err := filepath.Abs(output)
	if err != nil {
		return fmt.Errorf("resolve backup output: %w", err)
	}
	if err := os.MkdirAll(filepath.Dir(absOutput), 0o755); err != nil {
		return fmt.Errorf("create backup directory: %w", err)
	}
	tmp, err := os.CreateTemp(filepath.Dir(absOutput), ".agentdocs-backup-*.tmp")
	if err != nil {
		return fmt.Errorf("create backup temp file: %w", err)
	}
	tmpPath := tmp.Name()
	committed := false
	defer func() {
		_ = tmp.Close()
		if !committed {
			_ = os.Remove(tmpPath)
		}
	}()

	gz := gzip.NewWriter(tmp)
	tw := tar.NewWriter(gz)
	manifest, _ := json.Marshal(backupManifest{FormatVersion: backupFormatVersion, CreatedAt: time.Now().UTC()})
	if err := writeTarBytes(tw, "manifest.json", manifest, 0o600); err != nil {
		return err
	}
	if err := addDataTree(ctx, tw, absData); err != nil {
		return err
	}
	if err := tw.Close(); err != nil {
		return fmt.Errorf("finish backup archive: %w", err)
	}
	if err := gz.Close(); err != nil {
		return fmt.Errorf("finish backup compression: %w", err)
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("sync backup: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close backup: %w", err)
	}
	if err := os.Rename(tmpPath, absOutput); err != nil {
		return fmt.Errorf("publish backup: %w", err)
	}
	committed = true
	return nil
}

// Restore validates an offline backup and atomically replaces dataDir.
func Restore(ctx context.Context, archive, dataDir string, replace bool) error {
	lock, err := AcquireDataLock(dataDir)
	if err != nil {
		return err
	}
	defer lock.Close()
	absData, err := filepath.Abs(dataDir)
	if err != nil {
		return fmt.Errorf("resolve data directory: %w", err)
	}
	parent := filepath.Dir(absData)
	staging, err := os.MkdirTemp(parent, ".agentdocs-restore-*")
	if err != nil {
		return fmt.Errorf("create restore staging: %w", err)
	}
	defer os.RemoveAll(staging)
	if err := extractBackup(ctx, archive, staging); err != nil {
		return err
	}
	if err := validateRestoredData(ctx, staging); err != nil {
		return err
	}
	if _, err := os.Stat(absData); err == nil && !replace {
		return errors.New("data directory already exists; pass --replace")
	} else if err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("inspect target data directory: %w", err)
	}

	var previous string
	if _, err := os.Stat(absData); err == nil {
		previous = absData + ".pre-restore-" + time.Now().UTC().Format("20060102T150405Z")
		if err := os.Rename(absData, previous); err != nil {
			return fmt.Errorf("preserve current data directory: %w", err)
		}
	}
	if err := os.Rename(staging, absData); err != nil {
		if previous != "" {
			_ = os.Rename(previous, absData)
		}
		return fmt.Errorf("activate restored data directory: %w", err)
	}
	return nil
}

func addDataTree(ctx context.Context, tw *tar.Writer, root string) error {
	return filepath.Walk(root, func(path string, info os.FileInfo, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		rel, err := filepath.Rel(root, path)
		if err != nil || rel == "." {
			return err
		}
		if rel == "agentdocs.db-wal" || rel == "agentdocs.db-shm" {
			return nil
		}
		name := filepath.ToSlash(filepath.Join("data", rel))
		header, err := tar.FileInfoHeader(info, "")
		if err != nil {
			return err
		}
		header.Name = name
		if err := tw.WriteHeader(header); err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return nil
		}
		f, err := os.Open(path)
		if err != nil {
			return err
		}
		_, copyErr := io.Copy(tw, f)
		closeErr := f.Close()
		return errors.Join(copyErr, closeErr)
	})
}

func writeTarBytes(tw *tar.Writer, name string, data []byte, mode int64) error {
	if err := tw.WriteHeader(&tar.Header{Name: name, Mode: mode, Size: int64(len(data)), ModTime: time.Now().UTC()}); err != nil {
		return err
	}
	_, err := tw.Write(data)
	return err
}

func extractBackup(ctx context.Context, archive, staging string) error {
	f, err := os.Open(archive)
	if err != nil {
		return fmt.Errorf("open backup: %w", err)
	}
	defer f.Close()
	gz, err := gzip.NewReader(f)
	if err != nil {
		return fmt.Errorf("read backup compression: %w", err)
	}
	defer gz.Close()
	tr := tar.NewReader(gz)
	foundManifest := false
	for {
		header, err := tr.Next()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return fmt.Errorf("read backup archive: %w", err)
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		name := filepath.Clean(filepath.FromSlash(header.Name))
		if name == "manifest.json" {
			var manifest backupManifest
			if err := json.NewDecoder(io.LimitReader(tr, 1<<20)).Decode(&manifest); err != nil || manifest.FormatVersion != backupFormatVersion {
				return errors.New("invalid or unsupported backup manifest")
			}
			foundManifest = true
			continue
		}
		if !strings.HasPrefix(filepath.ToSlash(name), "data/") {
			return fmt.Errorf("invalid backup path %q", header.Name)
		}
		rel := strings.TrimPrefix(filepath.ToSlash(name), "data/")
		target := filepath.Join(staging, filepath.FromSlash(rel))
		if !pathWithin(staging, target) {
			return fmt.Errorf("backup path escapes target: %q", header.Name)
		}
		switch header.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, os.FileMode(header.Mode)&0o777); err != nil {
				return err
			}
		case tar.TypeReg, tar.TypeRegA:
			if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
				return err
			}
			out, err := os.OpenFile(target, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, os.FileMode(header.Mode)&0o777)
			if err != nil {
				return err
			}
			_, copyErr := io.CopyN(out, tr, header.Size)
			closeErr := out.Close()
			if err := errors.Join(copyErr, closeErr); err != nil {
				return err
			}
		default:
			return fmt.Errorf("unsupported archive entry %q", header.Name)
		}
	}
	if !foundManifest {
		return errors.New("backup manifest is missing")
	}
	return nil
}

func validateRestoredData(ctx context.Context, staging string) error {
	if _, err := os.Stat(filepath.Join(staging, "agentdocs.db")); err != nil {
		return fmt.Errorf("backup database is missing: %w", err)
	}
	db, err := storesqlite.Open(staging)
	if err != nil {
		return fmt.Errorf("open restored database: %w", err)
	}
	var integrity string
	queryErr := db.QueryRowContext(ctx, "PRAGMA integrity_check").Scan(&integrity)
	closeErr := db.Close()
	if err := errors.Join(queryErr, closeErr); err != nil {
		return fmt.Errorf("validate restored database: %w", err)
	}
	if integrity != "ok" {
		return fmt.Errorf("restored database integrity check: %s", integrity)
	}
	repos, err := filepath.Glob(filepath.Join(staging, "repos", "*", "repo.git"))
	if err != nil {
		return err
	}
	for _, repo := range repos {
		cmd := exec.CommandContext(ctx, "git", "--git-dir", repo, "fsck", "--no-dangling")
		if out, err := cmd.CombinedOutput(); err != nil {
			return fmt.Errorf("validate repository %s: %w: %s", filepath.Base(filepath.Dir(repo)), err, strings.TrimSpace(string(out)))
		}
	}
	return nil
}

func pathWithin(root, target string) bool {
	rel, err := filepath.Rel(root, target)
	return err == nil && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)) && !filepath.IsAbs(rel)
}
