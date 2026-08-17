package workapp

import (
	"context"
	"errors"
	"fmt"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

type startPartialProvider struct{}

func (startPartialProvider) Name() work.ProviderName { return "start-partial" }
func (startPartialProvider) ReadItems(context.Context, work.ProjectRef, []work.ItemID, work.ReadOptions) ([]work.Item, error) {
	return []work.Item{{ID: "7", Title: "Parent"}}, nil
}

type policyChildProvider struct{ created bool }

func (*policyChildProvider) Name() work.ProviderName { return "policy-child" }
func (*policyChildProvider) ReadItems(context.Context, work.ProjectRef, []work.ItemID, work.ReadOptions) ([]work.Item, error) {
	return []work.Item{{ID: "7", Title: "Parent", Type: "User Story"}}, nil
}
func (provider *policyChildProvider) CreateChild(_ context.Context, _ work.ProjectRef, request work.ChildCreate) (work.ChildCreateResult, error) {
	provider.created = true
	return work.ChildCreateResult{ID: "9", Title: request.Title}, nil
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

func TestStartCreatesChildWhenPreflightPolicyRequiresIt(t *testing.T) {
	provider := &policyChildProvider{}
	registry := work.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	service := &Service{Providers: registry, Starter: startPartialStarter{}, Children: &childWriter{}}
	_, execution, err := service.Start(context.Background(), StartRequest{Provider: "policy-child", Project: "project", WorkItemIDs: []string{"7"}, Repositories: []string{"repo"}, Slug: "parent", RequiredChildTaskTypes: []string{" user story "}, Execute: true}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !provider.created || execution == nil || len(execution.ChildTasks) != 1 {
		t.Fatalf("created = %t, execution = %#v", provider.created, execution)
	}
}

type multiParentChildProvider struct {
	parents []work.ItemID
}

func (*multiParentChildProvider) Name() work.ProviderName { return "multi-parent-child" }
func (*multiParentChildProvider) ReadItems(context.Context, work.ProjectRef, []work.ItemID, work.ReadOptions) ([]work.Item, error) {
	return []work.Item{{ID: "7", Title: "First"}, {ID: "8", Title: "Second"}}, nil
}
func (provider *multiParentChildProvider) CreateChild(_ context.Context, _ work.ProjectRef, request work.ChildCreate) (work.ChildCreateResult, error) {
	provider.parents = append(provider.parents, request.ParentID)
	return work.ChildCreateResult{ID: work.ItemID(fmt.Sprintf("child-%d", len(provider.parents))), Title: request.Title}, nil
}

func TestStartCreatesChildrenForEachWorkItem(t *testing.T) {
	provider := &multiParentChildProvider{}
	registry := work.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	service := &Service{Providers: registry, Starter: startPartialStarter{}, Children: &childWriter{}}
	_, execution, err := service.Start(context.Background(), StartRequest{Provider: "multi-parent-child", Project: "project", WorkItemIDs: []string{"7", "8"}, Repositories: []string{"repo"}, Slug: "multi", CreateChildTasks: true, Execute: true}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if execution == nil || len(provider.parents) != 2 || provider.parents[0] != "7" || provider.parents[1] != "8" {
		t.Fatalf("parents = %#v, execution = %#v", provider.parents, execution)
	}
}

type assigningStartProvider struct {
	assigned []work.ItemID
}

func (*assigningStartProvider) Name() work.ProviderName { return "assigning-start" }
func (*assigningStartProvider) ReadItems(context.Context, work.ProjectRef, []work.ItemID, work.ReadOptions) ([]work.Item, error) {
	return []work.Item{{ID: "7", Title: "Parent"}}, nil
}
func (provider *assigningStartProvider) AssignToCurrentUser(_ context.Context, _ work.ProjectRef, ids []work.ItemID) error {
	provider.assigned = append([]work.ItemID(nil), ids...)
	return nil
}

func TestStartAssignsSelectedItemsToCurrentUser(t *testing.T) {
	provider := &assigningStartProvider{}
	registry := work.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	service := &Service{Providers: registry, Starter: startPartialStarter{}}
	_, _, err := service.Start(context.Background(), StartRequest{Provider: "assigning-start", Project: "project", WorkItemIDs: []string{"7"}, Slug: "parent", Execute: true}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if len(provider.assigned) != 1 || provider.assigned[0] != "7" {
		t.Fatalf("assigned = %#v, want [7]", provider.assigned)
	}
}
