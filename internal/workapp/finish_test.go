package workapp

import (
	"context"
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
func (f *finishTestFinisher) ExecuteLocalFinish(context.Context, workspace.FinishPlanReport, workspace.FinishExecuteOptions, func(workspace.ActionEvent)) (workspace.FinishExecutionReport, error) {
	f.executed = true
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
