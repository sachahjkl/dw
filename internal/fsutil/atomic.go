package fsutil

import (
	"os"
	"path/filepath"
)

// WriteFileAtomic replaces path with content through a synced temporary file.
// A zero perm keeps the mode of an existing destination. A perm without group
// or other bits also restricts the Windows ACL to the current user.
func WriteFileAtomic(path string, content []byte, perm os.FileMode) error {
	file, err := os.CreateTemp(filepath.Dir(path), ".dw-*")
	if err != nil {
		return err
	}
	name := file.Name()
	keep := false
	defer func() {
		_ = file.Close()
		if !keep {
			_ = os.Remove(name)
		}
	}()
	if perm == 0 {
		if info, statErr := os.Stat(path); statErr == nil {
			perm = info.Mode().Perm()
		}
	} else if perm&0o077 == 0 {
		if err = restrictToOwner(name); err != nil {
			return err
		}
	}
	if perm != 0 {
		if err = file.Chmod(perm); err != nil {
			return err
		}
	}
	if _, err = file.Write(content); err != nil {
		return err
	}
	if err = file.Sync(); err != nil {
		return err
	}
	if err = file.Close(); err != nil {
		return err
	}
	if err = replaceFile(name, path); err != nil {
		return err
	}
	keep = true
	return nil
}

// RestrictToOwner limits an existing file to the current user.
func RestrictToOwner(path string) error { return restrictToOwner(path) }
