//go:build !windows

package fsutil

import "os"

func replaceFile(source, destination string) error { return os.Rename(source, destination) }

func restrictToOwner(string) error { return nil }
