//go:build !windows

package dbcompat

import "os"

func replaceFileAtomic(source, destination string) error {
	return os.Rename(source, destination)
}
