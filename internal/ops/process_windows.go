//go:build windows

package ops

import (
	"errors"
	"os"
	"syscall"
)

// processExists reports whether a process with the given pid is running.
// Windows has no kill(pid, 0); FindProcess + Signal(0) is the closest
// portable probe (Signal(0) is unsupported, which still proves the pid
// belongs to a live process handle).
func processExists(pid int) bool {
	if pid <= 0 {
		return false
	}
	p, err := os.FindProcess(pid)
	if err != nil {
		return false
	}
	err = p.Signal(syscall.Signal(0))
	return err == nil || errors.Is(err, syscall.EPERM) || errors.Is(err, syscall.EINVAL)
}
