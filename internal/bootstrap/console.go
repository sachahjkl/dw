package bootstrap

import (
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/providerapp"
	"github.com/sachahjkl/dw/internal/workapp"
)

func registerConsole(results *console.Registry, events *console.EventRegistry) error {
	registrations := controllerResultRegistrations()
	for _, actionID := range []console.EventKey{
		workapp.ActionProviderAuthLogin,
		workapp.ActionWorkItemList,
		workapp.ActionWorkPullRequestList,
		workapp.ActionWorkChangelog,
		workapp.ActionWorkContextShow,
		workapp.ActionWorkContextAI,
		workapp.ActionWorkItemShow,
		workapp.ActionWorkItemStateExecute,
		workapp.ActionWorkItemStateSet,
		workapp.ActionWorkItemDoingExecute,
		workapp.ActionWorkspaceStart,
		workapp.ActionWorkspacePullRequestStart,
		workapp.ActionWorkspaceOpen,
		workapp.ActionWorkspaceSync,
		workapp.ActionWorkItemChildCreate,
		workapp.ActionWorkspacePrune,
		workapp.ActionWorkspaceFinish,
	} {
		registrations = append(registrations, console.Registration{Action: actionID, Event: workEventRenderer})
	}
	return console.RegisterAll(results, events, registrations...)
}

func controllerResultRegistrations() []console.Registration {
	return []console.Registration{
		{Action: providerapp.ActionList, Result: console.PageRenderer(func(result providerapp.ListReport) console.Page {
			rows := make([][]string, len(result.Providers))
			for index, provider := range result.Providers {
				kinds := make([]string, len(provider.Kinds))
				for kindIndex := range provider.Kinds {
					kinds[kindIndex] = string(provider.Kinds[kindIndex])
				}
				rows[index] = []string{provider.Name, strings.Join(kinds, ", "), strings.Join(provider.Capabilities, ", ")}
			}
			return console.Page{Title: "result.title", Summary: []console.Field{{Label: "result.action", Value: string(providerapp.ActionList)}}, Sections: []console.Section{{Table: &console.Table{Columns: []console.MessageID{"result.provider", "result.kinds", "result.capabilities"}, Rows: rows}}}}
		})},
		{Action: providerapp.ActionShow, Result: console.PageRenderer(func(result providerapp.ShowReport) console.Page {
			kinds := make([]string, len(result.Provider.Kinds))
			for index := range result.Provider.Kinds {
				kinds[index] = string(result.Provider.Kinds[index])
			}
			return resultPage(providerapp.ActionShow, console.Field{Label: "result.provider", Value: result.Provider.Name}, console.Field{Label: "result.kinds", Value: strings.Join(kinds, ", ")}, console.Field{Label: "result.capabilities", Value: strings.Join(result.Provider.Capabilities, ", ")})
		})},
		{Action: providerapp.ActionCapabilities, Result: console.PageRenderer(func(result providerapp.CapabilitiesReport) console.Page {
			kinds := make([]string, len(result.Kinds))
			for index := range result.Kinds {
				kinds[index] = string(result.Kinds[index])
			}
			return resultPage(providerapp.ActionCapabilities, console.Field{Label: "result.provider", Value: result.Provider}, console.Field{Label: "result.kinds", Value: strings.Join(kinds, ", ")}, console.Field{Label: "result.capabilities", Value: strings.Join(result.Capabilities, ", ")})
		})},
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
		{Action: console.ResultWorkspaceStatus, Result: console.PageRenderer(func(result controller.WorkspaceStatusResult) console.Page {
			return resultPage(console.ResultWorkspaceStatus, console.Field{Label: "result.root", Value: result.Root, Style: console.ValuePath}, countField("result.items", len(result.Items)))
		})},
		{Action: console.ResultWorkspaceList, Result: console.PageRenderer(func(result controller.WorkspaceListResult) console.Page {
			return resultPage(console.ResultWorkspaceList, console.Field{Label: "result.root", Value: result.Root, Style: console.ValuePath}, countField("result.items", len(result.Items)))
		})},
		{Action: console.ResultWorkspaceCurrent, Result: console.PageRenderer(func(result controller.WorkspaceCurrentResult) console.Page {
			return resultPage(console.ResultWorkspaceCurrent, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.project", Value: result.Project})
		})},
		{Action: console.ResultWorkspaceItemAdd, Result: console.PageRenderer(workItemUpdatePage(console.ResultWorkspaceItemAdd))},
		{Action: console.ResultWorkspaceItemRemove, Result: console.PageRenderer(workItemUpdatePage(console.ResultWorkspaceItemRemove))},
		{Action: console.ResultWorkspacePreflight, Result: console.PageRenderer(func(result controller.WorkspacePreflightResult) console.Page {
			return resultPage(console.ResultWorkspacePreflight, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.status", Value: strconv.FormatBool(!result.HasBlockingIssues)})
		})},
		{Action: console.ResultWorkspaceRename, Result: console.PageRenderer(func(result controller.WorkspaceRenameResult) console.Page {
			return resultPage(console.ResultWorkspaceRename, console.Field{Label: "result.workspace", Value: result.Plan.NewWorkspace, Style: console.ValuePath}, executedField(result.Execution != nil))
		})},
		{Action: console.ResultWorkspaceAddRepo, Result: console.PageRenderer(func(result controller.WorkspaceRepoAddResult) console.Page {
			return resultPage(console.ResultWorkspaceAddRepo, console.Field{Label: "result.repository", Value: result.Plan.Repository}, executedField(result.Execution != nil))
		})},
		{Action: console.ResultWorkspaceRepoLatest, Result: console.PageRenderer(func(result controller.WorkspaceRepoLatestResult) console.Page {
			count := 0
			if result.Execution != nil {
				count = len(result.Execution.Updated)
			}
			return resultPage(console.ResultWorkspaceRepoLatest, console.Field{Label: "result.workspace", Value: result.Plan.Workspace, Style: console.ValuePath}, countField("result.repositories", count))
		})},
		{Action: console.ResultWorkspaceCommit, Result: console.PageRenderer(func(result controller.WorkspaceCommitResult) console.Page {
			return resultPage(console.ResultWorkspaceCommit, console.Field{Label: "result.workspace", Value: result.Plan.Workspace, Style: console.ValuePath}, executedField(result.Execution != nil))
		})},
		{Action: console.ResultWorkspaceHandoffValidate, Result: console.PageRenderer(func(result controller.WorkspaceHandoffResult) console.Page {
			return resultPage(console.ResultWorkspaceHandoffValidate, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.status", Value: strconv.FormatBool(result.IsValid)})
		})},
		{Action: console.ResultWorkspaceTeardown, Result: console.PageRenderer(func(result controller.WorkspaceTeardownResult) console.Page {
			workspacePath := ""
			if result.Plan.Workspace != nil {
				workspacePath = *result.Plan.Workspace
			}
			return resultPage(console.ResultWorkspaceTeardown, console.Field{Label: "result.workspace", Value: workspacePath, Style: console.ValuePath}, executedField(result.Execution != nil))
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
	projection := console.EventProjection{ActionID: event.ActionID()}
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
