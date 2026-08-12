package maintenance

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// ErrDataLocked reports that another AgentDocs process owns the data directory.
var ErrDataLocked = errors.New("agentdocs data directory is locked")

// DataLock is an advisory, cross-process lock for one data directory.
// The lock file lives beside the data directory so restore can replace the
// directory without replacing the lock itself.
type DataLock struct {
	file *os.File
}

// AcquireDataLock obtains exclusive ownership of dataDir until Close.
func AcquireDataLock(dataDir string) (*DataLock, error) {
	abs, err := filepath.Abs(dataDir)
	if err != nil {
		return nil, fmt.Errorf("resolve data directory: %w", err)
	}
	parent := filepath.Dir(abs)
	if err := os.MkdirAll(parent, 0o755); err != nil {
		return nil, fmt.Errorf("create lock parent: %w", err)
	}
	lockPath := filepath.Join(parent, "."+filepath.Base(abs)+".lock")
	f, err := os.OpenFile(lockPath, os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open data lock: %w", err)
	}
	if err := lockFile(f); err != nil {
		_ = f.Close()
		if isLockContended(err) {
			return nil, ErrDataLocked
		}
		return nil, fmt.Errorf("lock data directory: %w", err)
	}
	return &DataLock{file: f}, nil
}

// Close releases the data directory lock. It is safe to call more than once.
func (l *DataLock) Close() error {
	if l == nil || l.file == nil {
		return nil
	}
	f := l.file
	l.file = nil
	unlockErr := unlockFile(f)
	closeErr := f.Close()
	if unlockErr != nil {
		return fmt.Errorf("unlock data directory: %w", unlockErr)
	}
	return closeErr
}
