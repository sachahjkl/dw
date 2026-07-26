package controller

import (
	"context"
	"strings"

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
			root := resolvedRoot(invocation.Values)
			projects := config.ProjectValues(root)
			if invocation.Values.Bool("json") || !execution.Policy.Interactive() {
				return Outcome{}, usage(&projectSelectionError{Projects: projects})
			}
			if len(projects) == 0 {
				return Outcome{}, usage(&projectSelectionError{})
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

type projectSelectionError struct {
	Projects []string
}

func (*projectSelectionError) Error() string { return "cli.work-item-list-project-required" }
func (problem *projectSelectionError) Localized() l10n.Message {
	if len(problem.Projects) == 0 {
		return l10n.M(errorNoProjects)
	}
	return l10n.M(errorProjectRequired, l10n.A("projects", strings.Join(problem.Projects, ", ")))
}
