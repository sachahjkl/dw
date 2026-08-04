package workapp

import (
	"context"
	"errors"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

type finishTestProvider struct{}

func (finishTestProvider) Name() work.ProviderName { return "test" }

type finishTestLookup struct{}

func (finishTestLookup) Resolve(context.Context, string, *string, string, []string, bool) (string, error) {
	return "/workspace", nil
}

type finishSequenceProvider struct{ order *[]string }

func (finishSequenceProvider) Name() work.ProviderName { return "sequence" }
func (provider finishSequenceProvider) ListPullRequests(context.Context, work.ProjectRef, work.PullRequestQuery) ([]work.PullRequest, error) {
	return nil, nil
}
func (provider finishSequenceProvider) ActivePullRequest(context.Context, work.ProjectRef, work.RepositoryName, string) (*work.PullRequest, error) {
	*provider.order = append(*provider.order, "find-pr")
	return nil, nil
}
func (finishSequenceProvider) PullRequestWorkItemIDs(context.Context, work.ProjectRef, work.RepositoryName, work.PullRequestID) ([]work.ItemID, error) {
	return nil, nil
}
func (provider finishSequenceProvider) CreatePullRequest(context.Context, work.ProjectRef, work.PullRequestCreate) (work.PullRequestCreateResult, error) {
	*provider.order = append(*provider.order, "create-pr")
	return work.PullRequestCreateResult{ID: "1", URL: "https://example.invalid/pr/1"}, nil
}
func (finishSequenceProvider) LinkPullRequestWorkItem(context.Context, work.ProjectRef, work.RepositoryName, work.PullRequestID, work.ItemID) error {
	return nil
}

type finishSequenceFinisher struct {
	order *[]string
	err   error
}

func (*finishSequenceFinisher) PlanFinish(context.Context, string, string, string, bool, bool) (workspace.FinishPlanReport, error) {
	return workspace.FinishPlanReport{
		Manifest:              workspace.Manifest{Project: "project", BranchName: "feat/task"},
		Handoff:               workspace.HandoffValidationReport{IsValid: true},
		PullRequestCandidates: []workspace.PullRequestCandidate{{Repository: "repo", ProviderRepository: "org/repo", TargetBranch: "main"}},
	}, nil
}
func (finisher *finishSequenceFinisher) ExecuteLocalFinish(context.Context, workspace.FinishPlanReport, workspace.FinishExecuteOptions, func(workspace.ActionEvent)) (workspace.FinishExecutionReport, error) {
	*finisher.order = append(*finisher.order, "push")
	return workspace.FinishExecutionReport{}, finisher.err
}
func (finishTestLookup) Manifest(context.Context, string) (workspace.Manifest, error) {
	return workspace.Manifest{}, nil
}

type finishTestFinisher struct{ executed bool }

func (*finishTestFinisher) PlanFinish(context.Context, string, string, string, bool, bool) (workspace.FinishPlanReport, error) {
	return workspace.FinishPlanReport{
		Manifest: workspace.Manifest{Project: "project"},
		Handoff:  workspace.HandoffValidationReport{IsValid: true},
	}, nil
}
func (f *finishTestFinisher) ExecuteLocalFinish(_ context.Context, _ workspace.FinishPlanReport, _ workspace.FinishExecuteOptions, emit func(workspace.ActionEvent)) (workspace.FinishExecutionReport, error) {
	f.executed = true
	if emit != nil {
		emit(workspace.ActionEvent{Type: "verifyingFinish", Repository: "repo"})
	}
	return workspace.FinishExecutionReport{}, nil
}

func TestFinishChecksProviderCapabilitiesBeforeLocalEffects(t *testing.T) {
	registry := work.NewRegistry()
	if err := registry.Register(finishTestProvider{}); err != nil {
		t.Fatal(err)
	}
	finisher := &finishTestFinisher{}
	service := &Service{Providers: registry, Lookup: finishTestLookup{}, Finisher: finisher}
	_, err := service.Finish(context.Background(), FinishRequest{Provider: "test", Execute: true, CreatePR: true}, nil)
	if err == nil {
		t.Fatal("Finish succeeded without pull request capabilities")
	}
	if finisher.executed {
		t.Fatal("local finish ran before provider capability validation")
	}
}

func TestFinishForwardsLocalEvents(t *testing.T) {
	finisher := &finishTestFinisher{}
	service := &Service{Lookup: finishTestLookup{}, Finisher: finisher}
	var events []Event

	_, err := service.Finish(context.Background(), FinishRequest{Execute: true}, func(_ context.Context, event Event) error {
		events = append(events, event)
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 1 || events[0].Kind != "verifyingFinish" || events[0].Repository != "repo" {
		t.Fatalf("events = %#v, want forwarded verifyingFinish event", events)
	}
}

func TestFinishPushesBeforePullRequestCreation(t *testing.T) {
	order := []string{}
	registry := work.NewRegistry()
	if err := registry.Register(finishSequenceProvider{order: &order}); err != nil {
		t.Fatal(err)
	}
	service := &Service{Providers: registry, Lookup: finishTestLookup{}, Finisher: &finishSequenceFinisher{order: &order}}
	if _, err := service.Finish(context.Background(), FinishRequest{Provider: "sequence", Execute: true, CreatePR: true}, nil); err != nil {
		t.Fatal(err)
	}
	if want := []string{"push", "find-pr", "create-pr"}; !reflect.DeepEqual(order, want) {
		t.Fatalf("order = %v, want %v", order, want)
	}
}

func TestFinishDoesNotCreatePullRequestAfterPushFailure(t *testing.T) {
	order := []string{}
	registry := work.NewRegistry()
	if err := registry.Register(finishSequenceProvider{order: &order}); err != nil {
		t.Fatal(err)
	}
	service := &Service{Providers: registry, Lookup: finishTestLookup{}, Finisher: &finishSequenceFinisher{order: &order, err: errors.New("push failed")}}
	if _, err := service.Finish(context.Background(), FinishRequest{Provider: "sequence", Execute: true, CreatePR: true}, nil); err == nil {
		t.Fatal("Finish succeeded after push failure")
	}
	if want := []string{"push"}; !reflect.DeepEqual(order, want) {
		t.Fatalf("order = %v, want %v", order, want)
	}
}
