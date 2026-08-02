package workapp

import (
	"context"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

func (s *Service) runAuthLogin(ctx context.Context, request AuthLoginRequest, runtime action.Runtime) (AuthLoginReport, error) {
	if request.Mode == "" {
		choices := []action.Choice{
			{Value: action.ChoiceValue(AuthLoginBrowser), Label: l10n.M("cli.prompt.auth-browser")},
			{Value: action.ChoiceValue(AuthLoginDeviceCode), Label: l10n.M("cli.prompt.auth-device")},
			{Value: action.ChoiceValue(AuthLoginEnvironmentPAT), Label: l10n.M("cli.prompt.auth-pat")},
		}
		response, err := runtime.Ask(ctx, action.SelectOnePrompt{Meta: action.PromptMeta{ID: "provider-auth-login-mode", Label: l10n.M("cli.prompt.auth-mode")}, Required: true, Choices: choices})
		if err != nil {
			return AuthLoginReport{}, err
		}
		request.Mode = AuthLoginMode(response.(action.SelectOneResponse).Value)
	}
	return s.AuthLogin(ctx, request, eventSink(ActionProviderAuthLogin, runtime))
}

func (s *Service) runStateSet(ctx context.Context, request StateSetRequest, runtime action.Runtime) (StateSetResult, error) {
	plan, err := PlanState(request.Request)
	result := StateSetResult{Plan: plan}
	if err != nil {
		return result, err
	}
	if !request.Approved {
		accepted, err := askConfirm(ctx, runtime, "confirm:work.item.state.set", "cli.confirm.work-state")
		if err != nil {
			return result, err
		}
		if !accepted {
			return result, problem("work.execution-canceled", "work item state update canceled")
		}
	}
	executed, err := s.ExecuteState(ctx, plan, eventSink(ActionWorkItemStateSet, runtime))
	result.Execution = executed
	return result, err
}

func (s *Service) runDoingAction(ctx context.Context, request DoingActionRequest, runtime action.Runtime) (DoingActionResult, error) {
	plan, err := s.DoingPlan(ctx, request.Request)
	result := DoingActionResult{Plan: plan}
	if err != nil {
		return result, err
	}
	if !request.Approved {
		accepted, err := askConfirm(ctx, runtime, "confirm:work.item.doing", "cli.confirm.work-doing")
		if err != nil {
			return result, err
		}
		if !accepted {
			return result, problem("work.execution-canceled", "work item update canceled")
		}
	}
	executed, err := s.DoingExecute(ctx, plan, eventSink(ActionWorkItemDoing, runtime))
	result.Execution = &executed
	return result, err
}

func (s *Service) runStartAction(ctx context.Context, request StartRequest, runtime action.Runtime) (StartResult, error) {
	resolved, err := s.resolveStartInput(ctx, request, runtime)
	if err != nil {
		return StartResult{}, err
	}
	execute := resolved.Execute
	resolved.Execute = false
	plan, _, err := s.Start(ctx, resolved, eventSink(ActionWorkspaceStart, runtime))
	result := StartResult{Plan: plan}
	if err != nil {
		return result, err
	}
	if resolved.PromptToExecute && !execute {
		execute, err = askConfirm(ctx, runtime, "workspace-start-create", "cli.prompt.start-create")
		if err != nil {
			return result, err
		}
	}
	if !execute {
		return result, nil
	}
	if !resolved.Approved && !resolved.PromptToExecute {
		accepted, confirmErr := askConfirm(ctx, runtime, "workspace-start-create", "cli.prompt.start-create")
		if confirmErr != nil {
			return result, confirmErr
		}
		if !accepted {
			return result, problem("work.execution-canceled", "workspace creation canceled")
		}
	}
	resolved.Execute = true
	plan, executed, err := s.Start(ctx, resolved, eventSink(ActionWorkspaceStart, runtime))
	result.Plan, result.Execution = plan, executed
	if err != nil || executed == nil || !resolved.PromptToOpen {
		return result, err
	}
	open, err := askConfirm(ctx, runtime, "workspace-start-open", "cli.prompt.start-open")
	if err != nil {
		return result, err
	}
	if !open {
		return result, nil
	}
	opened, err := s.Open(ctx, OpenRequest{Provider: resolved.Provider, Root: resolved.Root, Project: resolved.Project, Workspace: &executed.Plan.Workspace}, eventSink(ActionWorkspaceStart, runtime))
	result.Open = &opened
	return result, err
}

func (s *Service) runPruneAction(ctx context.Context, request PruneRequest, runtime action.Runtime) (PruneReport, error) {
	execute := request.Execute
	request.Execute = false
	report, err := s.Prune(ctx, request, eventSink(ActionWorkspacePrune, runtime))
	if err != nil || !execute {
		return report, err
	}
	selected := make([]string, 0, len(report.Plan.Candidates))
	if request.Approved {
		for _, candidate := range report.Plan.Candidates {
			selected = append(selected, candidate.Path)
		}
	} else if len(report.Plan.Candidates) != 0 {
		choices := make([]action.Choice, len(report.Plan.Candidates))
		for index, candidate := range report.Plan.Candidates {
			choices[index] = action.Choice{Value: action.ChoiceValue(candidate.Path), Label: l10n.M("cli.prompt.choice-value", l10n.A("value", candidate.Path))}
		}
		response, askErr := runtime.Ask(ctx, action.SelectManyPrompt{Meta: action.PromptMeta{ID: "workspace-prune-candidates", Label: l10n.M("cli.confirm.workspace-prune")}, Choices: choices})
		if askErr != nil {
			return report, askErr
		}
		for _, value := range response.(action.SelectManyResponse).Values {
			selected = append(selected, string(value))
		}
	}
	if len(selected) == 0 {
		return report, nil
	}
	request.Execute = true
	request.SelectedWorkspaces = selected
	return s.Prune(ctx, request, eventSink(ActionWorkspacePrune, runtime))
}

func (s *Service) runFinishAction(ctx context.Context, request FinishRequest, runtime action.Runtime) (FinishReport, error) {
	execute := request.Execute
	if execute && !request.Approved && !request.CreatePR && !request.Ready && !request.SkipWork {
		choices := []action.Choice{
			{Value: "push-only", Label: l10n.M("cli.prompt.finish-push")},
			{Value: "draft-pr", Label: l10n.M("cli.prompt.finish-draft")},
			{Value: "ready-pr", Label: l10n.M("cli.prompt.finish-ready")},
			{Value: "keep", Label: l10n.M("cli.prompt.finish-keep")},
		}
		response, err := runtime.Ask(ctx, action.SelectOnePrompt{Meta: action.PromptMeta{ID: "finish-mode", Label: l10n.M("cli.prompt.finish-mode")}, Required: true, Choices: choices})
		if err != nil {
			return FinishReport{}, err
		}
		switch response.(action.SelectOneResponse).Value {
		case "push-only":
			request.SkipWork = true
		case "draft-pr":
			request.CreatePR = true
		case "ready-pr":
			request.CreatePR, request.Ready = true, true
		}
	}
	request.Execute = false
	report, err := s.Finish(ctx, request, eventSink(ActionWorkspaceFinish, runtime))
	if err != nil || !execute {
		return report, err
	}
	if !request.Approved {
		accepted, confirmErr := askConfirm(ctx, runtime, "confirm:workspace.finish", "cli.confirm.workspace-finish")
		if confirmErr != nil {
			return report, confirmErr
		}
		if !accepted {
			return report, problem("work.execution-canceled", "workspace finish canceled")
		}
	}
	request.Execute = true
	return s.Finish(ctx, request, eventSink(ActionWorkspaceFinish, runtime))
}

func askConfirm(ctx context.Context, runtime action.Runtime, id action.PromptID, label l10n.ID) (bool, error) {
	response, err := runtime.Ask(ctx, action.ConfirmPrompt{Meta: action.PromptMeta{ID: id, Label: l10n.M(label)}, Default: false})
	if err != nil {
		return false, err
	}
	return response.(action.ConfirmResponse).Accepted, nil
}
