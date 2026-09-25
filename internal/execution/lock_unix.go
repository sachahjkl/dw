//go:build !windows

package execution

import (
	"errors"
	"os"
	"runtime"
	"strings"

	"golang.org/x/sys/unix"
)

// normalizePlatformRoot folds case on darwin, whose default file systems are case-insensitive.
func normalizePlatformRoot(root string) string {
	if runtime.GOOS == "darwin" {
		return strings.ToLower(root)
	}
	return root
}

func openPlatformLock(path string) (*os.File, error) {
	return os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o600)
}

func tryPlatformLock(file *os.File, shared bool) (bool, error) {
	operation := unix.LOCK_EX | unix.LOCK_NB
	if shared {
		operation = unix.LOCK_SH | unix.LOCK_NB
	}
	err := unix.Flock(int(file.Fd()), operation)
	if errors.Is(err, unix.EWOULDBLOCK) || errors.Is(err, unix.EAGAIN) {
		return false, nil
	}
	return err == nil, err
}

func unlockPlatformLock(file *os.File) error {
	return unix.Flock(int(file.Fd()), unix.LOCK_UN)
}
