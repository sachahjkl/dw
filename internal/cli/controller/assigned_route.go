package controller

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workapp"
)

func workItemListRoute() Route {
	return Route{Key: "work.item.list", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		project := invocation.Values.String("project")
		if project == "" {
			if invocation.Values.Bool("json") {
				return Outcome{}, usage(fmt.Errorf("cli.work-item-list-project-required"))
			}
			if !execution.Policy.Interactive() {
				return Outcome{}, usage(fmt.Errorf("cli.work-item-list-project-required"))
			}
			projects := config.ProjectValues(resolvedRoot(invocation.Values))
			if len(projects) == 0 {
				return Outcome{}, fmt.Errorf("cli.work-item-list-no-projects")
			}
			if len(projects) == 1 {
				project = projects[0]
			} else {
				choices := make([]action.Choice, len(projects))
				for index, candidate := range projects {
					choices[index] = action.Choice{Value: action.ChoiceValue(candidate), Label: l10n.M(promptChoiceValue, l10n.A("value", candidate))}
				}
				response, err := NewTerminalInput(execution.Policy.Streams, execution.Localizer).Request(ctx, action.Prompt{
					ID: "work-item-list-project", Kind: action.PromptSelectOne, Label: l10n.M(promptProject), Required: true, Choices: choices,
				})
				if err != nil {
					return Outcome{}, err
				}
				project = string(response.Value)
			}
		}
		root := resolvedRoot(invocation.Values)
		request := workapp.AssignedRequest{Provider: selectedWorkProvider(invocation.Values, root, project), Root: root, Project: project, Top: int(invocation.Values.Int("top")), IncludeFinalStates: invocation.Values.Bool("all"), GroupByParent: invocation.Values.Bool("group_by_parent")}
		result, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := assignedProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, "work.item.list", format, projection)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}
