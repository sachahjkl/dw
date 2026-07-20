package bootstrap

import (
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/workapp"
)

func registerConsole(results *console.Registry, events *console.EventRegistry) error {
	registrations := controllerResultRegistrations()
	for _, actionID := range []console.EventKey{
		workapp.ActionAuthLogin,
		workapp.ActionAssigned,
		workapp.ActionPullRequests,
		workapp.ActionChangelog,
		workapp.ActionContext,
		workapp.ActionAIContext,
		workapp.ActionItemShow,
		workapp.ActionStateExecute,
		workapp.ActionStateSet,
		workapp.ActionDoingExecute,
		workapp.ActionStart,
		workapp.ActionStartPullRequest,
		workapp.ActionOpen,
		workapp.ActionSync,
		workapp.ActionChild,
		workapp.ActionPrune,
		workapp.ActionFinish,
	} {
		registrations = append(registrations, console.Registration{Action: actionID, Event: workEventRenderer})
	}
	return console.RegisterAll(results, events, registrations...)
}

func controllerResultRegistrations() []console.Registration {
	return []console.Registration{
		{Action: actionGuide, Result: func(context console.RenderContext, payload any) (console.Output, error) {
			result, ok := payload.(guideResult)
			if !ok {
				return console.Output{}, console.PayloadTypeError{Kind: string(actionGuide)}
			}
			return console.TextOutput(console.FormatHuman, console.RenderGuide(console.GuideResult{Version: result.Version}, context.Localizer, context.Theme)), nil
		}},
		{Action: console.ResultAgentContext, Result: console.PageRenderer(func(result controller.AgentContextResult) console.Page {
			return resultPage(console.ResultAgentContext, console.Field{Label: "result.root", Value: result.Root, Style: console.ValuePath})
		})},
		{Action: console.ResultWorkStatus, Result: console.PageRenderer(func(result controller.WorkspaceStatusResult) console.Page {
			return resultPage(console.ResultWorkStatus, console.Field{Label: "result.root", Value: result.Root, Style: console.ValuePath}, countField("result.items", len(result.Items)))
		})},
		{Action: console.ResultWorkList, Result: console.PageRenderer(func(result controller.WorkspaceListResult) console.Page {
			return resultPage(console.ResultWorkList, console.Field{Label: "result.root", Value: result.Root, Style: console.ValuePath}, countField("result.items", len(result.Items)))
		})},
		{Action: console.ResultWorkCurrent, Result: console.PageRenderer(func(result controller.WorkspaceCurrentResult) console.Page {
			return resultPage(console.ResultWorkCurrent, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.project", Value: result.Project})
		})},
		{Action: console.ResultWorkItemAdd, Result: console.PageRenderer(workItemUpdatePage(console.ResultWorkItemAdd))},
		{Action: console.ResultWorkItemRemove, Result: console.PageRenderer(workItemUpdatePage(console.ResultWorkItemRemove))},
		{Action: console.ResultWorkPreflight, Result: console.PageRenderer(func(result controller.WorkspacePreflightResult) console.Page {
			return resultPage(console.ResultWorkPreflight, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.status", Value: strconv.FormatBool(!result.HasBlockingIssues)})
		})},
		{Action: console.ResultWorkRename, Result: console.PageRenderer(func(result controller.WorkspaceRenameResult) console.Page {
			return resultPage(console.ResultWorkRename, console.Field{Label: "result.workspace", Value: result.Plan.NewWorkspace, Style: console.ValuePath}, executedField(result.Execution != nil))
		})},
		{Action: console.ResultWorkAddRepo, Result: console.PageRenderer(func(result controller.WorkspaceRepoAddResult) console.Page {
			return resultPage(console.ResultWorkAddRepo, console.Field{Label: "result.repository", Value: result.Plan.Repository}, executedField(result.Execution != nil))
		})},
		{Action: console.ResultWorkRepoLatest, Result: console.PageRenderer(func(result controller.WorkspaceRepoLatestResult) console.Page {
			count := 0
			if result.Execution != nil {
				count = len(result.Execution.Updated)
			}
			return resultPage(console.ResultWorkRepoLatest, console.Field{Label: "result.workspace", Value: result.Plan.Workspace, Style: console.ValuePath}, countField("result.repositories", count))
		})},
		{Action: console.ResultWorkCommit, Result: console.PageRenderer(func(result controller.WorkspaceCommitResult) console.Page {
			return resultPage(console.ResultWorkCommit, console.Field{Label: "result.workspace", Value: result.Plan.Workspace, Style: console.ValuePath}, executedField(result.Execution != nil))
		})},
		{Action: console.ResultWorkHandoffValidate, Result: console.PageRenderer(func(result controller.WorkspaceHandoffResult) console.Page {
			return resultPage(console.ResultWorkHandoffValidate, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.status", Value: strconv.FormatBool(result.IsValid)})
		})},
		{Action: console.ResultWorkTeardown, Result: console.PageRenderer(func(result controller.WorkspaceTeardownResult) console.Page {
			workspacePath := ""
			if result.Plan.Workspace != nil {
				workspacePath = *result.Plan.Workspace
			}
			return resultPage(console.ResultWorkTeardown, console.Field{Label: "result.workspace", Value: workspacePath, Style: console.ValuePath}, executedField(result.Execution != nil))
		})},
	}
}

func workItemUpdatePage(kind console.ResultKind) func(controller.WorkspaceItemUpdateResult) console.Page {
	return func(result controller.WorkspaceItemUpdateResult) console.Page {
		return resultPage(kind, console.Field{Label: "result.workspace", Value: result.Plan.NewWorkspace, Style: console.ValuePath}, countField("result.items", len(result.Plan.WorkItems)), executedField(result.Execution != nil))
	}
}

func resultPage(kind console.ResultKind, fields ...console.Field) console.Page {
	return console.Page{Title: "result.title", Summary: append([]console.Field{{Label: "result.action", Value: string(kind)}}, fields...)}
}

func countField(label console.MessageID, count int) console.Field {
	return console.Field{Label: label, Value: strconv.Itoa(count)}
}

func executedField(executed bool) console.Field {
	style := console.ValueWarning
	if executed {
		style = console.ValueSuccess
	}
	return console.Field{Label: "result.executed", Value: strconv.FormatBool(executed), Style: style}
}

func workEventRenderer(payload any) (console.EventProjection, error) {
	event, ok := payload.(workapp.Event)
	if !ok {
		return console.EventProjection{}, console.PayloadTypeError{}
	}
	projection := console.EventProjection{ActionID: workEventActionID(event.Kind)}
	if event.Project != nil {
		projection.Fields = append(projection.Fields, console.EventField{Key: "project", Value: *event.Project})
	}
	if event.VerificationURI != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "verification_uri", Value: event.VerificationURI})
	}
	if event.UserCode != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "user_code", Value: event.UserCode})
	}
	if event.Top != 0 {
		projection.Fields = append(projection.Fields, console.EventField{Key: "top", Value: strconv.Itoa(event.Top)})
	}
	if len(event.Repositories) != 0 {
		projection.Fields = append(projection.Fields, console.EventField{Key: "repositories", Value: strings.Join(event.Repositories, ",")})
	}
	if event.GitTo != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "git_to", Value: event.GitTo})
	}
	if event.ID != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "id", Value: event.ID})
	}
	if len(event.IDs) != 0 {
		projection.Fields = append(projection.Fields, console.EventField{Key: "ids", Value: strings.Join(event.IDs, ",")})
	}
	if event.State != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "state", Value: event.State})
	}
	return projection, nil
}

func workEventActionID(kind string) string {
	switch kind {
	case "authenticating":
		return "ado.auth"
	case "device-login-required":
		return "ado.auth.device.login"
	case "loading-assigned-work-items":
		return "ado.assigned.load"
	case "grouping-assigned-work-items":
		return "ado.assigned.group"
	case "loading-pull-requests":
		return "ado.pr.load"
	case "resolving-pull-request-work-items":
		return "ado.pr.resolve.workitems"
	case "extracting-git-work-items":
		return "ado.git.extract.workitems"
	case "loading-work-item":
		return "ado.workitem.load"
	case "loading-work-items":
		return "ado.workitems.load"
	case "loading-work-item-context":
		return "ado.workitem.context.load"
	case "loading-changelog":
		return "ado.changelog.load"
	case "loading-changelog-items":
		return "ado.changelog.items.load"
	case "updating-work-item-state":
		return "ado.workitem.state.update"
	case "updated-work-item-state":
		return "ado.workitem.state.updated"
	default:
		return ""
	}
}
