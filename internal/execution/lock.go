package execution

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
)

type LockHandle interface {
	Release() error
}

type Locker interface {
	Acquire(context.Context, LockSpec) (LockHandle, error)
}

type RootLocker struct {
	directory     string
	retryInterval time.Duration
}

type fileLockHandle struct {
	file *os.File
	once sync.Once
	err  error
}

type noLockHandle struct{}

func NewRootLocker(directory string) (*RootLocker, error) {
	return NewRootLockerWithRetry(directory, runtimeconfig.Milliseconds(runtimeconfig.Default().Execution.LockRetryMilliseconds))
}

func NewRootLockerWithRetry(directory string, retryInterval time.Duration) (*RootLocker, error) {
	if directory == "" {
		return nil, fmt.Errorf("execution.lock-directory-required")
	}
	if retryInterval <= 0 {
		return nil, fmt.Errorf("execution.lock-retry-required")
	}
	return &RootLocker{directory: directory, retryInterval: retryInterval}, nil
}

func DefaultLockDirectory() string {
	dirs := config.ResolvePlatformBaseDirs()
	base := dirs.StateDir
	if runtime.GOOS == "windows" {
		base = dirs.DataLocalDir
	}
	if base == "" {
		base = dirs.HomeDir
	}
	return filepath.Join(base, "DevWorkflow", "locks")
}

func CanonicalRoot(root string) (string, error) {
	if root == "" {
		return "", fmt.Errorf("execution.root-required")
	}
	absolute, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("execution.canonical-root:%w", err)
	}
	canonical, err := resolveExistingLinks(absolute)
	if err != nil {
		return "", fmt.Errorf("execution.canonical-root:%w", err)
	}
	return normalizePlatformRoot(filepath.Clean(canonical)), nil
}

func (locker *RootLocker) Acquire(ctx context.Context, spec LockSpec) (LockHandle, error) {
	if err := spec.Validate(); err != nil {
		return nil, err
	}
	if spec.Mode == LockNone {
		return noLockHandle{}, nil
	}
	key, err := canonicalLockKey(spec.Key)
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(locker.directory, 0o700); err != nil {
		return nil, fmt.Errorf("execution.lock-unavailable:%w", err)
	}
	digest := sha256.Sum256([]byte(key))
	path := filepath.Join(locker.directory, hex.EncodeToString(digest[:])+".lock")
	file, err := openPlatformLock(path)
	if err != nil {
		return nil, fmt.Errorf("execution.lock-unavailable:%w", err)
	}
	shared := spec.Mode == LockShared
	for {
		acquired, lockErr := tryPlatformLock(file, shared)
		if lockErr != nil {
			_ = file.Close()
			return nil, fmt.Errorf("execution.lock-unavailable:%w", lockErr)
		}
		if acquired {
			select {
			case <-ctx.Done():
				_ = unlockPlatformLock(file)
				_ = file.Close()
				return nil, ctx.Err()
			default:
				return &fileLockHandle{file: file}, nil
			}
		}
		timer := time.NewTimer(locker.retryInterval)
		select {
		case <-ctx.Done():
			if !timer.Stop() {
				<-timer.C
			}
			_ = file.Close()
			return nil, ctx.Err()
		case <-timer.C:
		}
	}
}

func (handle *fileLockHandle) Release() error {
	handle.once.Do(func() {
		if err := unlockPlatformLock(handle.file); err != nil {
			handle.err = fmt.Errorf("execution.lock-unavailable:%w", err)
		}
		if err := handle.file.Close(); err != nil && handle.err == nil {
			handle.err = fmt.Errorf("execution.lock-unavailable:%w", err)
		}
	})
	return handle.err
}

func (noLockHandle) Release() error { return nil }

func canonicalLockKey(key string) (string, error) {
	if strings.HasPrefix(key, "resource:") {
		if len(key) == len("resource:") {
			return "", fmt.Errorf("execution.lock-key-required")
		}
		return key, nil
	}
	return CanonicalRoot(key)
}

func resolveExistingLinks(path string) (string, error) {
	current := path
	tail := make([]string, 0)
	for {
		resolved, err := filepath.EvalSymlinks(current)
		if err == nil {
			for index := len(tail) - 1; index >= 0; index-- {
				resolved = filepath.Join(resolved, tail[index])
			}
			return resolved, nil
		}
		if !os.IsNotExist(err) {
			return "", err
		}
		parent := filepath.Dir(current)
		if parent == current {
			return path, nil
		}
		tail = append(tail, filepath.Base(current))
		current = parent
	}
}
