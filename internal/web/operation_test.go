package web

import (
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
)

type operationRequest struct{ id action.ID }

func (request operationRequest) ActionID() action.ID { return request.id }

func TestOperationViewSubmitsResourceRelation(t *testing.T) {
	reference := cockpit.ResourceRef{Kind: cockpit.ResourceWorkItem, Root: "/tmp/root", Project: "default", Key: "WI-42"}
	operations := []cockpit.Operation{{
		Relation: cockpit.RelationStart, Subject: reference, Label: "Start work",
		Request: operationRequest{id: "work.start"}, Active: true,
	}}
	views := operationViews("csrf-token", operations)
	if len(views) != 1 {
		t.Fatalf("operation views = %d, want 1", len(views))
	}
	submit := views[0].Submit
	for _, value := range []string{"/operations", `kind:"work-item"`, `project:"default"`, `key:"WI-42"`, `relation:"start"`} {
		if !strings.Contains(submit, value) {
			t.Errorf("submit expression does not contain %q: %s", value, submit)
		}
	}
	for _, forbidden := range []string{"commandKey", "actionId", "work.start"} {
		if strings.Contains(submit, forbidden) {
			t.Errorf("submit expression contains internal identifier %q: %s", forbidden, submit)
		}
	}
}

func TestOperationViewOmitsExternalOperations(t *testing.T) {
	reference := cockpit.ResourceRef{Kind: cockpit.ResourceWorkspace, Root: "/tmp/root", Key: "/tmp/root/work"}
	views := operationViews("csrf-token", []cockpit.Operation{{
		Relation: cockpit.RelationOpenWorkspace, Subject: reference, Label: "Open",
		Request: operationRequest{id: "workspace.open"}, Active: true, Risk: cockpit.RiskExternal,
	}})
	if len(views) != 0 {
		t.Fatalf("external operation views = %d, want 0", len(views))
	}
}
