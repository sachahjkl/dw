package workspace

import (
	"context"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/gitrepo"
)

type finishTestConfig struct{ project ProjectConfig }

func (config finishTestConfig) Project(context.Context, string, string) (ProjectConfig, bool, error) {
	return config.project, true, nil
}
func (finishTestConfig) Workflow(context.Context, string) (WorkflowConfig, error) {
	return WorkflowConfig{}, nil
}

type finishTestGit struct {
	statuses map[string]RepositoryStatus
	commits  []string
	pushes   []string
}

func (*finishTestGit) PrepareWorktree(context.Context, WorktreeRequest) (WorktreeResult, error) {
	return WorktreeResult{}, nil
}
func (git *finishTestGit) Status(_ context.Context, path string) (RepositoryStatus, error) {
	return git.statuses[filepath.Base(path)], nil
}
func (*finishTestGit) Update(context.Context, string, string, *gitrepo.Credential, *string) error {
	return nil
}
func (git *finishTestGit) Commit(_ context.Context, path, _ string) error {
	git.commits = append(git.commits, filepath.Base(path))
	return nil
}
func (git *finishTestGit) Push(_ context.Context, path, _ string, _ bool) error {
	git.pushes = append(git.pushes, filepath.Base(path))
	return nil
}
func (*finishTestGit) HasCommitsAhead(context.Context, string, string) (bool, error) {
	return false, nil
}
func (*finishTestGit) WorktreeRemove(context.Context, string, string) error { return nil }
func (*finishTestGit) WorktreePrune(context.Context, string) error          { return nil }

func TestFinishIncludesChangedAndUnpushedRepositories(t *testing.T) {
	workspace := t.TempDir()
	manifest := Manifest{WorkItemID: "42", Project: "project", Type: "feat", Slug: "finish", BranchName: "feat/finish", Repositories: []string{"changed", "unpushed", "both"}}
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		t.Fatal(err)
	}
	for _, repository := range manifest.Repositories {
		handoff := "```yaml\nstatus: done\nrepository: " + repository + "\n```\n"
		if err := writeFileAtomic(filepath.Join(workspace, HandoffPrefix+repository+".md"), []byte(handoff), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	git := &finishTestGit{statuses: map[string]RepositoryStatus{
		"changed":  {IsGitRepository: true, HasChanges: true},
		"unpushed": {IsGitRepository: true, HasUnpushed: true},
		"both":     {IsGitRepository: true, HasChanges: true, HasUnpushed: true},
	}}
	config := finishTestConfig{project: ProjectConfig{Key: "project", Repositories: []RepositoryConfig{
		{Name: "changed", ProviderRepository: "org/changed"},
		{Name: "unpushed", ProviderRepository: "org/unpushed"},
		{Name: "both", ProviderRepository: "org/both"},
	}}}
	engine := NewEngine(config, git, nil, nil)
	plan, err := engine.PlanFinish(context.Background(), t.TempDir(), workspace, "", true, false)
	if err != nil {
		t.Fatal(err)
	}
	wantRepositories := []string{"changed", "unpushed", "both"}
	if !reflect.DeepEqual(plan.ActionableRepositories, wantRepositories) {
		t.Fatalf("actionable repositories = %#v, want %#v", plan.ActionableRepositories, wantRepositories)
	}
	gotCandidates := make([]string, 0, len(plan.PullRequestCandidates))
	for _, candidate := range plan.PullRequestCandidates {
		gotCandidates = append(gotCandidates, candidate.Repository)
	}
	if !reflect.DeepEqual(gotCandidates, wantRepositories) {
		t.Fatalf("PR candidates = %#v, want %#v", gotCandidates, wantRepositories)
	}

	plan.CreatePR = false
	if _, err := engine.ExecuteFinish(context.Background(), plan, FinishExecuteOptions{SkipVerification: true}, nil); err != nil {
		t.Fatal(err)
	}
	if want := []string{"changed", "both"}; !reflect.DeepEqual(git.commits, want) {
		t.Fatalf("commits = %#v, want %#v", git.commits, want)
	}
	if want := []string{"changed", "both", "unpushed"}; !reflect.DeepEqual(git.pushes, want) {
		t.Fatalf("pushes = %#v, want %#v", git.pushes, want)
	}
}
