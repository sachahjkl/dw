package cockpit

import (
	"context"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
)

type affordanceRequest struct {
	id    action.ID
	value string
}

func (request affordanceRequest) ActionID() action.ID { return request.id }

func TestOperationBuildRequestValidatesClosedInput(t *testing.T) {
	reference := ResourceRef{Kind: ResourceWorkItem, Root: "/tmp/root", Project: "default", Key: "WI-42"}
	operation := Operation{
		Relation: RelationChangeState, Subject: reference, Label: "Change state", Active: true,
		Request: affordanceRequest{id: "work.state.set"},
		Inputs: []OperationInput{{
			Name: "state", Label: "State", Kind: InputSelect, Required: true,
			Options: []InputOption{{Value: "active", Label: "Active"}, {Value: "done", Label: "Done"}},
		}},
		Build: func(values []InputValue) (action.Request, error) {
			return affordanceRequest{id: "work.state.set", value: values[0].Value}, nil
		},
	}
	request, err := operation.BuildRequest([]InputValue{{Name: "state", Value: "done"}})
	if err != nil {
		t.Fatal(err)
	}
	if request.(affordanceRequest).value != "done" {
		t.Fatalf("request = %#v", request)
	}
	if _, err = operation.BuildRequest([]InputValue{{Name: "state", Value: "unknown"}}); err == nil || !strings.Contains(err.Error(), "invalid-operation-input") {
		t.Fatalf("invalid input error = %v", err)
	}
}

func TestResolveReloadsAndMatchesResourceRelation(t *testing.T) {
	reference := ResourceRef{Kind: ResourceWorkspace, Root: "/tmp/root", Project: "default", Key: "/tmp/root/work"}
	loads := 0
	service, err := New(func(context.Context, string) (Snapshot, error) {
		loads++
		operation := Operation{
			Relation: RelationSync, Subject: reference, Label: "Sync", Active: true,
			Request: affordanceRequest{id: "work.sync"},
		}
		return Snapshot{Ref: ResourceRef{Kind: ResourceRoot, Root: reference.Root, Key: reference.Root}, Root: reference.Root, Workspaces: []Workspace{{Ref: reference, Operations: []Operation{operation}}}}, nil
	}, func(context.Context, Snapshot) ([]WorkProject, error) {
		return nil, nil
	}, func(context.Context, Snapshot) ([]PullRequest, error) {
		return nil, nil
	})
	if err != nil {
		t.Fatal(err)
	}
	operation, request, err := service.Resolve(context.Background(), reference, RelationSync, nil)
	if err != nil {
		t.Fatal(err)
	}
	if loads != 1 || operation.Relation != RelationSync || request.ActionID() != "work.sync" {
		t.Fatalf("loads=%d operation=%#v request=%#v", loads, operation, request)
	}
	if _, _, err = service.Resolve(context.Background(), reference, RelationFinish, nil); err == nil || !strings.Contains(err.Error(), "operation-not-found") {
		t.Fatalf("missing relation error = %v", err)
	}
}

func TestGlobalDataSourceReferenceIsValidWithoutProject(t *testing.T) {
	global := ResourceRef{Kind: ResourceDataSource, Root: "/tmp/root", Key: "people"}
	if err := global.Validate(); err != nil {
		t.Fatalf("global data source reference = %v", err)
	}
	project := ResourceRef{Kind: ResourceProject, Root: "/tmp/root", Key: "default"}
	if err := project.Validate(); err == nil {
		t.Fatal("project reference without project was accepted")
	}
}
