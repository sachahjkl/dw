package gitrepo

import (
	"context"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	dwprocess "github.com/sachahjkl/dw/internal/process"
)

func TestUpdateRepositoryRestoresAutostashAndJoinsErrors(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("test shim requires a POSIX shell")
	}
	directory := t.TempDir()
	logPath := filepath.Join(directory, "commands.log")
	shimPath := filepath.Join(directory, "git-shim")
	shim := `#!/bin/sh
printf '%s\n' "$*" >> "$DW_GIT_TEST_LOG"
case " $* " in
  *" status --porcelain=v1 "*) printf ' M changed.txt\000'; exit 0 ;;
  *" remote get-url origin "*) printf 'https://example.invalid/repository.git\n'; exit 0 ;;
  *" fetch --prune origin "*) printf 'fetch failed\n' >&2; exit 1 ;;
  *" stash pop "*) printf 'stash pop failed\n' >&2; exit 1 ;;
  *) exit 0 ;;
esac
`
	if err := os.WriteFile(shimPath, []byte(shim), 0o755); err != nil {
		t.Fatal(err)
	}
	client := Client{Executable: shimPath, Environment: []dwprocess.EnvironmentVariable{{Name: "DW_GIT_TEST_LOG", Value: logPath}}}
	err := client.UpdateRepository(context.Background(), RepositoryPath(directory), BranchName("main"), nil, nil)
	if err == nil {
		t.Fatal("UpdateRepository succeeded, want fetch and stash restoration errors")
	}
	for _, detail := range []string{"fetch failed", "stash pop failed"} {
		if !strings.Contains(err.Error(), detail) {
			t.Fatalf("error %q does not contain %q", err, detail)
		}
	}
	commands, readErr := os.ReadFile(logPath)
	if readErr != nil {
		t.Fatal(readErr)
	}
	if !strings.Contains(string(commands), "stash pop") {
		t.Fatalf("stash restoration was not attempted after fetch failure:\n%s", commands)
	}
}
