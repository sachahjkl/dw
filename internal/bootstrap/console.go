package bootstrap

import (
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/providerapp"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
	"github.com/sachahjkl/dw/internal/workspaceapp"
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
		console.PageRegistration(providerapp.ActionList, providerListPage),
		console.PageRegistration(providerapp.ActionShow, func(result providerapp.ShowReport) console.Page {
			page := resultPage(providerapp.ActionShow,
				console.Field{Label: "result.provider", Value: result.Provider.Name},
				console.Field{Label: "result.type", Value: providerKinds(result.Provider.Kinds)},
			)
			page.Sections = []console.Section{{Title: "result.features", Items: providerFeatures(result.Provider.Capabilities)}}
			return page
		}),
		console.PageRegistration(providerapp.ActionCapabilities, func(result providerapp.CapabilitiesReport) console.Page {
			rows := make([][]string, len(result.Capabilities))
			for index, capability := range result.Capabilities {
				rows[index] = []string{capability, capabilityDescription(capability)}
			}
			page := resultPage(providerapp.ActionCapabilities,
				console.Field{Label: "result.provider", Value: result.Provider},
				console.Field{Label: "result.type", Value: providerKinds(result.Kinds)},
			)
			page.Sections = []console.Section{{Table: &console.Table{Columns: []console.MessageID{"result.capability", "result.description"}, Rows: rows}}}
			return page
		}),
		{Action: actionGuide, Result: func(context console.RenderContext, payload any) (console.Output, error) {
			result, ok := payload.(guideResult)
			if !ok {
				return console.Output{}, console.PayloadTypeError{Kind: string(actionGuide)}
			}
			return console.TextOutput(console.FormatHuman, console.RenderGuide(console.GuideResult{Version: result.Version}, context.Localizer, context.Theme)), nil
		}},
		console.PageRegistration(console.ResultAgentContext, func(result controller.AgentContextResult) console.Page {
			return resultPage(console.ResultAgentContext, console.Field{Label: "result.root", Value: result.Root, Style: console.ValuePath})
		}),
		console.PageRegistration(console.ResultWorkspaceStatus, func(result workspaceapp.StatusResult) console.Page {
			return workspaceListPage(console.ResultWorkspaceStatus, result.Root, result.Items)
		}),
		console.PageRegistration(console.ResultWorkspaceList, func(result workspaceapp.ListResult) console.Page {
			return workspaceListPage(console.ResultWorkspaceList, result.Root, result.Items)
		}),
		console.PageRegistration(console.ResultWorkspaceCurrent, func(result workspaceapp.CurrentResult) console.Page {
			return resultPage(console.ResultWorkspaceCurrent, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, console.Field{Label: "result.project", Value: result.Project})
		}),
		console.PageRegistration(console.ResultWorkspaceItemAdd, workItemUpdatePage(console.ResultWorkspaceItemAdd)),
		console.PageRegistration(console.ResultWorkspaceItemRemove, workItemUpdatePage(console.ResultWorkspaceItemRemove)),
		console.PageRegistration(console.ResultWorkspacePreflight, func(result workspaceapp.PreflightResult) console.Page {
			return resultPage(console.ResultWorkspacePreflight, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, statusField(!result.HasBlockingIssues, "Ready", "Blocked"))
		}),
		console.PageRegistration(console.ResultWorkspaceRename, func(result workspaceapp.RenameResult) console.Page {
			return resultPage(console.ResultWorkspaceRename, console.Field{Label: "result.workspace", Value: result.Plan.NewWorkspace, Style: console.ValuePath}, executedField(result.Execution != nil))
		}),
		console.PageRegistration(console.ResultWorkspaceAddRepo, func(result workspaceapp.RepoAddResult) console.Page {
			return resultPage(console.ResultWorkspaceAddRepo, console.Field{Label: "result.repository", Value: result.Plan.Repository}, executedField(result.Execution != nil))
		}),
		console.PageRegistration(console.ResultWorkspaceRepoLatest, func(result workspaceapp.RepoLatestResult) console.Page {
			count := 0
			if result.Execution != nil {
				count = len(result.Execution.Updated)
			}
			return resultPage(console.ResultWorkspaceRepoLatest, console.Field{Label: "result.workspace", Value: result.Plan.Workspace, Style: console.ValuePath}, countField("result.repositories", count))
		}),
		console.PageRegistration(console.ResultWorkspaceCommit, func(result workspaceapp.CommitResult) console.Page {
			return resultPage(console.ResultWorkspaceCommit, console.Field{Label: "result.workspace", Value: result.Plan.Workspace, Style: console.ValuePath}, executedField(result.Execution != nil))
		}),
		console.PageRegistration(console.ResultWorkspaceHandoffValidate, func(result workspaceapp.HandoffResult) console.Page {
			return resultPage(console.ResultWorkspaceHandoffValidate, console.Field{Label: "result.workspace", Value: result.Workspace, Style: console.ValuePath}, statusField(result.IsValid, "Valid", "Invalid"))
		}),
		console.PageRegistration(console.ResultWorkspaceTeardown, func(result workspaceapp.TeardownResult) console.Page {
			workspacePath := ""
			if result.Plan.Workspace != nil {
				workspacePath = *result.Plan.Workspace
			}
			return resultPage(console.ResultWorkspaceTeardown, console.Field{Label: "result.workspace", Value: workspacePath, Style: console.ValuePath}, executedField(result.Execution != nil))
		}),
	}
}

func providerListPage(result providerapp.ListReport) console.Page {
	rows := make([][]string, 0)
	for _, provider := range result.Providers {
		features := providerFeatures(provider.Capabilities)
		if len(features) == 0 {
			features = []string{"None"}
		}
		for _, feature := range features {
			rows = append(rows, []string{provider.Name, providerKinds(provider.Kinds), feature})
		}
	}
	page := resultPage(providerapp.ActionList, countField("result.items", len(result.Providers)))
	page.Sections = []console.Section{{Table: &console.Table{Columns: []console.MessageID{"result.provider", "result.type", "result.features"}, Rows: rows}}}
	return page
}

func providerKinds(kinds []providerapp.Kind) string {
	values := make([]string, len(kinds))
	for index, kind := range kinds {
		values[index] = console.HumanizeIdentifier(string(kind))
	}
	return strings.Join(values, ", ")
}

func providerFeatures(capabilities []string) []string {
	seen := make(map[string]struct{}, len(capabilities))
	features := make([]string, 0, len(capabilities))
	for _, capability := range capabilities {
		feature := capabilityFeature(capability)
		if _, exists := seen[feature]; exists {
			continue
		}
		seen[feature] = struct{}{}
		features = append(features, feature)
	}
	return features
}

func capabilityFeature(capability string) string {
	switch capability {
	case "authenticator":
		return "Authentication"
	case "item-reader", "assigned-querier", "raw-item-reader":
		return "Work items"
	case "relation-reader", "rich-context-reader":
		return "Relationships and context"
	case "state-writer", "state-classifier":
		return "State updates"
	case "child-creator":
		return "Child items"
	case "pull-request-reader", "pull-request-writer":
		return "Pull requests"
	case "commit-reference-extractor":
		return "Commit references"
	case "discoverer":
		return "Discovery"
	case "cataloger", "describer":
		return "Catalog"
	case "native-querier":
		return "SQL queries"
	case "tabular-reader":
		return "Tables"
	case "workbook-reader":
		return "Workbooks"
	case "document-reader":
		return "Documents"
	case "read-policy":
		return "Read-only guard"
	case "credential-resolver":
		return "Credentials"
	default:
		return console.HumanizeIdentifier(capability)
	}
}

func capabilityDescription(capability string) string {
	switch capability {
	case "authenticator":
		return "Sign in and inspect connection status."
	case "item-reader":
		return "Read work item details."
	case "assigned-querier":
		return "List work assigned to the current user."
	case "relation-reader":
		return "Load parent and child relationships."
	case "state-writer":
		return "Change work item states."
	case "state-classifier":
		return "Distinguish active and final states."
	case "child-creator":
		return "Create child work items."
	case "pull-request-reader":
		return "Read pull requests and linked work items."
	case "pull-request-writer":
		return "Create and update pull requests."
	case "rich-context-reader":
		return "Load structured implementation context."
	case "raw-item-reader":
		return "Load provider-native work item data."
	case "commit-reference-extractor":
		return "Find work items referenced by commits."
	case "discoverer":
		return "Discover local data sources."
	case "cataloger":
		return "List available data resources."
	case "describer":
		return "Describe resource fields."
	case "native-querier":
		return "Run provider-native read queries."
	case "tabular-reader":
		return "Read tabular rows."
	case "workbook-reader":
		return "Read workbook data."
	case "document-reader":
		return "Read document content."
	case "read-policy":
		return "Reject unsafe data operations."
	case "credential-resolver":
		return "Resolve configured credentials."
	default:
		return console.HumanizeIdentifier(capability) + "."
	}
}

func workspaceListPage(kind console.ResultKind, root string, items []workspace.ListItem) console.Page {
	page := resultPage(kind, console.Field{Label: "result.root", Value: root, Style: console.ValuePath}, countField("result.items", len(items)))
	if len(items) == 0 {
		page.Summary = append(page.Summary, console.Field{Label: "result.status", Value: "No workspaces found", Style: console.ValueWarning})
		page.Actions = []contract.ActionLink{{Relation: "start"}}
		return page
	}
	rows := make([][]string, len(items))
	for index, item := range items {
		state := ""
		if item.WorkItemState != nil {
			state = *item.WorkItemState
		}
		rows[index] = []string{item.Project, item.WorkItemID, state, item.BranchName, item.Path}
	}
	page.Sections = []console.Section{{Table: &console.Table{Columns: []console.MessageID{"result.project", "result.item", "result.state", "result.branch", "result.workspace"}, Rows: rows}}}
	return page
}

func statusField(ok bool, success, failure string) console.Field {
	if ok {
		return console.Field{Label: "result.status", Value: success, Style: console.ValueSuccess}
	}
	return console.Field{Label: "result.status", Value: failure, Style: console.ValueFailure}
}

func workItemUpdatePage(kind console.ResultKind) func(workspaceapp.ItemUpdateResult) console.Page {
	return func(result workspaceapp.ItemUpdateResult) console.Page {
		return resultPage(kind, console.Field{Label: "result.workspace", Value: result.Plan.NewWorkspace, Style: console.ValuePath}, countField("result.items", len(result.Plan.WorkItems)), executedField(result.Execution != nil))
	}
}

func resultPage(kind console.ResultKind, fields ...console.Field) console.Page {
	return console.ActionPage(string(kind), fields...)
}

func countField(label console.MessageID, count int) console.Field {
	return console.Field{Label: label, Value: strconv.Itoa(count)}
}

func executedField(executed bool) console.Field {
	if executed {
		return console.Field{Label: "result.executed", Value: "Yes", Style: console.ValueSuccess}
	}
	return console.Field{Label: "result.executed", Value: "No", Style: console.ValueWarning}
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
	if event.AuthorizationURL != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "authorization_url", Value: event.AuthorizationURL})
	}
	if event.CallbackURI != "" {
		projection.Fields = append(projection.Fields, console.EventField{Key: "callback_uri", Value: event.CallbackURI})
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
