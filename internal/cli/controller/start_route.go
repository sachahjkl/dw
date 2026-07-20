package controller

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workapp"
)

func startRoute() Route {
	return Route{Key: "workspace.start", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		built, err := buildWorkspaceStart(invocation)
		if err != nil {
			return Outcome{}, err
		}
		request := built.(workapp.StartRequest)
		result, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := workspacePhaseProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, "workspace.start", format, projection)
		if err != nil {
			return Outcome{}, err
		}
		if request.Execute || invocation.Values.Bool("json") || !execution.Policy.Interactive() {
			return success(output), nil
		}

		if err := console.WriteOutput(execution.Policy.Streams.Stdout, output); err != nil {
			return Outcome{}, err
		}
		accepted, err := askConfirmation(ctx, execution, "workspace-start-create", promptStartCreate)
		if err != nil {
			return Outcome{}, err
		}
		if !accepted {
			return success(console.Output{}), nil
		}
		request.Execute = true
		executed, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		executedOutput, err := execution.Console.RenderResultKind(console.NewRenderContext(execution.Policy, execution.Localizer), executed, "workspace.start", console.FormatHuman, nil)
		if err != nil {
			return Outcome{}, err
		}
		if err := console.WriteOutput(execution.Policy.Streams.Stdout, executedOutput); err != nil {
			return Outcome{}, err
		}

		open, err := askConfirmation(ctx, execution, "workspace-start-open", promptStartOpen)
		if err != nil {
			return Outcome{}, err
		}
		if !open {
			return success(console.Output{}), nil
		}
		start, ok := executed.Result.(workapp.StartResult)
		if !ok || start.Execution == nil {
			return Outcome{}, fmt.Errorf("cli.invalid-result:workspace.start:%T", executed.Result)
		}
		opened, err := dispatchDirect(ctx, execution, invocation, workapp.OpenRequest{Provider: request.Provider, Root: request.Root, Workspace: &start.Execution.Plan.Workspace})
		if err != nil {
			return Outcome{}, err
		}
		report, ok := opened.Result.(workapp.OpenReport)
		if !ok {
			return Outcome{}, fmt.Errorf("cli.invalid-result:workspace.open:%T", opened.Result)
		}
		launch, ok := report.Launch.(agent.Launch)
		if !ok {
			return Outcome{}, fmt.Errorf("cli.invalid-external-launch:workspace.open:%T", report.Launch)
		}
		if err := agent.RunLaunch(ctx, launch, execution.Policy.Streams.Stdin, execution.Policy.Streams.Stdout, execution.Policy.Streams.Stderr); err != nil {
			return Outcome{}, err
		}
		return success(console.Output{}), nil
	}}
}

func askConfirmation(ctx context.Context, execution Execution, id action.PromptID, label l10n.ID) (bool, error) {
	defaultValue := action.ChoiceValue("false")
	response, err := NewTerminalInput(execution.Policy.Streams, execution.Localizer).Request(ctx, action.Prompt{ID: id, Kind: action.PromptConfirm, Label: l10n.M(label), Default: &defaultValue})
	if err != nil {
		return false, err
	}
	return response.Accepted, nil
}

func joinOutputs(first, second console.Output) console.Output {
	if first.Empty() {
		return second
	}
	if second.Empty() {
		return first
	}
	body := make([]byte, 0, len(first.Body)+len(second.Body)+1)
	body = append(body, first.Body...)
	if body[len(body)-1] != '\n' {
		body = append(body, '\n')
	}
	body = append(body, second.Body...)
	return console.Output{Format: console.FormatHuman, Body: body}
}

const (
	promptStartCreate l10n.ID = "cli.prompt.start-create"
	promptStartOpen   l10n.ID = "cli.prompt.start-open"
)
