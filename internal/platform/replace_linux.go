//go:build linux

package platform

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
)

func CleanupExitCode() (int, bool) { return 0, false }

// ReplaceExecutable atomically replaces executable with replacement. The copy is
// staged beside executable so the final rename cannot cross file systems.
func ReplaceExecutable(executable, replacement string) (Replacement, error) {
	target, err := filepath.EvalSymlinks(executable)
	if err != nil {
		return Replacement{}, fmt.Errorf("platform: resolve-executable: %w", err)
	}
	info, err := os.Stat(target)
	if err != nil {
		return Replacement{}, fmt.Errorf("platform: stat-executable: %w", err)
	}
	staged, err := copyBeside(target, replacement, info.Mode())
	if err != nil {
		return Replacement{}, err
	}
	if err := os.Rename(staged, target); err != nil {
		_ = os.Remove(staged)
		return Replacement{}, fmt.Errorf("platform: replace-executable: %w", err)
	}
	_ = os.Remove(replacement)
	return Replacement{ExecutablePath: executable}, nil
}

func copyBeside(target, source string, mode os.FileMode) (staged string, resultErr error) {
	input, err := os.Open(source)
	if err != nil {
		return "", fmt.Errorf("platform: open-replacement: %w", err)
	}
	defer input.Close()

	output, err := os.CreateTemp(filepath.Dir(target), ".dw.__temp__.*")
	if err != nil {
		return "", fmt.Errorf("platform: create-staged-executable: %w", err)
	}
	staged = output.Name()
	defer func() {
		if resultErr != nil {
			_ = output.Close()
			_ = os.Remove(staged)
		}
	}()
	if _, err := io.Copy(output, input); err != nil {
		return "", fmt.Errorf("platform: copy-replacement: %w", err)
	}
	if err := output.Chmod(mode); err != nil {
		return "", fmt.Errorf("platform: chmod-replacement: %w", err)
	}
	if err := output.Sync(); err != nil {
		return "", fmt.Errorf("platform: sync-replacement: %w", err)
	}
	if err := output.Close(); err != nil {
		return "", fmt.Errorf("platform: close-replacement: %w", err)
	}
	return staged, nil
}
