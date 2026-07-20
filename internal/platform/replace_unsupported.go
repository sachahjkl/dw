//go:build !linux && !windows

package platform

import "fmt"

func CleanupExitCode() (int, bool) { return 0, false }

func ReplaceExecutable(executable, replacement string) (Replacement, error) {
	return Replacement{}, fmt.Errorf("platform: executable-replacement-unsupported")
}
