//go:build !windows

package dataapp

import "os"

func replaceFileAtomic(source, destination string) error {
	return os.Rename(source, destination)
}
