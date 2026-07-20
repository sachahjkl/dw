package controller

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workapp"
)

func teardownRoute() Route {
	return Route{Key: "workspace.teardown", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		built, err := buildWorkspaceTeardown(invocation)
		if err != nil {
			return Outcome{}, err
		}
		request := built.(WorkspaceTeardownRequest)
		execute := request.Execute
		request.Execute, request.Approved = false, false
		preview, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		previewOutput, err := execution.Console.RenderResultKind(console.NewRenderContext(execution.Policy, execution.Localizer), preview, "workspace.teardown", console.FormatHuman, nil)
		if err != nil {
			return Outcome{}, err
		}
		if !execute {
			if invocation.Values.Bool("json") {
				format, projection, err := workspacePhaseProject(preview, invocation)
				if err != nil {
					return Outcome{}, err
				}
				output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), preview, "workspace.teardown", format, projection)
				return Outcome{Output: output, Code: console.ExitSuccess}, err
			}
			return success(previewOutput), nil
		}
		if !invocation.Values.Bool("json") {
			if err := console.WriteOutput(execution.Policy.Streams.Stdout, previewOutput); err != nil {
				return Outcome{}, err
			}
		}
		if _, err := confirmExecution(ctx, execution, invocation, true, invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkspaceRemove); err != nil {
			return Outcome{}, err
		}
		request.Execute, request.Approved = true, true
		result, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := workspacePhaseProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, "workspace.teardown", format, projection)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}

func pruneRoute() Route {
	return Route{Key: "workspace.prune", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		built, err := buildWorkspacePrune(invocation)
		if err != nil {
			return Outcome{}, err
		}
		request := built.(workapp.PruneRequest)
		execute := request.Execute
		request.Execute, request.SelectedWorkspaces = false, nil
		preview, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		report, ok := preview.Result.(workapp.PruneReport)
		if !ok {
			return Outcome{}, fmt.Errorf("cli.invalid-result:workspace.prune:%T", preview.Result)
		}
		previewOutput, err := execution.Console.RenderResultKind(console.NewRenderContext(execution.Policy, execution.Localizer), preview, "workspace.prune", console.FormatHuman, nil)
		if err != nil {
			return Outcome{}, err
		}
		if !execute {
			if invocation.Values.Bool("json") {
				format, projection, err := workspacePhaseProject(preview, invocation)
				if err != nil {
					return Outcome{}, err
				}
				output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), preview, "workspace.prune", format, projection)
				return Outcome{Output: output, Code: console.ExitSuccess}, err
			}
			return success(previewOutput), nil
		}
		if invocation.Values.Bool("json") && !invocation.Values.Bool("yes") {
			return Outcome{}, usage(fmt.Errorf("cli.confirmation-required:workspace.prune"))
		}
		if !execution.Policy.Interactive() && !invocation.Values.Bool("yes") {
			return Outcome{}, usage(fmt.Errorf("cli.confirmation-required:workspace.prune"))
		}
		if !invocation.Values.Bool("json") {
			if err := console.WriteOutput(execution.Policy.Streams.Stdout, previewOutput); err != nil {
				return Outcome{}, err
			}
		}
		selected := make([]string, 0, len(report.Plan.Candidates))
		if invocation.Values.Bool("yes") {
			for _, candidate := range report.Plan.Candidates {
				selected = append(selected, candidate.Path)
			}
		} else {
			choices := make([]action.Choice, len(report.Plan.Candidates))
			for index, candidate := range report.Plan.Candidates {
				choices[index] = action.Choice{Value: action.ChoiceValue(candidate.Path), Label: l10n.M(promptChoiceValue, l10n.A("value", candidate.Path))}
			}
			if len(choices) != 0 {
				response, err := NewTerminalInput(execution.Policy.Streams, execution.Localizer).Request(ctx, action.Prompt{ID: "workspace-prune-candidates", Kind: action.PromptSelectMany, Label: l10n.M(promptWorkspacePrune), Choices: choices})
				if err != nil {
					return Outcome{}, err
				}
				for _, value := range response.Values {
					selected = append(selected, string(value))
				}
			}
		}
		if len(selected) == 0 {
			return success(console.Output{}), nil
		}
		request.Execute, request.SelectedWorkspaces = true, selected
		result, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := workspacePhaseProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, "workspace.prune", format, projection)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}
