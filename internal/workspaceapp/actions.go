package workspaceapp

import (
	"context"
	"fmt"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workspace"
)

const (
	ActionStatus     action.ID = "workspace.status"
	ActionList       action.ID = "workspace.list"
	ActionCurrent    action.ID = "workspace.current"
	ActionItemAdd    action.ID = "workspace.item.add"
	ActionItemRemove action.ID = "workspace.item.remove"
	ActionPreflight  action.ID = "workspace.preflight"
	ActionRename     action.ID = "workspace.rename"
	ActionRepoAdd    action.ID = "workspace.repo.add"
	ActionRepoLatest action.ID = "workspace.repo.latest"
	ActionCommit     action.ID = "workspace.commit"
	ActionHandoff    action.ID = "workspace.handoff.validate"
	ActionTeardown   action.ID = "workspace.teardown"
)

const (
	promptWorkspaceRemove l10n.ID = "cli.confirm.workspace-remove"
	promptWorkRepository  l10n.ID = "cli.prompt.work-repository"
	promptChoiceValue     l10n.ID = "cli.prompt.choice-value"
	promptWorkItemIDs     l10n.ID = "cli.prompt.work-item-ids"
)

type Selection struct {
	Root      string   `json:"root,omitempty"`
	Workspace *string  `json:"workspace,omitempty"`
	Project   string   `json:"project,omitempty"`
	IDs       []string `json:"workItemIds,omitempty"`
	Continue  bool     `json:"continue"`
}

type StatusRequest struct {
	Root string `json:"root,omitempty"`
}
type ListRequest struct {
	Root        string   `json:"root,omitempty"`
	Project     *string  `json:"project,omitempty"`
	WorkItemIDs []string `json:"workItemIds"`
}
type CurrentRequest struct{}
type ItemAddRequest struct {
	Selection Selection `json:"selection"`
	IDs       []string  `json:"ids"`
	Provider  string    `json:"provider,omitempty"`
	SkipWork  bool      `json:"skipWork"`
	Type      string    `json:"type"`
	Title     string    `json:"title"`
	State     string    `json:"state"`
	Execute   bool      `json:"execute"`
}
type ItemRemoveRequest struct {
	Selection Selection `json:"selection"`
	IDs       []string  `json:"ids"`
	Execute   bool      `json:"execute"`
}
type PreflightRequest struct {
	Selection Selection `json:"selection"`
	Files     []string  `json:"files"`
}
type RenameRequest struct {
	Selection Selection `json:"selection"`
	Slug      string    `json:"slug"`
	Execute   bool      `json:"execute"`
}
type RepoAddRequest struct {
	Selection  Selection `json:"selection"`
	Repository string    `json:"repository"`
	Execute    bool      `json:"execute"`
}
type RepoLatestRequest struct {
	Selection    Selection `json:"selection"`
	Repositories []string  `json:"repositories"`
	Execute      bool      `json:"execute"`
}
type CommitRequest struct {
	Selection Selection `json:"selection"`
	Message   string    `json:"message,omitempty"`
	Execute   bool      `json:"execute"`
}
type HandoffRequest struct {
	Selection Selection `json:"selection"`
}
type TeardownRequest struct {
	Selection Selection `json:"selection"`
	Execute   bool      `json:"execute"`
	Approved  bool      `json:"approved"`
}

func (StatusRequest) ActionID() action.ID     { return ActionStatus }
func (ListRequest) ActionID() action.ID       { return ActionList }
func (CurrentRequest) ActionID() action.ID    { return ActionCurrent }
func (ItemAddRequest) ActionID() action.ID    { return ActionItemAdd }
func (ItemRemoveRequest) ActionID() action.ID { return ActionItemRemove }
func (PreflightRequest) ActionID() action.ID  { return ActionPreflight }
func (RenameRequest) ActionID() action.ID     { return ActionRename }
func (RepoAddRequest) ActionID() action.ID    { return ActionRepoAdd }
func (RepoLatestRequest) ActionID() action.ID { return ActionRepoLatest }
func (CommitRequest) ActionID() action.ID     { return ActionCommit }
func (HandoffRequest) ActionID() action.ID    { return ActionHandoff }
func (TeardownRequest) ActionID() action.ID   { return ActionTeardown }

type StatusResult struct{ workspace.StatusReport }
type ListResult struct{ workspace.ListReport }
type CurrentResult struct{ workspace.CurrentItem }
type ItemUpdateResult struct {
	Plan      workspace.WorkItemUpdatePlan    `json:"plan"`
	Execution *workspace.WorkItemUpdateReport `json:"execution,omitempty"`
	Operation action.ID                       `json:"operation"`
}
type PreflightResult struct{ workspace.PreflightReport }
type RenameResult struct {
	Plan      workspace.RenamePlan             `json:"plan"`
	Execution *workspace.RenameExecutionReport `json:"execution,omitempty"`
}
type RepoAddResult struct {
	Plan      workspace.AddRepositoryPlan    `json:"plan"`
	Execution *workspace.AddRepositoryReport `json:"execution,omitempty"`
}
type RepoLatestResult struct {
	Plan      workspace.RepositoryLatestPlanReport       `json:"plan"`
	Execution *workspace.RepositoryLatestExecutionReport `json:"execution,omitempty"`
}
type CommitResult struct {
	Plan      workspace.CommitPlanReport       `json:"plan"`
	Execution *workspace.CommitExecutionReport `json:"execution,omitempty"`
}
type HandoffResult struct {
	workspace.HandoffValidationReport
}
type TeardownResult struct {
	Plan      workspace.TeardownPlanReport       `json:"plan"`
	Execution *workspace.TeardownExecutionReport `json:"execution,omitempty"`
}

func (StatusResult) ActionID() action.ID            { return ActionStatus }
func (ListResult) ActionID() action.ID              { return ActionList }
func (CurrentResult) ActionID() action.ID           { return ActionCurrent }
func (result ItemUpdateResult) ActionID() action.ID { return result.Operation }
func (PreflightResult) ActionID() action.ID         { return ActionPreflight }
func (RenameResult) ActionID() action.ID            { return ActionRename }
func (RepoAddResult) ActionID() action.ID           { return ActionRepoAdd }
func (RepoLatestResult) ActionID() action.ID        { return ActionRepoLatest }
func (CommitResult) ActionID() action.ID            { return ActionCommit }
func (HandoffResult) ActionID() action.ID           { return ActionHandoff }
func (TeardownResult) ActionID() action.ID          { return ActionTeardown }

type WorkItemLoader interface {
	LoadWorkspaceItems(context.Context, string, string, string, []string) ([]workspace.WorkItem, error)
}

type Service struct {
	engine           *workspace.Engine
	workItems        WorkItemLoader
	currentDirectory string
}

func Handlers(engine *workspace.Engine, workItems WorkItemLoader, currentDirectory string) []action.Handler {
	service := Service{engine: engine, workItems: workItems, currentDirectory: currentDirectory}
	return []action.Handler{
		handler[StatusRequest](ActionStatus, service.status),
		handler[ListRequest](ActionList, service.list),
		handler[CurrentRequest](ActionCurrent, service.current),
		handler[ItemAddRequest](ActionItemAdd, service.itemAdd),
		handler[ItemRemoveRequest](ActionItemRemove, service.itemRemove),
		handler[PreflightRequest](ActionPreflight, service.preflight),
		handler[RenameRequest](ActionRename, service.rename),
		handler[RepoAddRequest](ActionRepoAdd, service.repoAdd),
		handler[RepoLatestRequest](ActionRepoLatest, service.repoLatest),
		handler[CommitRequest](ActionCommit, service.commit),
		handler[HandoffRequest](ActionHandoff, service.handoff),
		handler[TeardownRequest](ActionTeardown, service.teardown),
	}
}

func handler[T action.Request](id action.ID, execute func(context.Context, T, action.Runtime) (action.Result, error)) action.Handler {
	return action.HandlerFunc{Action: id, ExecuteFunc: func(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
		typed, ok := request.(T)
		if !ok {
			return nil, fmt.Errorf("workspaceapp.invalid-action-request:%s:%T", id, request)
		}
		return execute(ctx, typed, runtime)
	}}
}

func (service Service) require() error {
	if service.engine == nil {
		return fmt.Errorf("workspaceapp.nil-workspace-engine")
	}
	return nil
}

func (service Service) root(value string) string { return config.ResolveRoot(value) }

func (service Service) resolve(selection Selection) (string, string, error) {
	if err := service.require(); err != nil {
		return "", "", err
	}
	root := service.root(selection.Root)
	explicit := ""
	if selection.Workspace != nil {
		explicit = *selection.Workspace
	}
	path, err := workspace.Resolve(root, explicit, selection.Project, selection.IDs, selection.Continue, service.currentDirectory)
	return root, path, err
}

func (service Service) status(_ context.Context, request StatusRequest, _ action.Runtime) (action.Result, error) {
	root := service.root(request.Root)
	return StatusResult{StatusReport: workspace.BuildStatusReport(root)}, nil
}

func (service Service) list(_ context.Context, request ListRequest, _ action.Runtime) (action.Result, error) {
	root := service.root(request.Root)
	return ListResult{ListReport: workspace.BuildListReport(root, request.Project, request.WorkItemIDs)}, nil
}

func (service Service) current(_ context.Context, _ CurrentRequest, _ action.Runtime) (action.Result, error) {
	report, err := workspace.Current(service.currentDirectory)
	return CurrentResult{CurrentItem: report}, err
}

func (service Service) itemAdd(ctx context.Context, request ItemAddRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
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
		if service.workItems == nil {
			return nil, workspace.ErrWorkCapabilityRequired
		}
		items, err = service.workItems.LoadWorkspaceItems(ctx, request.Provider, root, manifest.Project, request.IDs)
		if err != nil {
			return nil, err
		}
	}
	original, plan, err := workspace.PlanAddWorkItems(root, path, items)
	if err != nil {
		return nil, err
	}
	result := ItemUpdateResult{Plan: plan, Operation: ActionItemAdd}
	if request.Execute {
		execution, executeErr := workspace.ExecuteWorkItemUpdate(original, plan, "add")
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func (service Service) itemRemove(ctx context.Context, request ItemRemoveRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
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
	result := ItemUpdateResult{Plan: plan, Operation: ActionItemRemove}
	if request.Execute {
		execution, executeErr := workspace.ExecuteWorkItemUpdate(manifest, plan, "remove")
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func requestIDs(ctx context.Context, runtime action.Runtime, ids []string) ([]string, error) {
	if len(ids) != 0 {
		return ids, nil
	}
	response, err := runtime.Ask(ctx, action.TextPrompt{Meta: action.PromptMeta{ID: "work-item-ids", Label: l10n.M(promptWorkItemIDs)}, Required: true})
	if err != nil {
		return nil, err
	}
	ids = split(response.(action.TextResponse).Value)
	if len(ids) == 0 {
		return nil, fmt.Errorf("workspaceapp.work-item-ids-required")
	}
	return ids, nil
}

func (service Service) preflight(_ context.Context, request PreflightRequest, _ action.Runtime) (action.Result, error) {
	_, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	report, err := workspace.BuildPreflight(path, request.Files)
	return PreflightResult{PreflightReport: report}, err
}

func (service Service) rename(ctx context.Context, request RenameRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	manifest, plan, err := service.engine.PlanRename(ctx, root, path, request.Slug)
	if err != nil {
		return nil, err
	}
	result := RenameResult{Plan: plan}
	if request.Execute {
		execution, executeErr := service.engine.ExecuteRename(ctx, manifest, plan)
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func (service Service) repoAdd(ctx context.Context, request RepoAddRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(request.Repository) == "" {
		choices, choicesErr := service.engine.AddRepositoryChoices(ctx, root, path)
		if choicesErr != nil {
			return nil, choicesErr
		}
		if len(choices) == 0 {
			return nil, fmt.Errorf("workspaceapp.work-repo-add-no-candidates")
		}
		if len(choices) == 1 {
			request.Repository = choices[0]
		} else {
			options := make([]action.Choice, len(choices))
			for index, choice := range choices {
				options[index] = action.Choice{Value: action.ChoiceValue(choice), Label: l10n.M(promptChoiceValue, l10n.A("value", choice))}
			}
			response, askErr := runtime.Ask(ctx, action.SelectOnePrompt{Meta: action.PromptMeta{ID: "work-repository", Label: l10n.M(promptWorkRepository)}, Required: true, Choices: options})
			if askErr != nil {
				return nil, askErr
			}
			request.Repository = string(response.(action.SelectOneResponse).Value)
		}
	}
	manifest, plan, err := service.engine.PlanAddRepository(ctx, root, path, request.Repository)
	if err != nil {
		return nil, err
	}
	result := RepoAddResult{Plan: plan}
	if request.Execute {
		execution, executeErr := service.engine.ExecuteAddRepository(ctx, manifest, plan)
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func (service Service) repoLatest(ctx context.Context, request RepoLatestRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	plan, err := service.engine.PlanRepositoryLatestReport(ctx, root, path, request.Repositories)
	if err != nil {
		return nil, err
	}
	result := RepoLatestResult{Plan: plan}
	if request.Execute {
		execution, executeErr := service.engine.ExecuteRepositoryLatestReport(ctx, plan)
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func (service Service) commit(ctx context.Context, request CommitRequest, _ action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	plan, err := service.engine.PlanCommit(ctx, root, path, request.Message)
	if err != nil {
		return nil, err
	}
	result := CommitResult{Plan: plan}
	if request.Execute {
		execution, executeErr := service.engine.ExecuteCommit(ctx, plan)
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func (service Service) handoff(_ context.Context, request HandoffRequest, _ action.Runtime) (action.Result, error) {
	_, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	report, err := workspace.ValidateHandoffs(path)
	return HandoffResult{HandoffValidationReport: report}, err
}

func (service Service) teardown(ctx context.Context, request TeardownRequest, runtime action.Runtime) (action.Result, error) {
	root, path, err := service.resolve(request.Selection)
	if err != nil {
		return nil, err
	}
	_, plan, err := service.engine.PlanTeardown(ctx, root, path)
	if err != nil {
		return nil, err
	}
	result := TeardownResult{Plan: plan}
	if request.Execute {
		if !request.Approved {
			response, askErr := runtime.Ask(ctx, action.ConfirmPrompt{Meta: action.PromptMeta{ID: "confirm:workspace.teardown", Label: l10n.M(promptWorkspaceRemove)}, Default: false})
			if askErr != nil {
				return result, askErr
			}
			if !response.(action.ConfirmResponse).Accepted {
				return result, context.Canceled
			}
		}
		execution, executeErr := service.engine.ExecuteTeardown(ctx, plan, true)
		if executeErr != nil {
			return result, executeErr
		}
		result.Execution = &execution
	}
	return result, nil
}

func split(value string) []string {
	fields := strings.FieldsFunc(value, func(character rune) bool { return character == ',' || character == ';' })
	result := make([]string, 0, len(fields))
	for _, field := range fields {
		if value := strings.TrimSpace(field); value != "" {
			result = append(result, value)
		}
	}
	return result
}
