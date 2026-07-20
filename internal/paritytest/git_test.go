package paritytest_test

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/gitrepo"
	dwprocess "github.com/sachahjkl/dw/internal/process"
)

func TestLocalGitWorktreeCommitPushLifecycle(t *testing.T) {
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("requires native Git executable on PATH")
	}
	base := t.TempDir()
	home := filepath.Join(base, "home")
	if err := os.Mkdir(home, 0o700); err != nil {
		t.Fatal(err)
	}
	environment := []dwprocess.EnvironmentVariable{
		{Name: "HOME", Value: home},
		{Name: "GIT_CONFIG_NOSYSTEM", Value: "1"},
		{Name: "GIT_TERMINAL_PROMPT", Value: "0"},
		{Name: "GIT_AUTHOR_NAME", Value: "dw parity"},
		{Name: "GIT_AUTHOR_EMAIL", Value: "dw-parity@example.invalid"},
		{Name: "GIT_COMMITTER_NAME", Value: "dw parity"},
		{Name: "GIT_COMMITTER_EMAIL", Value: "dw-parity@example.invalid"},
	}
	runGit := func(directory string, arguments ...string) string {
		t.Helper()
		result, err := dwprocess.Output(context.Background(), dwprocess.Command{
			FileName: "git", Arguments: arguments, WorkingDirectory: directory, Environment: environment,
		})
		if err != nil {
			t.Fatalf("git %s: %v\nstderr: %s", strings.Join(arguments, " "), err, result.Stderr)
		}
		return strings.TrimSpace(string(result.Stdout))
	}

	remote := filepath.Join(base, "remote.git")
	runGit(base, "init", "--bare", "--initial-branch=main", remote)
	seed := filepath.Join(base, "seed")
	runGit(base, "clone", remote, seed)
	if err := os.WriteFile(filepath.Join(seed, "README.txt"), []byte("initial\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	runGit(seed, "add", "README.txt")
	runGit(seed, "commit", "-m", "initial")
	runGit(seed, "push", "-u", "origin", "main")

	client := gitrepo.NewClient()
	client.Environment = environment
	projectRoot := filepath.Join(base, "project")
	worktree := filepath.Join(base, "workspace", "repo")
	request := gitrepo.WorktreePrepareRequest{
		ProjectRoot:   gitrepo.ProjectRootPath(projectRoot),
		Repository:    gitrepo.WorkspaceRepositoryName("repo"),
		HTTPURL:       gitrepo.RemoteURL(remote),
		DefaultBranch: gitrepo.BranchName("main"),
		AnchorName:    gitrepo.AnchorName("repo.git"),
		BranchName:    gitrepo.BranchName("feat/42-parity"),
		WorktreePath:  gitrepo.RepositoryPath(worktree),
	}
	prepared, err := client.PrepareWorktree(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	if prepared.Status != gitrepo.WorktreePrepared || prepared.Detail.Kind != gitrepo.WorktreeCreatedFromBaseReference {
		t.Fatalf("prepare result = %#v", prepared)
	}
	status := client.RepositoryStatus(context.Background(), gitrepo.RepositoryPath(worktree))
	if !status.IsGitRepository || status.HasChanges || status.Detail.Kind != gitrepo.StatusClean {
		t.Fatalf("initial status = %#v", status)
	}

	if err := os.WriteFile(filepath.Join(worktree, "change.txt"), []byte("local change\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	status = client.RepositoryStatus(context.Background(), gitrepo.RepositoryPath(worktree))
	if !status.HasChanges || status.Detail.Kind != gitrepo.StatusChanged || len(status.Detail.Paths) != 1 || status.Detail.Paths[0] != "change.txt" {
		t.Fatalf("changed status = %#v", status)
	}
	if err := client.CommitRepository(context.Background(), gitrepo.RepositoryPath(worktree), gitrepo.CommitMessage("parity commit")); err != nil {
		t.Fatal(err)
	}
	ahead, err := client.HasCommitsAheadOf(context.Background(), gitrepo.RepositoryPath(worktree), gitrepo.Revision("origin/main"))
	if err != nil {
		t.Fatal(err)
	}
	if !ahead {
		t.Fatal("new commit is not ahead of origin/main")
	}
	messages, err := client.CommitMessagesInRangeAt(context.Background(), gitrepo.RepositoryPath(worktree), gitrepo.RevisionRange{From: "origin/main", To: "HEAD"})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(messages.String(), "parity commit") {
		t.Fatalf("commit messages = %q", messages)
	}
	if err := client.PushRepository(context.Background(), gitrepo.RepositoryPath(worktree), gitrepo.BranchName("feat/42-parity"), false); err != nil {
		t.Fatal(err)
	}
	if got := runGit(remote, "rev-parse", "refs/heads/feat/42-parity"); len(got) != 40 {
		t.Fatalf("remote branch object id = %q", got)
	}

	anchor := filepath.Join(projectRoot, "repositories", "repo.git")
	if err := client.WorktreeRemove(context.Background(), gitrepo.RepositoryPath(anchor), gitrepo.RepositoryPath(worktree)); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(worktree); !os.IsNotExist(err) {
		t.Fatalf("worktree still exists after removal: %v", err)
	}
	if err := client.WorktreePrune(context.Background(), gitrepo.RepositoryPath(anchor)); err != nil {
		t.Fatal(err)
	}
}
