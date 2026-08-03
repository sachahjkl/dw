package tui

import (
	"errors"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
)

type localizedTestError struct{}

type semanticResult struct{}

func (semanticResult) ActionID() action.ID { return "doctor" }

func (localizedTestError) Error() string { return "internal.test-code" }
func (localizedTestError) Localized() l10n.Message {
	return l10n.M("execution.unclassified-error")
}

func TestPersistedExecutionMessageCannotCrashHistoryReplay(t *testing.T) {
	model := NewModel(Dependencies{})
	event := execution.Event{
		Kind: execution.EventInterrupted, ActionID: "doctor",
		Message: execution.MessageV1{Schema: execution.MessageSchemaV1, ID: "execution.interrupted-restart"},
	}
	projected, err := model.projectExecutionEvent(event)
	if err != nil {
		t.Fatal(err)
	}
	if projected.Text != "Action interrupted because DevWorkflow restarted." {
		t.Fatalf("interrupted event = %q", projected.Text)
	}
	event.Message.ID = "plugin.removed-message"
	projected, err = model.projectExecutionEvent(event)
	if err != nil || projected.Text != "Plugin Removed Message" {
		t.Fatalf("unknown persisted event = %#v, %v", projected, err)
	}
}

func TestResultNextActionResolvesNativeOperationForSameSubject(t *testing.T) {
	reference := cockpit.ResourceRef{Kind: cockpit.ResourceRoot, Root: `S:\dw`, Key: `S:\dw`}
	model := NewModel(Dependencies{ProjectPage: func(action.Result) (console.Page, bool, error) {
		return console.Page{Actions: []contract.ActionLink{{Relation: contract.RelationID(cockpit.RelationDoctorFix)}}}, true, nil
	}})
	model.snapshot.Operations = []cockpit.Operation{{
		Relation: cockpit.RelationDoctorFix, Subject: reference, Label: "Fix issues", Active: true,
		Request: operationRequestForTest{id: "doctor"},
	}}
	next := model.nextResultAction(semanticResult{}, reference)
	if next == nil || next.ID != action.ID(cockpit.RelationDoctorFix) || next.Label != "Fix issues" {
		t.Fatalf("native next action = %#v", next)
	}
}

type operationRequestForTest struct{ id action.ID }

func (request operationRequestForTest) ActionID() action.ID { return request.id }

func TestModelLocalizesErrorsBeforeDisplay(t *testing.T) {
	model := NewModel(Dependencies{})
	text := model.errorText(localizedTestError{})
	if strings.Contains(text, "internal.test-code") || text != "The action failed." {
		t.Fatalf("localized error = %q", text)
	}
	if text := model.errorText(errors.New("plain failure")); text != "plain failure" {
		t.Fatalf("plain error = %q", text)
	}
}
