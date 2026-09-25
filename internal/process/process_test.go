package process

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

func TestMissingWorkingDirectoryIsNotReportedAsMissingExecutable(t *testing.T) {
	missing := filepath.Join(t.TempDir(), "missing")
	_, err := Output(context.Background(), Command{FileName: "git", Arguments: []string{"--version"}, WorkingDirectory: missing})
	var start *StartError
	if !errors.As(err, &start) || !errors.Is(err, os.ErrNotExist) || errors.Is(err, exec.ErrNotFound) {
		t.Fatalf("Output error = %v, want working directory error", err)
	}
	if err := Run(context.Background(), Command{FileName: "git", WorkingDirectory: missing}, nil, nil, nil); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("Run error = %v, want working directory error", err)
	}
}
