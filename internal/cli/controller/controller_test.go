package controller

import (
	"context"
	"errors"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/workapp"
)

type rendererTestExecutor struct {
	record execution.Record
}

func (executor rendererTestExecutor) Submit(_ context.Context, submission execution.Submission) (execution.ExecutionID, error) {
	if submission.Request.ActionID() != executor.record.ActionID {
		return execution.ExecutionID{}, errors.New("unexpected request action")
	}
	return executor.record.ExecutionID, nil
}
func (rendererTestExecutor) Get(context.Context, execution.Actor, execution.ExecutionID) (execution.Record, error) {
	return execution.Record{}, nil
}
func (rendererTestExecutor) List(context.Context, execution.Actor, execution.ListFilter) ([]execution.Record, error) {
	return nil, nil
}
func (rendererTestExecutor) Cancel(context.Context, execution.Actor, execution.ExecutionID) error {
	return nil
}
func (rendererTestExecutor) Respond(context.Context, execution.Actor, execution.ExecutionID, action.PromptID, action.Response) error {
	return nil
}
func (rendererTestExecutor) Subscribe(context.Context, execution.Actor, execution.ExecutionID, execution.EventSequence) (execution.Subscription, error) {
	events := make(chan execution.Event)
	errors := make(chan error)
	close(events)
	close(errors)
	return execution.Subscription{Events: events, Errors: errors}, nil
}
func (executor rendererTestExecutor) Wait(context.Context, execution.Actor, execution.ExecutionID) (execution.Record, error) {
	return executor.record, nil
}
func (rendererTestExecutor) Close(context.Context) error { return nil }

func TestDispatchRendersAliasByResultActionID(t *testing.T) {
	outcome, err := dispatchRendererAlias(t, workapp.ActionWorkspaceOpen)
	if err != nil {
		t.Fatal(err)
	}
	if text := string(outcome.Output.Body); text != "canonical renderer" {
		t.Fatalf("output = %q", text)
	}
}

func TestDispatchDoesNotFallBackToRouteRenderer(t *testing.T) {
	_, err := dispatchRendererAlias(t, "agent.open")
	var missing console.RendererNotFoundError
	if !errors.As(err, &missing) || missing.Kind != string(workapp.ActionWorkspaceOpen) {
		t.Fatalf("error = %T %v", err, err)
	}
}

func dispatchRendererAlias(t *testing.T, rendererID action.ID) (Outcome, error) {
	t.Helper()
	invocation, parseErr := parse.Parse(spec.Root(nil), []string{"agent", "open", "--workspace", t.TempDir()})
	if parseErr != nil {
		t.Fatal(parseErr)
	}
	result := workapp.OpenReport{Workspace: "workspace", Events: []workapp.Event{}}
	record := execution.Record{ActionID: workapp.ActionWorkspaceOpen, Status: execution.StatusSucceeded, TypedResult: result}
	results := console.NewRegistry()
	if err := console.RegisterResult(results, rendererID, func(console.RenderContext, workapp.OpenReport) (console.Output, error) {
		return console.TextOutput(console.FormatHuman, "canonical renderer"), nil
	}); err != nil {
		t.Fatal(err)
	}
	route := Route{
		Key: "test.agent.open",
		Build: func(*parse.Result) (action.Request, error) {
			return workapp.OpenRequest{}, nil
		},
		Project: func(action.ResultEnvelope, *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
			return console.FormatHuman, nil, nil
		},
	}
	adapter := Execution{Executor: rendererTestExecutor{record: record}, Actor: execution.Actor{Principal: "test", Origin: execution.OriginCLI}, Console: console.NewEngine(results, nil)}
	return (&Controller{}).dispatch(context.Background(), adapter, route, invocation)
}
