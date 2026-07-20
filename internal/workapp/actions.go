package workapp

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	ActionAuthLogin        action.ID = "ado.auth.login"
	ActionAuthStatus       action.ID = "ado.auth.status"
	ActionAuthLogout       action.ID = "ado.auth.logout"
	ActionAssigned         action.ID = "ado.assigned"
	ActionPullRequests     action.ID = "ado.prs"
	ActionChangelog        action.ID = "ado.changelog"
	ActionContext          action.ID = "ado.context"
	ActionAIContext        action.ID = "ado.ai.context"
	ActionItemShow         action.ID = "ado.workitem"
	ActionStatePlan        action.ID = "ado.state.plan"
	ActionStateExecute     action.ID = "ado.state.execute"
	ActionStateSet         action.ID = "ado.state.set"
	ActionDoingPlan        action.ID = "task.doing.plan"
	ActionDoingExecute     action.ID = "task.doing.execute"
	ActionStart            action.ID = "task.start"
	ActionStartPullRequest action.ID = "task.start.pr"
	ActionOpen             action.ID = "task.open"
	ActionSync             action.ID = "task.sync"
	ActionChild            action.ID = "task.child.create"
	ActionPrune            action.ID = "task.prune"
	ActionFinish           action.ID = "task.finish"
)

func (AuthLoginRequest) ActionID() action.ID    { return ActionAuthLogin }
func (AuthStatusRequest) ActionID() action.ID   { return ActionAuthStatus }
func (AuthLogoutRequest) ActionID() action.ID   { return ActionAuthLogout }
func (AssignedRequest) ActionID() action.ID     { return ActionAssigned }
func (PullRequestsRequest) ActionID() action.ID { return ActionPullRequests }
func (ChangelogRequest) ActionID() action.ID    { return ActionChangelog }
func (ContextRequest) ActionID() action.ID      { return ActionContext }

type AIContextRequest struct{ ContextRequest }

func (AIContextRequest) ActionID() action.ID { return ActionAIContext }
func (ItemShowRequest) ActionID() action.ID  { return ActionItemShow }
func (StatePlanRequest) ActionID() action.ID { return ActionStatePlan }

type StateExecuteRequest struct{ Plan StatePlanReport }

func (StateExecuteRequest) ActionID() action.ID { return ActionStateExecute }

type StateSetRequest struct{ Request StatePlanRequest }

func (StateSetRequest) ActionID() action.ID { return ActionStateSet }
func (DoingRequest) ActionID() action.ID    { return ActionDoingPlan }

type DoingExecuteRequest struct{ Plan DoingPlanReport }

func (DoingExecuteRequest) ActionID() action.ID     { return ActionDoingExecute }
func (StartRequest) ActionID() action.ID            { return ActionStart }
func (StartPullRequestRequest) ActionID() action.ID { return ActionStartPullRequest }
func (OpenRequest) ActionID() action.ID             { return ActionOpen }
func (SyncRequest) ActionID() action.ID             { return ActionSync }
func (ChildRequest) ActionID() action.ID            { return ActionChild }
func (PruneRequest) ActionID() action.ID            { return ActionPrune }
func (FinishRequest) ActionID() action.ID           { return ActionFinish }

func (AuthLoginReport) ActionID() action.ID    { return ActionAuthLogin }
func (AuthStatusReport) ActionID() action.ID   { return ActionAuthStatus }
func (AuthLogoutReport) ActionID() action.ID   { return ActionAuthLogout }
func (AssignedReport) ActionID() action.ID     { return ActionAssigned }
func (PullRequestsReport) ActionID() action.ID { return ActionPullRequests }
func (ChangelogReport) ActionID() action.ID    { return ActionChangelog }
func (ContextReport) ActionID() action.ID      { return ActionContext }

type AIContextResult struct{ ContextReport }

func (AIContextResult) ActionID() action.ID      { return ActionAIContext }
func (ItemShowReport) ActionID() action.ID       { return ActionItemShow }
func (StatePlanReport) ActionID() action.ID      { return ActionStatePlan }
func (StateExecutionReport) ActionID() action.ID { return ActionStateExecute }

type StateSetResult struct{ StateExecutionReport }

func (StateSetResult) ActionID() action.ID       { return ActionStateSet }
func (DoingPlanReport) ActionID() action.ID      { return ActionDoingPlan }
func (DoingExecutionReport) ActionID() action.ID { return ActionDoingExecute }

type StartResult struct {
	Plan      StartPlanReport       `json:"plan"`
	Execution *StartExecutionReport `json:"execution,omitempty"`
}

func (StartResult) ActionID() action.ID { return ActionStart }

type StartPullRequestResult struct {
	Plan      StartPullRequestPlanReport `json:"plan"`
	Execution *StartExecutionReport      `json:"execution,omitempty"`
}

func (StartPullRequestResult) ActionID() action.ID { return ActionStartPullRequest }
func (OpenReport) ActionID() action.ID             { return ActionOpen }
func (SyncReport) ActionID() action.ID             { return ActionSync }
func (ChildReport) ActionID() action.ID            { return ActionChild }
func (PruneReport) ActionID() action.ID            { return ActionPrune }
func (FinishReport) ActionID() action.ID           { return ActionFinish }

func Handlers(service *Service) []action.Handler {
	return []action.Handler{
		handler[AuthLoginRequest](ActionAuthLogin, func(ctx context.Context, r AuthLoginRequest, rt action.Runtime) (action.Result, error) {
			return service.AuthLogin(ctx, r, eventSink(ActionAuthLogin, rt))
		}),
		handler[AuthStatusRequest](ActionAuthStatus, func(ctx context.Context, r AuthStatusRequest, _ action.Runtime) (action.Result, error) {
			return service.AuthStatus(ctx, r)
		}),
		handler[AuthLogoutRequest](ActionAuthLogout, func(ctx context.Context, r AuthLogoutRequest, _ action.Runtime) (action.Result, error) {
			return service.AuthLogout(ctx, r)
		}),
		handler[AssignedRequest](ActionAssigned, func(ctx context.Context, r AssignedRequest, rt action.Runtime) (action.Result, error) {
			resolved, err := service.resolveAssignedInput(ctx, r, rt)
			if err != nil {
				return nil, err
			}
			return service.Assigned(ctx, resolved, eventSink(ActionAssigned, rt))
		}),
		handler[PullRequestsRequest](ActionPullRequests, func(ctx context.Context, r PullRequestsRequest, rt action.Runtime) (action.Result, error) {
			return service.PullRequests(ctx, r, eventSink(ActionPullRequests, rt))
		}),
		handler[ChangelogRequest](ActionChangelog, func(ctx context.Context, r ChangelogRequest, rt action.Runtime) (action.Result, error) {
			return service.Changelog(ctx, r, eventSink(ActionChangelog, rt))
		}),
		handler[ContextRequest](ActionContext, func(ctx context.Context, r ContextRequest, rt action.Runtime) (action.Result, error) {
			return service.Context(ctx, r, eventSink(ActionContext, rt))
		}),
		handler[AIContextRequest](ActionAIContext, func(ctx context.Context, r AIContextRequest, rt action.Runtime) (action.Result, error) {
			r.Mode = ContextRich
			value, err := service.Context(ctx, r.ContextRequest, eventSink(ActionAIContext, rt))
			return AIContextResult{value}, err
		}),
		handler[ItemShowRequest](ActionItemShow, func(ctx context.Context, r ItemShowRequest, rt action.Runtime) (action.Result, error) {
			return service.ItemShow(ctx, r, eventSink(ActionItemShow, rt))
		}),
		handler[StatePlanRequest](ActionStatePlan, func(_ context.Context, r StatePlanRequest, _ action.Runtime) (action.Result, error) {
			return PlanState(r)
		}),
		handler[StateExecuteRequest](ActionStateExecute, func(ctx context.Context, r StateExecuteRequest, rt action.Runtime) (action.Result, error) {
			return service.ExecuteState(ctx, r.Plan, eventSink(ActionStateExecute, rt))
		}),
		handler[StateSetRequest](ActionStateSet, func(ctx context.Context, r StateSetRequest, rt action.Runtime) (action.Result, error) {
			plan, err := PlanState(r.Request)
			if err != nil {
				return nil, err
			}
			value, err := service.ExecuteState(ctx, plan, eventSink(ActionStateSet, rt))
			return StateSetResult{value}, err
		}),
		handler[DoingRequest](ActionDoingPlan, func(ctx context.Context, r DoingRequest, _ action.Runtime) (action.Result, error) {
			return service.DoingPlan(ctx, r)
		}),
		handler[DoingExecuteRequest](ActionDoingExecute, func(ctx context.Context, r DoingExecuteRequest, rt action.Runtime) (action.Result, error) {
			return service.DoingExecute(ctx, r.Plan, eventSink(ActionDoingExecute, rt))
		}),
		handler[StartRequest](ActionStart, func(ctx context.Context, r StartRequest, rt action.Runtime) (action.Result, error) {
			resolved, err := service.resolveStartInput(ctx, r, rt)
			if err != nil {
				return nil, err
			}
			plan, execution, err := service.Start(ctx, resolved, eventSink(ActionStart, rt))
			return StartResult{Plan: plan, Execution: execution}, err
		}),
		handler[StartPullRequestRequest](ActionStartPullRequest, func(ctx context.Context, r StartPullRequestRequest, rt action.Runtime) (action.Result, error) {
			plan, execution, err := service.StartPullRequest(ctx, r, eventSink(ActionStartPullRequest, rt))
			return StartPullRequestResult{Plan: plan, Execution: execution}, err
		}),
		handler[OpenRequest](ActionOpen, func(ctx context.Context, r OpenRequest, rt action.Runtime) (action.Result, error) {
			return service.Open(ctx, r, eventSink(ActionOpen, rt))
		}),
		handler[SyncRequest](ActionSync, func(ctx context.Context, r SyncRequest, rt action.Runtime) (action.Result, error) {
			return service.Sync(ctx, r, eventSink(ActionSync, rt))
		}),
		handler[ChildRequest](ActionChild, func(ctx context.Context, r ChildRequest, rt action.Runtime) (action.Result, error) {
			return service.CreateChild(ctx, r, eventSink(ActionChild, rt))
		}),
		handler[PruneRequest](ActionPrune, func(ctx context.Context, r PruneRequest, rt action.Runtime) (action.Result, error) {
			return service.Prune(ctx, r, eventSink(ActionPrune, rt))
		}),
		handler[FinishRequest](ActionFinish, func(ctx context.Context, r FinishRequest, rt action.Runtime) (action.Result, error) {
			return service.Finish(ctx, r, eventSink(ActionFinish, rt))
		}),
	}
}

func (s *Service) resolveAssignedInput(ctx context.Context, request AssignedRequest, runtime action.Runtime) (AssignedRequest, error) {
	if request.Project != "" {
		return request, nil
	}
	if s.Choices == nil {
		return request, projectRequired("ado assigned")
	}
	choices, err := s.Choices.ProjectChoices(ctx, request.Root)
	if err != nil {
		return request, err
	}
	if len(choices) == 0 {
		return request, projectRequired("ado assigned")
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
		return request, projectRequired("ado assigned")
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
		assigned, err := s.Assigned(ctx, AssignedRequest{Provider: request.Provider, Root: request.Root, Project: request.Project, Top: 50}, eventSink(ActionStart, runtime))
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
			return nil, fmt.Errorf("workapp.invalid-request:%s:%T", id, request)
		}
		return execute(ctx, typed, runtime)
	}}
}
func eventSink(id action.ID, runtime action.Runtime) EventSink {
	return func(ctx context.Context, event Event) error {
		return runtime.Emit(ctx, action.EventEnvelope{Action: id, Kind: action.EventProgress, Message: l10n.M(l10n.ID("work.event." + event.Kind)), Data: event})
	}
}
