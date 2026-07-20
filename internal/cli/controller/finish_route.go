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

func finishRoute() Route {
	return Route{Key: "work.finish", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		createPR := invocation.Values.Bool("create_pr")
		ready := invocation.Values.Bool("ready")
		skipWork := invocation.Values.Bool("skip_ado")
		execute := invocation.Values.Bool("execute")
		machine := invocation.Values.Bool("json")
		if execute && !machine && execution.Policy.Interactive() && !createPR && !ready && !skipWork {
			choices := []action.Choice{
				{Value: "push-only", Label: l10n.M(promptFinishPush)},
				{Value: "draft-pr", Label: l10n.M(promptFinishDraft)},
				{Value: "ready-pr", Label: l10n.M(promptFinishReady)},
				{Value: "keep", Label: l10n.M(promptFinishKeep)},
			}
			response, err := NewTerminalInput(execution.Policy.Streams, execution.Localizer).Request(ctx, action.Prompt{ID: "finish-mode", Kind: action.PromptSelectOne, Label: l10n.M(promptFinishMode), Required: true, Choices: choices})
			if err != nil {
				return Outcome{}, err
			}
			switch response.Value {
			case "push-only":
				skipWork = true
			case "draft-pr":
				createPR = true
			case "ready-pr":
				createPR, ready = true, true
			}
		}
		root := resolvedRoot(invocation.Values)
		request := workapp.FinishRequest{Root: root, Workspace: optional(invocation.Values, "workspace"), Continue: invocation.Values.Bool("continue"), CreatePR: createPR, Ready: ready, SkipVerify: invocation.Values.Bool("skip_verify"), SkipWork: skipWork, ForceWithLease: invocation.Values.Bool("force_with_lease"), Message: optional(invocation.Values, "message"), FinishStates: taskFinishStates(root)}
		preview, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		plan, ok := preview.Result.(workapp.FinishReport)
		if !ok {
			return Outcome{}, fmt.Errorf("cli.invalid-result:work.finish:%T", preview.Result)
		}
		previewOutput, err := execution.Console.RenderResultKind(console.NewRenderContext(execution.Policy, execution.Localizer), preview, "work.finish", console.FormatHuman, nil)
		if err != nil {
			return Outcome{}, err
		}
		if !execute {
			if machine {
				format, projection, err := workspacePhaseProject(preview, invocation)
				if err != nil {
					return Outcome{}, err
				}
				output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), preview, "work.finish", format, projection)
				return Outcome{Output: output, Code: console.ExitSuccess}, err
			}
			return success(previewOutput), nil
		}
		if !machine {
			if err := console.WriteOutput(execution.Policy.Streams.Stdout, previewOutput); err != nil {
				return Outcome{}, err
			}
		}
		if len(plan.Plan.ActionableRepositories) == 0 && len(plan.Plan.PullRequestCandidates) == 0 && skipWork {
			return success(console.Output{}), nil
		}
		if _, err := confirmExecution(ctx, execution, invocation, true, invocation.Values.Bool("yes"), machine, promptWorkFinish); err != nil {
			return Outcome{}, err
		}
		request.Execute = true
		result, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := workspacePhaseProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, "work.finish", format, projection)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}

const (
	promptFinishMode  l10n.ID = "cli.prompt.finish-mode"
	promptFinishPush  l10n.ID = "cli.prompt.finish-push"
	promptFinishDraft l10n.ID = "cli.prompt.finish-draft"
	promptFinishReady l10n.ID = "cli.prompt.finish-ready"
	promptFinishKeep  l10n.ID = "cli.prompt.finish-keep"
)
