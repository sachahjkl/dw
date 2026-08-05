package workapp

import (
	"context"
	"errors"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

type startPartialProvider struct{}

func (startPartialProvider) Name() work.ProviderName { return "start-partial" }
func (startPartialProvider) ReadItems(context.Context, work.ProjectRef, []work.ItemID, work.ReadOptions) ([]work.Item, error) {
	return []work.Item{{ID: "7", Title: "Parent"}}, nil
}
func (startPartialProvider) CreateChild(context.Context, work.ProjectRef, work.ChildCreate) (work.ChildCreateResult, error) {
	return work.ChildCreateResult{ID: "9", Title: "Child"}, errors.New("link failed")
}

type startPartialStarter struct{}

func (startPartialStarter) PlanStart(_ context.Context, request workspace.StartRequest) (workspace.StartPlan, error) {
	return workspace.StartPlan{WorkItemIDs: request.WorkItemIDs, PrimaryWorkItemID: request.WorkItemIDs[0], Project: request.Project, Workspace: "/workspace", Repositories: request.Repositories}, nil
}
func (startPartialStarter) ExecuteStart(_ context.Context, plan workspace.StartPlan, items []workspace.WorkItem, _ []workspace.ChildTask, _ func(workspace.ActionEvent)) (workspace.StartExecutionReport, error) {
	return workspace.StartExecutionReport{Plan: plan, Manifest: workspace.Manifest{WorkItemID: plan.PrimaryWorkItemID}, WorkItems: items}, nil
}

func TestStartReportsRemoteChildWhenLinkFails(t *testing.T) {
	registry := work.NewRegistry()
	if err := registry.Register(startPartialProvider{}); err != nil {
		t.Fatal(err)
	}
	writer := &childWriter{}
	service := &Service{Providers: registry, Starter: startPartialStarter{}, Children: writer}
	_, execution, err := service.Start(context.Background(), StartRequest{Provider: "start-partial", Project: "project", WorkItemIDs: []string{"7"}, Repositories: []string{"repo"}, Slug: "parent", CreateChildTasks: true, Execute: true}, nil)
	if err == nil || execution == nil {
		t.Fatalf("execution = %#v, err = %v", execution, err)
	}
	if len(execution.ChildTasks) != 1 || execution.ChildTasks[0].ID != "9" || execution.ChildTasks[0].Repository != "repo" {
		t.Fatalf("child tasks = %#v", execution.ChildTasks)
	}
	if writer.called {
		t.Fatal("partial child was persisted after provider failure")
	}
}
