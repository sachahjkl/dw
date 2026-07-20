package controller

import (
	"context"
	"fmt"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
)

const (
	actionAgentContext        = action.ID("agent.context")
	actionWorkspaceStatus     = action.ID("workspace.status")
	actionWorkspaceList       = action.ID("workspace.list")
	actionWorkspaceCurrent    = action.ID("workspace.current")
	actionWorkspaceItemAdd    = action.ID("workspace.item.add")
	actionWorkspaceItemRemove = action.ID("workspace.item.remove")
	actionWorkspacePreflight  = action.ID("workspace.preflight")
	actionWorkspaceRename     = action.ID("workspace.rename")
	actionWorkspaceRepoAdd    = action.ID("workspace.repo.add")
	actionWorkspaceRepoLatest = action.ID("workspace.repo.latest")
	actionWorkspaceCommit     = action.ID("workspace.commit")
	actionWorkspaceHandoff    = action.ID("workspace.handoff.validate")
	actionWorkspaceTeardown   = action.ID("workspace.teardown")
)

type AgentContextRequest struct {
	Root string `json:"root,omitempty"`
}
type AgentContextResult struct{ agent.ContextReport }

func (AgentContextRequest) ActionID() action.ID { return actionAgentContext }
func (AgentContextResult) ActionID() action.ID  { return actionAgentContext }

type WorkspaceSelection struct {
	Root      string   `json:"root,omitempty"`
	Workspace *string  `json:"workspace,omitempty"`
	Project   string   `json:"project,omitempty"`
	IDs       []string `json:"workItemIds,omitempty"`
	Continue  bool     `json:"continue"`
}

type WorkspaceStatusRequest struct {
	Root string `json:"root,omitempty"`
}
type WorkspaceListRequest struct {
	Root        string   `json:"root,omitempty"`
	Project     *string  `json:"project,omitempty"`
	WorkItemIDs []string `json:"workItemIds"`
}
type WorkspaceCurrentRequest struct{}
type WorkspaceItemAddRequest struct {
	Selection          WorkspaceSelection `json:"selection"`
	IDs                []string           `json:"ids"`
	Provider           string             `json:"provider,omitempty"`
	SkipWork           bool               `json:"skipWork"`
	Type, Title, State string
	Execute            bool `json:"execute"`
}
type WorkspaceItemRemoveRequest struct {
	Selection WorkspaceSelection `json:"selection"`
	IDs       []string           `json:"ids"`
	Execute   bool               `json:"execute"`
}
type WorkspacePreflightRequest struct {
	Selection WorkspaceSelection `json:"selection"`
	Files     []string           `json:"files"`
}
type WorkspaceRenameRequest struct {
	Selection WorkspaceSelection `json:"selection"`
	Slug      string             `json:"slug"`
	Execute   bool               `json:"execute"`
}
type WorkspaceRepoAddRequest struct {
	Selection  WorkspaceSelection `json:"selection"`
	Repository string             `json:"repository"`
	Execute    bool               `json:"execute"`
}
type WorkspaceRepoLatestRequest struct {
	Selection    WorkspaceSelection `json:"selection"`
	Repositories []string           `json:"repositories"`
	Execute      bool               `json:"execute"`
}
type WorkspaceCommitRequest struct {
	Selection WorkspaceSelection `json:"selection"`
	Message   string             `json:"message,omitempty"`
	Execute   bool               `json:"execute"`
}
type WorkspaceHandoffRequest struct {
	Selection WorkspaceSelection `json:"selection"`
}
type WorkspaceTeardownRequest struct {
	Selection         WorkspaceSelection `json:"selection"`
	Execute, Approved bool
}

func (WorkspaceStatusRequest) ActionID() action.ID     { return actionWorkspaceStatus }
func (WorkspaceListRequest) ActionID() action.ID       { return actionWorkspaceList }
func (WorkspaceCurrentRequest) ActionID() action.ID    { return actionWorkspaceCurrent }
func (WorkspaceItemAddRequest) ActionID() action.ID    { return actionWorkspaceItemAdd }
func (WorkspaceItemRemoveRequest) ActionID() action.ID { return actionWorkspaceItemRemove }
func (WorkspacePreflightRequest) ActionID() action.ID  { return actionWorkspacePreflight }
func (WorkspaceRenameRequest) ActionID() action.ID     { return actionWorkspaceRename }
func (WorkspaceRepoAddRequest) ActionID() action.ID    { return actionWorkspaceRepoAdd }
func (WorkspaceRepoLatestRequest) ActionID() action.ID { return actionWorkspaceRepoLatest }
func (WorkspaceCommitRequest) ActionID() action.ID     { return actionWorkspaceCommit }
func (WorkspaceHandoffRequest) ActionID() action.ID    { return actionWorkspaceHandoff }
func (WorkspaceTeardownRequest) ActionID() action.ID   { return actionWorkspaceTeardown }

type WorkspaceStatusResult struct{ workspace.StatusReport }
type WorkspaceListResult struct{ workspace.ListReport }
type WorkspaceCurrentResult struct{ workspace.CurrentItem }
type WorkspaceItemUpdateResult struct {
	Plan      workspace.WorkItemUpdatePlan    `json:"plan"`
	Execution *workspace.WorkItemUpdateReport `json:"execution,omitempty"`
	operation action.ID
}
type WorkspacePreflightResult struct{ workspace.PreflightReport }
type WorkspaceRenameResult struct {
	Plan      workspace.RenamePlan             `json:"plan"`
	Execution *workspace.RenameExecutionReport `json:"execution,omitempty"`
}
type WorkspaceRepoAddResult struct {
	Plan      workspace.AddRepositoryPlan    `json:"plan"`
	Execution *workspace.AddRepositoryReport `json:"execution,omitempty"`
}
type WorkspaceRepoLatestResult struct {
	Plan      workspace.RepositoryLatestPlanReport       `json:"plan"`
	Execution *workspace.RepositoryLatestExecutionReport `json:"execution,omitempty"`
}
type WorkspaceCommitResult struct {
	Plan      workspace.CommitPlanReport       `json:"plan"`
	Execution *workspace.CommitExecutionReport `json:"execution,omitempty"`
}
type WorkspaceHandoffResult struct {
	workspace.HandoffValidationReport
}
type WorkspaceTeardownResult struct {
	Plan      workspace.TeardownPlanReport       `json:"plan"`
	Execution *workspace.TeardownExecutionReport `json:"execution,omitempty"`
}

func (WorkspaceStatusResult) ActionID() action.ID       { return actionWorkspaceStatus }
func (WorkspaceListResult) ActionID() action.ID         { return actionWorkspaceList }
func (WorkspaceCurrentResult) ActionID() action.ID      { return actionWorkspaceCurrent }
func (r WorkspaceItemUpdateResult) ActionID() action.ID { return r.operation }
func (WorkspacePreflightResult) ActionID() action.ID    { return actionWorkspacePreflight }
func (WorkspaceRenameResult) ActionID() action.ID       { return actionWorkspaceRename }
func (WorkspaceRepoAddResult) ActionID() action.ID      { return actionWorkspaceRepoAdd }
func (WorkspaceRepoLatestResult) ActionID() action.ID   { return actionWorkspaceRepoLatest }
func (WorkspaceCommitResult) ActionID() action.ID       { return actionWorkspaceCommit }
func (WorkspaceHandoffResult) ActionID() action.ID      { return actionWorkspaceHandoff }
func (WorkspaceTeardownResult) ActionID() action.ID     { return actionWorkspaceTeardown }

type WorkspaceWorkItemLoader interface {
	LoadWorkspaceItems(context.Context, string, string, string, []string) ([]workspace.WorkItem, error)
}

type workspaceActions struct {
	engine           *workspace.Engine
	workItems        WorkspaceWorkItemLoader
	currentDirectory string
}

// WorkspaceHandlers adapts provider-neutral workspace lifecycle operations to
// the shared dispatcher without leaking CLI command IDs into the domain.
func WorkspaceHandlers(engine *workspace.Engine, workItems WorkspaceWorkItemLoader, currentDirectory string) []action.Handler {
	service := workspaceActions{engine: engine, workItems: workItems, currentDirectory: currentDirectory}
	return []action.Handler{
		controllerHandler[WorkspaceStatusRequest](actionWorkspaceStatus, service.status),
		controllerHandler[WorkspaceListRequest](actionWorkspaceList, service.list),
		controllerHandler[WorkspaceCurrentRequest](actionWorkspaceCurrent, service.current),
		controllerHandler[WorkspaceItemAddRequest](actionWorkspaceItemAdd, service.itemAdd),
		controllerHandler[WorkspaceItemRemoveRequest](actionWorkspaceItemRemove, service.itemRemove),
		controllerHandler[WorkspacePreflightRequest](actionWorkspacePreflight, service.preflight),
		controllerHandler[WorkspaceRenameRequest](actionWorkspaceRename, service.rename),
		controllerHandler[WorkspaceRepoAddRequest](actionWorkspaceRepoAdd, service.repoAdd),
		controllerHandler[WorkspaceRepoLatestRequest](actionWorkspaceRepoLatest, service.repoLatest),
		controllerHandler[WorkspaceCommitRequest](actionWorkspaceCommit, service.commit),
		controllerHandler[WorkspaceHandoffRequest](actionWorkspaceHandoff, service.handoff),
		controllerHandler[WorkspaceTeardownRequest](actionWorkspaceTeardown, service.teardown),
	}
}

// IntegrationHandlers contains controller-owned non-workspace adapters.
func IntegrationHandlers() []action.Handler {
	return []action.Handler{controllerHandler[AgentContextRequest](actionAgentContext, func(_ context.Context, request AgentContextRequest, _ action.Runtime) (action.Result, error) {
		return AgentContextResult{ContextReport: agent.Context(config.ResolveRoot(request.Root))}, nil
	})}
}

func controllerHandler[T action.Request](id action.ID, execute func(context.Context, T, action.Runtime) (action.Result, error)) action.Handler {
	return action.HandlerFunc{Action: id, ExecuteFunc: func(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
		typed, ok := request.(T)
		if !ok {
			return nil, fmt.Errorf("cli.invalid-action-request:%s:%T", id, request)
		}
		return execute(ctx, typed, runtime)
	}}
}

func (s workspaceActions) require() error {
	if s.engine == nil {
		return fmt.Errorf("cli.nil-workspace-engine")
	}
	return nil
}
func (s workspaceActions) root(value string) string { return config.ResolveRoot(value) }
func (s workspaceActions) resolve(selection WorkspaceSelection) (string, string, error) {
	if err := s.require(); err != nil {
		return "", "", err
	}
	root := s.root(selection.Root)
	explicit := ""
	if selection.Workspace != nil {
		explicit = *selection.Workspace
	}
	path, err := workspace.Resolve(root, explicit, selection.Project, selection.IDs, selection.Continue, s.currentDirectory)
	return root, path, err
}
func (s workspaceActions) status(_ context.Context, request WorkspaceStatusRequest, _ action.Runtime) (action.Result, error) {
	root := s.root(request.Root)
	return WorkspaceStatusResult{StatusReport: workspace.BuildStatusReport(root)}, nil
}
func (s workspaceActions) list(_ context.Context, request WorkspaceListRequest, _ action.Runtime) (action.Result, error) {
	root := s.root(request.Root)
	return WorkspaceListResult{ListReport: workspace.BuildListReport(root, request.Project, request.WorkItemIDs)}, nil
}
func (s workspaceActions) current(_ context.Context, _ WorkspaceCurrentRequest, _ action.Runtime) (action.Result, error) {
	report, err := workspace.Current(s.currentDirectory)
	return WorkspaceCurrentResult{CurrentItem: report}, err
}

func (s workspaceActions) itemAdd(ctx context.Context, request WorkspaceItemAddRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	request.IDs, err = requestIDs(ctx, runtime, request.IDs)
	if err != nil {
		return nil, err
	}
	manifest, err := workspace.ReadManifest(filepath.Join(path, workspace.ManifestFile))
	if err != nil {
		return nil, err
	}
	items := make([]workspace.WorkItem, 0, len(request.IDs))
	if request.SkipWork {
		for _, id := range request.IDs {
			item := workspace.WorkItem{ID: id}
			if request.Type != "" {
				value := request.Type
				item.Type = &value
			}
			if request.Title != "" {
				value := request.Title
				item.Title = &value
			}
			if request.State != "" {
				value := request.State
				item.State = &value
			}
			items = append(items, item)
		}
	} else {
		if s.workItems == nil {
			return nil, workspace.ErrWorkCapabilityRequired
		}
		items, err = s.workItems.LoadWorkspaceItems(ctx, request.Provider, root, manifest.Project, request.IDs)
		if err != nil {
			return nil, err
		}
	}
	original, plan, err := workspace.PlanAddWorkItems(root, path, items)
	if err != nil {
		return nil, err
	}
	result := WorkspaceItemUpdateResult{Plan: plan, operation: actionWorkspaceItemAdd}
	if request.Execute {
		execution, err := workspace.ExecuteWorkItemUpdate(original, plan, "add")
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}
func (s workspaceActions) itemRemove(ctx context.Context, request WorkspaceItemRemoveRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	request.IDs, err = requestIDs(ctx, runtime, request.IDs)
	if err != nil {
		return nil, err
	}
	manifest, plan, err := workspace.PlanRemoveWorkItems(root, path, request.IDs)
	if err != nil {
		return nil, err
	}
	result := WorkspaceItemUpdateResult{Plan: plan, operation: actionWorkspaceItemRemove}
	if request.Execute {
		execution, err := workspace.ExecuteWorkItemUpdate(manifest, plan, "remove")
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}
func requestIDs(ctx context.Context, runtime action.Runtime, ids []string) ([]string, error) {
	if len(ids) != 0 {
		return ids, nil
	}
	response, err := runtime.Ask(ctx, action.Prompt{ID: "work-item-ids", Kind: action.PromptText, Label: l10n.M(promptWorkItemIDs), Required: true})
	if err != nil {
		return nil, err
	}
	ids = split(response.Text)
	if len(ids) == 0 {
		return nil, fmt.Errorf("cli.work-item-ids-required")
	}
	return ids, nil
}

func (s workspaceActions) preflight(_ context.Context, request WorkspacePreflightRequest, _ action.Runtime) (action.Result, error) {
	_, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	report, err := workspace.BuildPreflight(path, request.Files)
	return WorkspacePreflightResult{PreflightReport: report}, err
}
func (s workspaceActions) rename(ctx context.Context, request WorkspaceRenameRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	manifest, plan, err := s.engine.PlanRename(ctx, root, path, request.Slug)
	if err != nil {
		return nil, err
	}
	result := WorkspaceRenameResult{Plan: plan}
	if request.Execute {
		execution, err := s.engine.ExecuteRename(ctx, manifest, plan)
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}
func (s workspaceActions) repoAdd(ctx context.Context, request WorkspaceRepoAddRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(request.Repository) == "" {
		choices, err := s.engine.AddRepositoryChoices(ctx, root, path)
		if err != nil {
			return nil, err
		}
		if len(choices) == 0 {
			return nil, fmt.Errorf("cli.work-repo-add-no-candidates")
		}
		if len(choices) == 1 {
			request.Repository = choices[0]
		} else {
			options := make([]action.Choice, len(choices))
			for index, choice := range choices {
				options[index] = action.Choice{Value: action.ChoiceValue(choice), Label: l10n.M(promptChoiceValue, l10n.A("value", choice))}
			}
			response, err := runtime.Ask(ctx, action.Prompt{ID: "work-repository", Kind: action.PromptSelectOne, Label: l10n.M(promptWorkRepository), Required: true, Choices: options})
			if err != nil {
				return nil, err
			}
			request.Repository = string(response.Value)
		}
	}
	manifest, plan, err := s.engine.PlanAddRepository(ctx, root, path, request.Repository)
	if err != nil {
		return nil, err
	}
	result := WorkspaceRepoAddResult{Plan: plan}
	if request.Execute {
		execution, err := s.engine.ExecuteAddRepository(ctx, manifest, plan)
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}
func (s workspaceActions) repoLatest(ctx context.Context, request WorkspaceRepoLatestRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	plan, err := s.engine.PlanRepositoryLatestReport(ctx, root, path, request.Repositories)
	if err != nil {
		return nil, err
	}
	result := WorkspaceRepoLatestResult{Plan: plan}
	if request.Execute {
		execution, err := s.engine.ExecuteRepositoryLatestReport(ctx, plan)
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}
func (s workspaceActions) commit(ctx context.Context, request WorkspaceCommitRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	plan, err := s.engine.PlanCommit(ctx, root, path, request.Message)
	if err != nil {
		return nil, err
	}
	result := WorkspaceCommitResult{Plan: plan}
	if request.Execute {
		execution, err := s.engine.ExecuteCommit(ctx, plan)
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}
func (s workspaceActions) handoff(_ context.Context, request WorkspaceHandoffRequest, _ action.Runtime) (action.Result, error) {
	_, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	report, err := workspace.ValidateHandoffs(path)
	return WorkspaceHandoffResult{HandoffValidationReport: report}, err
}
func (s workspaceActions) teardown(ctx context.Context, request WorkspaceTeardownRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := s.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	_, plan, err := s.engine.PlanTeardown(ctx, root, path)
	if err != nil {
		return nil, err
	}
	result := WorkspaceTeardownResult{Plan: plan}
	if request.Execute {
		execution, err := s.engine.ExecuteTeardown(ctx, plan, request.Approved)
		if err != nil {
			return nil, err
		}
		result.Execution = &execution
	}
	return result, nil
}

func stateSetRoute() Route {
	return Route{Key: "work.item.state.set", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		built, err := buildWorkItemStateSet(invocation)
		if err != nil {
			return Outcome{}, err
		}
		var request workapp.StatePlanRequest
		switch value := built.(type) {
		case workapp.StatePlanRequest:
			request = value
		case workapp.StateSetRequest:
			request = value.Request
		default:
			return Outcome{}, fmt.Errorf("cli.invalid-request:work.item.state.set:%T", built)
		}
		planEnvelope, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		_, ok := planEnvelope.Result.(workapp.StatePlanReport)
		if !ok {
			return Outcome{}, fmt.Errorf("cli.invalid-result:work.item.state.plan:%T", planEnvelope.Result)
		}
		if !invocation.Values.Bool("json") {
			preview, err := execution.Console.RenderResultKind(console.NewRenderContext(execution.Policy, execution.Localizer), planEnvelope, "work.item.state.plan", console.FormatHuman, nil)
			if err != nil {
				return Outcome{}, err
			}
			if err := console.WriteOutput(execution.Policy.Streams.Stdout, preview); err != nil {
				return Outcome{}, err
			}
		}
		if _, err := confirmExecution(ctx, execution, invocation, true, invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkState); err != nil {
			return Outcome{}, err
		}
		result, err := dispatchDirect(ctx, execution, invocation, workapp.StateSetRequest{Request: request})
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := jsonOptionProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, "work.item.state.set", format, projection)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}

func doingRoute() Route {
	return Route{Key: "work.item.doing", Machine: jsonMachine, Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		root := resolvedRoot(invocation.Values)
		states, _, _ := taskStartSettings(root)
		project := invocation.Values.String("project")
		request := workapp.DoingRequest{Provider: selectedWorkProvider(invocation.Values, root, project), Root: root, Project: project, IDs: split(invocation.Values.String("id")), States: states}
		planEnvelope, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		plan, ok := planEnvelope.Result.(workapp.DoingPlanReport)
		if !ok {
			return Outcome{}, fmt.Errorf("cli.invalid-result:work.item.doing:%T", planEnvelope.Result)
		}
		if !invocation.Values.Bool("json") {
			preview, err := execution.Console.RenderResultKind(console.NewRenderContext(execution.Policy, execution.Localizer), planEnvelope, actionWorkItemDoingRoute, console.FormatHuman, nil)
			if err != nil {
				return Outcome{}, err
			}
			if err := console.WriteOutput(execution.Policy.Streams.Stdout, preview); err != nil {
				return Outcome{}, err
			}
		}
		if _, err := confirmExecution(ctx, execution, invocation, true, invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkDoing); err != nil {
			return Outcome{}, err
		}
		result, err := dispatchDirect(ctx, execution, invocation, workapp.DoingExecuteRequest{Plan: plan})
		if err != nil {
			return Outcome{}, err
		}
		format, projection, err := jsonOptionProject(result, invocation)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format), result, actionWorkItemDoingRoute, format, projection)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}

const actionWorkItemDoingRoute = action.ID("work.item.doing")

func dispatchDirect(ctx context.Context, execution Execution, invocation *parse.Result, request action.Request) (action.ResultEnvelope, error) {
	runtime := action.Runtime{Events: NewEventSink(execution.Console, execution.Policy, execution.Localizer, invocation.Verbosity), Input: NewTerminalInput(execution.Policy.Streams, execution.Localizer)}
	return execution.Dispatcher.Dispatch(ctx, request, runtime)
}
