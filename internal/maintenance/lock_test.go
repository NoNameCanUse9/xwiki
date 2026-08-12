package maintenance

import (
	"errors"
	"testing"
)

func TestDataDirectoryLockIsExclusiveAndReusable(t *testing.T) {
	dataDir := t.TempDir()

	first, err := AcquireDataLock(dataDir)
	if err != nil {
		t.Fatalf("first acquire: %v", err)
	}

	if _, err := AcquireDataLock(dataDir); !errors.Is(err, ErrDataLocked) {
		t.Fatalf("second acquire = %v, want ErrDataLocked", err)
	}

	if err := first.Close(); err != nil {
		t.Fatalf("close first lock: %v", err)
	}
	third, err := AcquireDataLock(dataDir)
	if err != nil {
		t.Fatalf("acquire after release: %v", err)
	}
	if err := third.Close(); err != nil {
		t.Fatalf("close third lock: %v", err)
	}
}
