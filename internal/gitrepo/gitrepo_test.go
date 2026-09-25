package gitrepo

import (
	"context"
	"errors"
	"os"
	"os/exec"
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

func TestPushRepositorySetsUpstream(t *testing.T) {
	remote := filepath.Join(t.TempDir(), "remote.git")
	repository := filepath.Join(t.TempDir(), "repository")
	runGitTestCommand(t, "", "init", "--bare", remote)
	runGitTestCommand(t, "", "init", "--initial-branch=feat/task", repository)
	runGitTestCommand(t, repository, "config", "user.name", "DevWorkflow Test")
	runGitTestCommand(t, repository, "config", "user.email", "test@example.invalid")
	if err := os.WriteFile(filepath.Join(repository, "file.txt"), []byte("content"), 0o644); err != nil {
		t.Fatal(err)
	}
	runGitTestCommand(t, repository, "add", "file.txt")
	runGitTestCommand(t, repository, "commit", "-m", "test")
	runGitTestCommand(t, repository, "remote", "add", "origin", remote)
	if err := NewClient().PushRepository(context.Background(), RepositoryPath(repository), BranchName("feat/task"), false); err != nil {
		t.Fatal(err)
	}
	upstream := runGitTestCommand(t, repository, "rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}")
	if strings.TrimSpace(upstream) != "origin/feat/task" {
		t.Fatalf("upstream = %q", upstream)
	}
}

func TestConfigureRemotesExcludesSSHFallbackFromFetchAll(t *testing.T) {
	repository := filepath.Join(t.TempDir(), "repository")
	runGitTestCommand(t, "", "init", repository)
	origin := RemoteURL("https://example.invalid/repository.git")
	ssh := RemoteURL("ssh://git@example.invalid/repository.git")
	if err := NewClient().ConfigureRemotes(context.Background(), RepositoryPath(repository), origin, &ssh); err != nil {
		t.Fatal(err)
	}
	value := runGitTestCommand(t, repository, "config", "--get", "remote."+fallbackSSHRemote+".skipFetchAll")
	if strings.TrimSpace(value) != "true" {
		t.Fatalf("remote.%s.skipFetchAll = %q, want true", fallbackSSHRemote, value)
	}
}

func runGitTestCommand(t *testing.T, directory string, arguments ...string) string {
	t.Helper()
	command := exec.Command("git", arguments...)
	command.Dir = directory
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v\n%s", arguments, err, output)
	}
	return string(output)
}

func TestConfigCountAppendsAfterExistingEntries(t *testing.T) {
	t.Setenv("GIT_CONFIG_COUNT", "2")
	if got := configCount(nil); got != 2 {
		t.Fatalf("inherited count = %d, want 2", got)
	}
	environment := []dwprocess.EnvironmentVariable{{Name: "GIT_CONFIG_COUNT", Value: "3"}}
	if got := configCount(environment); got != 3 {
		t.Fatalf("client count = %d, want 3", got)
	}
}

func TestOperationErrorRedactsDetailAndCause(t *testing.T) {
	cause := &dwprocess.ExitError{FileName: "git", Code: 128, Stderr: "fatal: https://user:pat@example.invalid/repo.git"}
	err := Client{}.operationError(OperationFetch, "", dwprocess.Result{Stderr: []byte(cause.Stderr)}, cause, false, nil, "")
	var problem *Error
	if !errors.As(err, &problem) || strings.Contains(problem.Detail, "pat") {
		t.Fatalf("detail = %v", err)
	}
	var exitError *dwprocess.ExitError
	if !errors.As(err, &exitError) || strings.Contains(exitError.Stderr, "pat") || strings.Contains(exitError.Error(), "pat") || !strings.Contains(exitError.Stderr, "://***@") {
		t.Fatalf("cause stderr = %q", exitError.Stderr)
	}
	if cause.Stderr == exitError.Stderr {
		t.Fatal("cause was not replaced")
	}
}
