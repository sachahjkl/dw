package workspace

import (
	"context"
	"errors"
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
	pushErr  error
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
	return git.pushErr
}

type finishTestWork struct{ created int }

func (*finishTestWork) GetWorkItems(context.Context, string, []string) ([]WorkItem, error) {
	return nil, nil
}
func (*finishTestWork) UpdateWorkItemState(context.Context, string, string, string) error {
	return nil
}
func (*finishTestWork) CreateChildTask(context.Context, string, WorkItem, string, string) (ChildTask, error) {
	return ChildTask{}, nil
}
func (*finishTestWork) FindActivePullRequest(context.Context, string, string, string) (*WorkPullRequest, error) {
	return nil, nil
}
func (work *finishTestWork) CreatePullRequest(context.Context, string, PullRequestInput) (WorkPullRequest, error) {
	work.created++
	return WorkPullRequest{ID: 1}, nil
}
func (*finishTestWork) LinkWorkItemToPullRequest(context.Context, string, string, int64, string) error {
	return nil
}

func TestFinishPushesCleanPullRequestCandidateBeforeCreation(t *testing.T) {
	git := &finishTestGit{}
	work := &finishTestWork{}
	engine := NewEngine(finishTestConfig{}, git, nil, work)
	plan := finishPullRequestPlan(t)

	report, err := engine.ExecuteFinish(context.Background(), plan, FinishExecuteOptions{SkipVerification: true}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(git.pushes, []string{"repo"}) || work.created != 1 {
		t.Fatalf("pushes = %v, created PRs = %d", git.pushes, work.created)
	}
	if len(report.GitActions) != 1 || report.GitActions[0].Operation != "push" {
		t.Fatalf("git actions = %#v", report.GitActions)
	}
}

func TestFinishDoesNotCreatePullRequestWhenCandidatePushFails(t *testing.T) {
	git := &finishTestGit{pushErr: errors.New("push failed")}
	work := &finishTestWork{}
	engine := NewEngine(finishTestConfig{}, git, nil, work)

	_, err := engine.ExecuteFinish(context.Background(), finishPullRequestPlan(t), FinishExecuteOptions{SkipVerification: true}, nil)
	if err == nil || work.created != 0 {
		t.Fatalf("error = %v, created PRs = %d", err, work.created)
	}
}

func finishPullRequestPlan(t *testing.T) FinishPlanReport {
	t.Helper()
	workspace := t.TempDir()
	if err := writeFileAtomic(filepath.Join(workspace, HandoffPrefix+"repo.md"), []byte("```yaml\nstatus: done\nrepository: repo\n```\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	target := RepositoryTarget{Repository: "repo", Path: filepath.Join(workspace, "repo")}
	return FinishPlanReport{
		Workspace:             workspace,
		Manifest:              Manifest{Project: "project", BranchName: "feat/task"},
		Targets:               []TargetStatus{{Target: target, Status: RepositoryStatus{IsGitRepository: true}}},
		Handoff:               HandoffValidationReport{IsValid: true},
		CreatePR:              true,
		PullRequestCandidates: []PullRequestCandidate{{Repository: "repo", Path: target.Path, ProviderRepository: "org/repo", TargetBranch: "main"}},
	}
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
