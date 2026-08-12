package ops

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"syscall"
	"time"

	_ "modernc.org/sqlite"
)

const backupFormat = 1

// DataLock prevents the server and offline maintenance commands from using
// the same data directory at the same time. The lock is deliberately a small
// PID file so it remains useful even before SQLite is opened.
type DataLock struct {
	path string
	file *os.File
}

func AcquireDataLock(dataDir string) (*DataLock, error) {
	if err := os.MkdirAll(dataDir, 0o750); err != nil {
		return nil, err
	}
	path := filepath.Join(dataDir, ".xwiki.lock")
	for attempt := 0; attempt < 2; attempt++ {
		f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
		if err == nil {
			_, _ = fmt.Fprintf(f, "%d\n", os.Getpid())
			return &DataLock{path: path, file: f}, nil
		}
		if !errors.Is(err, os.ErrExist) || attempt == 1 {
			return nil, fmt.Errorf("data directory is locked: %w", err)
		}
		b, readErr := os.ReadFile(path)
		pid, parseErr := strconv.Atoi(strings.TrimSpace(string(b)))
		if readErr == nil && parseErr == nil && processExists(pid) {
			return nil, fmt.Errorf("data directory is locked by pid %d", pid)
		}
		if err := os.Remove(path); err != nil && !errors.Is(err, os.ErrNotExist) {
			return nil, fmt.Errorf("remove stale data lock: %w", err)
		}
	}
	return nil, errors.New("could not acquire data lock")
}

func (l *DataLock) Close() error {
	if l == nil {
		return nil
	}
	if l.file != nil {
		_ = l.file.Close()
	}
	return os.Remove(l.path)
}

func processExists(pid int) bool {
	if pid <= 0 {
		return false
	}
	err := syscall.Kill(pid, 0)
	return err == nil || errors.Is(err, syscall.EPERM)
}

type manifestEntry struct {
	Path   string `json:"path"`
	Size   int64  `json:"size"`
	SHA256 string `json:"sha256"`
}

type manifest struct {
	Format    int             `json:"format"`
	Service   string          `json:"service"`
	CreatedAt string          `json:"created_at"`
	Files     []manifestEntry `json:"files"`
}

func BackupCreate(dataDir, output, service string) error {
	lock, err := AcquireDataLock(dataDir)
	if err != nil {
		return err
	}
	defer lock.Close()
	if err := checkSQLite(dataDir, true); err != nil {
		return err
	}
	if err := checkRepositories(dataDir, false); err != nil {
		return err
	}
	entries, err := collectFiles(dataDir)
	if err != nil {
		return err
	}
	m := manifest{Format: backupFormat, Service: service, CreatedAt: time.Now().UTC().Format(time.RFC3339), Files: entries}
	if err := os.MkdirAll(filepath.Dir(output), 0o750); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(output), ".xwiki-backup-*")
	if err != nil {
		return err
	}
	tmpPath := tmp.Name()
	defer os.Remove(tmpPath)
	if err := tmp.Chmod(0o600); err != nil {
		_ = tmp.Close()
		return err
	}
	gz := gzip.NewWriter(tmp)
	tw := tar.NewWriter(gz)
	for _, e := range entries {
		if err := addTarFile(tw, filepath.Join(dataDir, filepath.FromSlash(e.Path)), e.Path, e.Size); err != nil {
			_ = tw.Close()
			_ = gz.Close()
			_ = tmp.Close()
			return err
		}
	}
	b, _ := json.MarshalIndent(m, "", "  ")
	if err := writeTarBytes(tw, "MANIFEST.json", b, 0o600); err != nil {
		_ = tw.Close()
		_ = gz.Close()
		_ = tmp.Close()
		return err
	}
	if err := tw.Close(); err != nil {
		_ = gz.Close()
		_ = tmp.Close()
		return err
	}
	if err := gz.Close(); err != nil {
		_ = tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	if err := os.Rename(tmpPath, output); err != nil {
		return err
	}
	return os.Chmod(output, 0o600)
}

func BackupRestore(input, dataDir string) error {
	var lock *DataLock
	if _, err := os.Stat(dataDir); err == nil {
		lock, err = AcquireDataLock(dataDir)
		if err != nil {
			return err
		}
		defer lock.Close()
	}
	if _, err := os.Stat(dataDir); err == nil {
		ents, readErr := os.ReadDir(dataDir)
		if readErr != nil {
			return readErr
		}
		for i := range ents {
			if ents[i].Name() == ".xwiki.lock" {
				ents = append(ents[:i], ents[i+1:]...)
				break
			}
		}
		if len(ents) != 0 {
			return fmt.Errorf("restore target must be empty: %s", dataDir)
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	parent := filepath.Dir(filepath.Clean(dataDir))
	if err := os.MkdirAll(parent, 0o750); err != nil {
		return err
	}
	tmp, err := os.MkdirTemp(parent, ".xwiki-restore-*")
	if err != nil {
		return err
	}
	defer os.RemoveAll(tmp)
	if err := extractBackup(input, tmp); err != nil {
		return err
	}
	if err := validateRestored(tmp); err != nil {
		return err
	}
	if _, err := os.Stat(dataDir); err == nil {
		if lock != nil {
			_ = lock.Close()
			lock = nil
		}
		if err := os.Remove(dataDir); err != nil {
			return fmt.Errorf("remove empty restore target: %w", err)
		}
	}
	return os.Rename(tmp, dataDir)
}

func Doctor(dataDir string) error {
	var problems []string
	if _, err := exec.LookPath("git"); err != nil {
		problems = append(problems, "git is not available")
	} else if out, err := exec.Command("git", "--version").CombinedOutput(); err != nil {
		problems = append(problems, fmt.Sprintf("git check: %v", err))
	} else {
		fmt.Printf("git: %s", out)
	}
	if st, err := os.Stat(dataDir); err != nil {
		problems = append(problems, fmt.Sprintf("data dir: %v", err))
	} else if !st.IsDir() {
		problems = append(problems, "data dir is not a directory")
	} else if err := checkSQLite(dataDir, false); err != nil {
		problems = append(problems, err.Error())
	}
	if err := checkRepositories(dataDir, true); err != nil {
		problems = append(problems, err.Error())
	}
	if len(problems) > 0 {
		for _, p := range problems {
			fmt.Printf("FAIL: %s\n", p)
		}
		return errors.New("doctor found problems")
	}
	fmt.Println("OK: data directory is healthy")
	return nil
}

func checkSQLite(dataDir string, checkpoint bool) error {
	path := filepath.Join(dataDir, "xwiki.db")
	if _, err := os.Stat(path); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return fmt.Errorf("sqlite: %w", err)
	}
	db, err := sql.Open("sqlite", path)
	if err != nil {
		return fmt.Errorf("sqlite open: %w", err)
	}
	defer db.Close()
	if checkpoint {
		if _, err := db.Exec("PRAGMA wal_checkpoint(TRUNCATE)"); err != nil {
			return fmt.Errorf("sqlite checkpoint: %w", err)
		}
	}
	var result string
	if err := db.QueryRow("PRAGMA integrity_check").Scan(&result); err != nil || result != "ok" {
		return fmt.Errorf("sqlite integrity check: %s (%v)", result, err)
	}
	return nil
}

func collectFiles(dataDir string) ([]manifestEntry, error) {
	var out []manifestEntry
	err := filepath.Walk(dataDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if path == dataDir {
			return nil
		}
		if info.IsDir() {
			return nil
		}
		rel, err := filepath.Rel(dataDir, path)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		if rel == ".xwiki.lock" || strings.HasSuffix(rel, "-wal") || strings.HasSuffix(rel, "-shm") {
			return nil
		}
		h, err := fileSHA(path)
		if err != nil {
			return err
		}
		out = append(out, manifestEntry{Path: rel, Size: info.Size(), SHA256: h})
		return nil
	})
	sort.Slice(out, func(i, j int) bool { return out[i].Path < out[j].Path })
	return out, err
}

func fileSHA(path string) (string, error) {
	f, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", err
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}

func addTarFile(tw *tar.Writer, path, name string, size int64) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()
	if err := tw.WriteHeader(&tar.Header{Name: name, Mode: 0o600, Size: size, ModTime: time.Unix(0, 0)}); err != nil {
		return err
	}
	_, err = io.Copy(tw, f)
	return err
}

func writeTarBytes(tw *tar.Writer, name string, data []byte, mode int64) error {
	if err := tw.WriteHeader(&tar.Header{Name: name, Mode: mode, Size: int64(len(data)), ModTime: time.Unix(0, 0)}); err != nil {
		return err
	}
	_, err := tw.Write(data)
	return err
}

func extractBackup(input, target string) error {
	f, err := os.Open(input)
	if err != nil {
		return err
	}
	defer f.Close()
	gz, err := gzip.NewReader(f)
	if err != nil {
		return fmt.Errorf("open backup: %w", err)
	}
	defer gz.Close()
	tr := tar.NewReader(gz)
	for {
		h, err := tr.Next()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return err
		}
		name := filepath.ToSlash(filepath.Clean(h.Name))
		if name == "." || name == ".." || strings.HasPrefix(name, "../") || strings.HasPrefix(name, "/") || filepath.IsAbs(h.Name) {
			return fmt.Errorf("unsafe backup path: %q", h.Name)
		}
		if h.Typeflag == tar.TypeDir {
			if err := os.MkdirAll(filepath.Join(target, filepath.FromSlash(name)), 0o750); err != nil {
				return err
			}
			continue
		}
		if h.Typeflag != tar.TypeReg {
			return fmt.Errorf("unsupported backup entry: %q", h.Name)
		}
		path := filepath.Join(target, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o750); err != nil {
			return err
		}
		out, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
		if err != nil {
			return err
		}
		_, copyErr := io.CopyN(out, tr, h.Size)
		closeErr := out.Close()
		if copyErr != nil {
			return copyErr
		}
		if closeErr != nil {
			return closeErr
		}
	}
	return nil
}

func validateRestored(dir string) error {
	b, err := os.ReadFile(filepath.Join(dir, "MANIFEST.json"))
	if err != nil {
		return fmt.Errorf("backup manifest missing: %w", err)
	}
	var m manifest
	if err := json.Unmarshal(b, &m); err != nil || m.Format != backupFormat {
		return errors.New("unsupported backup manifest")
	}
	for _, e := range m.Files {
		path := filepath.Join(dir, filepath.FromSlash(e.Path))
		st, err := os.Stat(path)
		if err != nil || st.Size() != e.Size {
			return fmt.Errorf("backup file mismatch: %s", e.Path)
		}
		h, err := fileSHA(path)
		if err != nil || h != e.SHA256 {
			return fmt.Errorf("backup checksum mismatch: %s", e.Path)
		}
	}
	if err := checkSQLite(dir, false); err != nil {
		return err
	}
	if err := checkRepositories(dir, true); err != nil {
		return err
	}
	return nil
}

func checkRepositories(dataDir string, strict bool) error {
	reposRoot := filepath.Join(dataDir, "repos")
	ents, err := os.ReadDir(reposRoot)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	for _, ent := range ents {
		if !ent.IsDir() {
			continue
		}
		repo := filepath.Join(reposRoot, ent.Name(), "repo.git")
		if _, err := os.Stat(repo); err != nil {
			if strict {
				return fmt.Errorf("missing repository for %s: %w", ent.Name(), err)
			}
			continue
		}
		cmd := exec.Command("git", "--git-dir", repo, "fsck", "--full")
		if out, err := cmd.CombinedOutput(); err != nil {
			return fmt.Errorf("git fsck %s: %v: %s", ent.Name(), err, strings.TrimSpace(string(out)))
		}
	}
	return nil
}
