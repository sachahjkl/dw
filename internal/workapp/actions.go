package workapp

import (
	"context"
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	ActionProviderAuthLogin         action.ID = "provider.auth.login"
	ActionProviderAuthStatus        action.ID = "provider.auth.status"
	ActionProviderAuthLogout        action.ID = "provider.auth.logout"
	ActionWorkItemList              action.ID = "work.item.list"
	ActionWorkPullRequestList       action.ID = "work.pr.list"
	ActionWorkChangelog             action.ID = "work.changelog"
	ActionWorkContextShow           action.ID = "work.context.show"
	ActionWorkContextAI             action.ID = "work.context.ai"
	ActionWorkItemShow              action.ID = "work.item.show"
	ActionWorkItemStatePlan         action.ID = "work.item.state.plan"
	ActionWorkItemStateExecute      action.ID = "work.item.state.execute"
	ActionWorkItemStateSet          action.ID = "work.item.state.set"
	ActionWorkItemDoingPlan         action.ID = "work.item.doing.plan"
	ActionWorkItemDoingExecute      action.ID = "work.item.doing.execute"
	ActionWorkspaceStart            action.ID = "workspace.start"
	ActionWorkspacePullRequestStart action.ID = "workspace.pr.start"
	ActionWorkspaceOpen             action.ID = "workspace.open"
	ActionWorkspaceSync             action.ID = "workspace.sync"
	ActionWorkItemChildCreate       action.ID = "work.item.child.create"
	ActionWorkspacePrune            action.ID = "workspace.prune"
	ActionWorkspaceFinish           action.ID = "workspace.finish"
)

func (AuthLoginRequest) ActionID() action.ID    { return ActionProviderAuthLogin }
func (AuthStatusRequest) ActionID() action.ID   { return ActionProviderAuthStatus }
func (AuthLogoutRequest) ActionID() action.ID   { return ActionProviderAuthLogout }
func (AssignedRequest) ActionID() action.ID     { return ActionWorkItemList }
func (PullRequestsRequest) ActionID() action.ID { return ActionWorkPullRequestList }
func (ChangelogRequest) ActionID() action.ID    { return ActionWorkChangelog }
func (ContextRequest) ActionID() action.ID      { return ActionWorkContextShow }

type AIContextRequest struct{ ContextRequest }

func (AIContextRequest) ActionID() action.ID { return ActionWorkContextAI }
func (ItemShowRequest) ActionID() action.ID  { return ActionWorkItemShow }
func (StatePlanRequest) ActionID() action.ID { return ActionWorkItemStatePlan }

type StateExecuteRequest struct{ Plan StatePlanReport }

func (StateExecuteRequest) ActionID() action.ID { return ActionWorkItemStateExecute }

type StateSetRequest struct{ Request StatePlanRequest }

func (StateSetRequest) ActionID() action.ID { return ActionWorkItemStateSet }
func (DoingRequest) ActionID() action.ID    { return ActionWorkItemDoingPlan }

type DoingExecuteRequest struct{ Plan DoingPlanReport }

func (DoingExecuteRequest) ActionID() action.ID     { return ActionWorkItemDoingExecute }
func (StartRequest) ActionID() action.ID            { return ActionWorkspaceStart }
func (StartPullRequestRequest) ActionID() action.ID { return ActionWorkspacePullRequestStart }
func (OpenRequest) ActionID() action.ID             { return ActionWorkspaceOpen }
func (SyncRequest) ActionID() action.ID             { return ActionWorkspaceSync }
func (ChildRequest) ActionID() action.ID            { return ActionWorkItemChildCreate }
func (PruneRequest) ActionID() action.ID            { return ActionWorkspacePrune }
func (FinishRequest) ActionID() action.ID           { return ActionWorkspaceFinish }

func (AuthLoginReport) ActionID() action.ID    { return ActionProviderAuthLogin }
func (AuthStatusReport) ActionID() action.ID   { return ActionProviderAuthStatus }
func (AuthLogoutReport) ActionID() action.ID   { return ActionProviderAuthLogout }
func (AssignedReport) ActionID() action.ID     { return ActionWorkItemList }
func (PullRequestsReport) ActionID() action.ID { return ActionWorkPullRequestList }
func (ChangelogReport) ActionID() action.ID    { return ActionWorkChangelog }
func (ContextReport) ActionID() action.ID      { return ActionWorkContextShow }

type AIContextResult struct{ ContextReport }

func (AIContextResult) ActionID() action.ID      { return ActionWorkContextAI }
func (ItemShowReport) ActionID() action.ID       { return ActionWorkItemShow }
func (StatePlanReport) ActionID() action.ID      { return ActionWorkItemStatePlan }
func (StateExecutionReport) ActionID() action.ID { return ActionWorkItemStateExecute }

type StateSetResult struct{ StateExecutionReport }

func (StateSetResult) ActionID() action.ID       { return ActionWorkItemStateSet }
func (DoingPlanReport) ActionID() action.ID      { return ActionWorkItemDoingPlan }
func (DoingExecutionReport) ActionID() action.ID { return ActionWorkItemDoingExecute }

type StartResult struct {
	Plan      StartPlanReport       `json:"plan"`
	Execution *StartExecutionReport `json:"execution,omitempty"`
}

func (StartResult) ActionID() action.ID { return ActionWorkspaceStart }

type StartPullRequestResult struct {
	Plan      StartPullRequestPlanReport `json:"plan"`
	Execution *StartExecutionReport      `json:"execution,omitempty"`
}

func (StartPullRequestResult) ActionID() action.ID { return ActionWorkspacePullRequestStart }
func (OpenReport) ActionID() action.ID             { return ActionWorkspaceOpen }
func (SyncReport) ActionID() action.ID             { return ActionWorkspaceSync }
func (ChildReport) ActionID() action.ID            { return ActionWorkItemChildCreate }
func (PruneReport) ActionID() action.ID            { return ActionWorkspacePrune }
func (FinishReport) ActionID() action.ID           { return ActionWorkspaceFinish }

func Handlers(service *Service) []action.Handler {
	return []action.Handler{
		handler[AuthLoginRequest](ActionProviderAuthLogin, func(ctx context.Context, r AuthLoginRequest, rt action.Runtime) (action.Result, error) {
			return service.AuthLogin(ctx, r, eventSink(ActionProviderAuthLogin, rt))
		}),
		handler[AuthStatusRequest](ActionProviderAuthStatus, func(ctx context.Context, r AuthStatusRequest, _ action.Runtime) (action.Result, error) {
			return service.AuthStatus(ctx, r)
		}),
		handler[AuthLogoutRequest](ActionProviderAuthLogout, func(ctx context.Context, r AuthLogoutRequest, _ action.Runtime) (action.Result, error) {
			return service.AuthLogout(ctx, r)
		}),
		handler[AssignedRequest](ActionWorkItemList, func(ctx context.Context, r AssignedRequest, rt action.Runtime) (action.Result, error) {
			resolved, err := service.resolveAssignedInput(ctx, r, rt)
			if err != nil {
				return nil, err
			}
			return service.Assigned(ctx, resolved, eventSink(ActionWorkItemList, rt))
		}),
		handler[PullRequestsRequest](ActionWorkPullRequestList, func(ctx context.Context, r PullRequestsRequest, rt action.Runtime) (action.Result, error) {
			return service.PullRequests(ctx, r, eventSink(ActionWorkPullRequestList, rt))
		}),
		handler[ChangelogRequest](ActionWorkChangelog, func(ctx context.Context, r ChangelogRequest, rt action.Runtime) (action.Result, error) {
			return service.Changelog(ctx, r, eventSink(ActionWorkChangelog, rt))
		}),
		handler[ContextRequest](ActionWorkContextShow, func(ctx context.Context, r ContextRequest, rt action.Runtime) (action.Result, error) {
			return service.Context(ctx, r, eventSink(ActionWorkContextShow, rt))
		}),
		handler[AIContextRequest](ActionWorkContextAI, func(ctx context.Context, r AIContextRequest, rt action.Runtime) (action.Result, error) {
			r.Mode = ContextRich
			value, err := service.Context(ctx, r.ContextRequest, eventSink(ActionWorkContextAI, rt))
			return AIContextResult{value}, err
		}),
		handler[ItemShowRequest](ActionWorkItemShow, func(ctx context.Context, r ItemShowRequest, rt action.Runtime) (action.Result, error) {
			return service.ItemShow(ctx, r, eventSink(ActionWorkItemShow, rt))
		}),
		handler[StatePlanRequest](ActionWorkItemStatePlan, func(_ context.Context, r StatePlanRequest, _ action.Runtime) (action.Result, error) {
			return PlanState(r)
		}),
		handler[StateExecuteRequest](ActionWorkItemStateExecute, func(ctx context.Context, r StateExecuteRequest, rt action.Runtime) (action.Result, error) {
			return service.ExecuteState(ctx, r.Plan, eventSink(ActionWorkItemStateExecute, rt))
		}),
		handler[StateSetRequest](ActionWorkItemStateSet, func(ctx context.Context, r StateSetRequest, rt action.Runtime) (action.Result, error) {
			plan, err := PlanState(r.Request)
			if err != nil {
				return nil, err
			}
			value, err := service.ExecuteState(ctx, plan, eventSink(ActionWorkItemStateSet, rt))
			return StateSetResult{value}, err
		}),
		handler[DoingRequest](ActionWorkItemDoingPlan, func(ctx context.Context, r DoingRequest, _ action.Runtime) (action.Result, error) {
			return service.DoingPlan(ctx, r)
		}),
		handler[DoingExecuteRequest](ActionWorkItemDoingExecute, func(ctx context.Context, r DoingExecuteRequest, rt action.Runtime) (action.Result, error) {
			return service.DoingExecute(ctx, r.Plan, eventSink(ActionWorkItemDoingExecute, rt))
		}),
		handler[StartRequest](ActionWorkspaceStart, func(ctx context.Context, r StartRequest, rt action.Runtime) (action.Result, error) {
			resolved, err := service.resolveStartInput(ctx, r, rt)
			if err != nil {
				return nil, err
			}
			plan, execution, err := service.Start(ctx, resolved, eventSink(ActionWorkspaceStart, rt))
			return StartResult{Plan: plan, Execution: execution}, err
		}),
		handler[StartPullRequestRequest](ActionWorkspacePullRequestStart, func(ctx context.Context, r StartPullRequestRequest, rt action.Runtime) (action.Result, error) {
			plan, execution, err := service.StartPullRequest(ctx, r, eventSink(ActionWorkspacePullRequestStart, rt))
			return StartPullRequestResult{Plan: plan, Execution: execution}, err
		}),
		handler[OpenRequest](ActionWorkspaceOpen, func(ctx context.Context, r OpenRequest, rt action.Runtime) (action.Result, error) {
			return service.Open(ctx, r, eventSink(ActionWorkspaceOpen, rt))
		}),
		handler[SyncRequest](ActionWorkspaceSync, func(ctx context.Context, r SyncRequest, rt action.Runtime) (action.Result, error) {
			return service.Sync(ctx, r, eventSink(ActionWorkspaceSync, rt))
		}),
		handler[ChildRequest](ActionWorkItemChildCreate, func(ctx context.Context, r ChildRequest, rt action.Runtime) (action.Result, error) {
			return service.CreateChild(ctx, r, eventSink(ActionWorkItemChildCreate, rt))
		}),
		handler[PruneRequest](ActionWorkspacePrune, func(ctx context.Context, r PruneRequest, rt action.Runtime) (action.Result, error) {
			return service.Prune(ctx, r, eventSink(ActionWorkspacePrune, rt))
		}),
		handler[FinishRequest](ActionWorkspaceFinish, func(ctx context.Context, r FinishRequest, rt action.Runtime) (action.Result, error) {
			return service.Finish(ctx, r, eventSink(ActionWorkspaceFinish, rt))
		}),
	}
}

func (s *Service) resolveAssignedInput(ctx context.Context, request AssignedRequest, runtime action.Runtime) (AssignedRequest, error) {
	if request.Project != "" {
		return request, nil
	}
	if s.Choices == nil {
		return request, projectRequired("work item list")
	}
	choices, err := s.Choices.ProjectChoices(ctx, request.Root)
	if err != nil {
		return request, err
	}
	if len(choices) == 0 {
		return request, projectRequired("work item list")
	}
	values := make([]action.Choice, len(choices))
	for index, choice := range choices {
		values[index] = action.Choice{Value: action.ChoiceValue(choice.Value), Label: choice.Label}
	}
	response, err := runtime.Ask(ctx, action.Prompt{ID: "project", Kind: action.PromptSelectOne, Label: l10n.M("prompt.project"), Required: true, Choices: values})
	if err != nil {
		return request, err
	}
	request.Project = string(response.Value)
	if request.Project == "" {
		return request, projectRequired("work item list")
	}
	return request, nil
}

func (s *Service) resolveStartInput(ctx context.Context, request StartRequest, runtime action.Runtime) (StartRequest, error) {
	if len(request.WorkItemIDs) > 0 {
		return request, nil
	}
	if request.Project == "" && s.Choices != nil {
		choices, err := s.Choices.ProjectChoices(ctx, request.Root)
		if err != nil {
			return request, err
		}
		if len(choices) > 0 {
			values := make([]action.Choice, len(choices))
			for index, choice := range choices {
				values[index] = action.Choice{Value: action.ChoiceValue(choice.Value), Label: choice.Label}
			}
			response, err := runtime.Ask(ctx, action.Prompt{ID: "project", Kind: action.PromptSelectOne, Label: l10n.M("prompt.project"), Required: true, Choices: values})
			if err != nil {
				return request, err
			}
			request.Project = string(response.Value)
		}
	}
	if len(request.Repositories) == 0 && request.Project != "" && s.Choices != nil {
		choices, err := s.Choices.RepositoryChoices(ctx, request.Root, request.Project)
		if err != nil {
			return request, err
		}
		if len(choices) > 1 {
			values := make([]action.Choice, len(choices))
			for index, choice := range choices {
				values[index] = action.Choice{Value: action.ChoiceValue(choice.Value), Label: choice.Label}
			}
			response, err := runtime.Ask(ctx, action.Prompt{ID: "repositories", Kind: action.PromptSelectMany, Label: l10n.M("prompt.repositories"), Choices: values})
			if err != nil {
				return request, err
			}
			for _, value := range response.Values {
				request.Repositories = append(request.Repositories, string(value))
			}
		}
	}
	manual := request.SkipWork || request.Project == ""
	if !manual {
		assigned, err := s.Assigned(ctx, AssignedRequest{Provider: request.Provider, Root: request.Root, Project: request.Project, Top: 50}, eventSink(ActionWorkspaceStart, runtime))
		if err != nil {
			return request, err
		}
		choices := make([]action.Choice, 0, len(assigned.Items)+1)
		for _, item := range assigned.Items {
			label := "#" + item.ID
			if item.Type != nil {
				label += " [" + *item.Type + "]"
			}
			if item.State != nil {
				label += " (" + *item.State + ")"
			}
			if item.Title != nil {
				label += " " + *item.Title
			}
			choices = append(choices, action.Choice{Value: action.ChoiceValue(item.ID), Label: l10n.M("prompt.choice.value", l10n.A("value", label))})
		}
		choices = append(choices, action.Choice{Value: "__manual_work_item_id__", Label: l10n.M("prompt.work-item.manual")})
		response, err := runtime.Ask(ctx, action.Prompt{ID: "assigned-work-item", Kind: action.PromptSelectOne, Label: l10n.M("prompt.work-item"), Required: true, Choices: choices})
		if err != nil {
			return request, err
		}
		if response.Value != "__manual_work_item_id__" {
			request.WorkItemIDs = []string{string(response.Value)}
			for _, item := range assigned.Items {
				if item.ID == string(response.Value) && request.Slug == "" && item.Title != nil {
					request.Slug = *item.Title
					break
				}
			}
			return request, nil
		}
		manual = true
	}
	if manual {
		response, err := runtime.Ask(ctx, action.Prompt{ID: "work-item-id", Kind: action.PromptText, Label: l10n.M("prompt.work-item-id"), Required: true})
		if err != nil {
			return request, err
		}
		if response.Text == "" {
			return request, ErrWorkItemsRequired
		}
		request.WorkItemIDs = []string{response.Text}
	}
	return request, nil
}

func handler[T action.Request](id action.ID, execute func(context.Context, T, action.Runtime) (action.Result, error)) action.Handler {
	return action.HandlerFunc{Action: id, ExecuteFunc: func(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
		typed, ok := request.(T)
		if !ok {
			domain, _, _ := strings.Cut(id.String(), ".")
			return nil, fmt.Errorf("%s.invalid-request:%s:%T", domain, id, request)
		}
		return execute(ctx, typed, runtime)
	}}
}
func eventSink(id action.ID, runtime action.Runtime) EventSink {
	return func(ctx context.Context, event Event) error {
		return runtime.Emit(ctx, action.EventEnvelope{Action: id, Kind: action.EventProgress, Message: l10n.M(l10n.ID("work.event." + event.Kind)), Data: event})
	}
}
