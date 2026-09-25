package ado

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"time"
)

const (
	refreshTokenLockRetry = 50 * time.Millisecond
	refreshTokenLockStale = 2 * time.Minute
)

func refreshTokenLockPath() string {
	base, err := os.UserCacheDir()
	if err != nil || base == "" {
		base = os.TempDir()
	}
	return filepath.Join(base, "DevWorkflow", "locks", KeyringService+".lock")
}

func lockRefreshToken(ctx context.Context) (func(), error) {
	return acquireFileLock(ctx, refreshTokenLockPath(), time.Now)
}

// acquireFileLock creates path exclusively; a lock file older than
// refreshTokenLockStale is considered abandoned by a crashed process.
func acquireFileLock(ctx context.Context, path string, now func() time.Time) (func(), error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, err
	}
	for {
		file, err := os.OpenFile(path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err == nil {
			_ = file.Close()
			return func() { _ = os.Remove(path) }, nil
		}
		if !errors.Is(err, os.ErrExist) {
			return nil, err
		}
		if info, statErr := os.Stat(path); statErr == nil && now().Sub(info.ModTime()) > refreshTokenLockStale {
			_ = os.Remove(path)
			continue
		}
		timer := time.NewTimer(refreshTokenLockRetry)
		select {
		case <-ctx.Done():
			timer.Stop()
			return nil, ctx.Err()
		case <-timer.C:
		}
	}
}
