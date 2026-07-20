package controller

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	promptWorkState       l10n.ID = "cli.confirm.work-state"
	promptWorkDoing       l10n.ID = "cli.confirm.work-doing"
	promptWorkspaceFinish l10n.ID = "cli.confirm.workspace-finish"
	promptWorkspaceRemove l10n.ID = "cli.confirm.workspace-remove"
	promptWorkspacePrune  l10n.ID = "cli.confirm.workspace-prune"
	promptWorkRepository  l10n.ID = "cli.prompt.work-repository"
	promptChoiceValue     l10n.ID = "cli.prompt.choice-value"
	promptWorkItemIDs     l10n.ID = "cli.prompt.work-item-ids"
	promptProject         l10n.ID = "cli.prompt.project"
	promptAuthMode        l10n.ID = "cli.prompt.auth-mode"
	promptAuthBrowser     l10n.ID = "cli.prompt.auth-browser"
	promptAuthDevice      l10n.ID = "cli.prompt.auth-device"
	promptAuthPAT         l10n.ID = "cli.prompt.auth-pat"
)

// SafetyEnglishEntries is composed into the CLI catalog by bootstrap so action
// prompts remain localized presentation rather than hard-coded terminal text.
var SafetyEnglishEntries = []l10n.Entry{
	{ID: promptWorkState, Text: "Apply the provider work item state change?"},
	{ID: promptWorkDoing, Text: "Move the selected work items to their in-progress state?"},
	{ID: promptWorkspaceFinish, Text: "Execute finish operations, including commits, pushes, pull requests, and work-item updates?"},
	{ID: promptWorkspaceRemove, Text: "Remove this workspace and its Git worktrees?"},
	{ID: promptWorkspacePrune, Text: "Remove every selected finished workspace and its Git worktrees?"},
	{ID: promptWorkRepository, Text: "Select the repository to add"},
	{ID: promptChoiceValue, Text: "{value}"},
	{ID: promptWorkItemIDs, Text: "Enter work item IDs, separated by commas"},
	{ID: promptProject, Text: "Select a project"},
	{ID: promptAuthMode, Text: "Provider connection mode"},
	{ID: promptAuthBrowser, Text: "Browser"},
	{ID: promptAuthDevice, Text: "Device code"},
	{ID: promptAuthPAT, Text: "Environment credential"},
	{ID: promptFinishMode, Text: "Finish mode"},
	{ID: promptFinishPush, Text: "Push only, no provider updates"},
	{ID: promptFinishDraft, Text: "Push + draft provider pull request"},
	{ID: promptFinishReady, Text: "Push + ready provider pull request"},
	{ID: promptFinishKeep, Text: "Keep current flags"},
	{ID: promptStartCreate, Text: "Create this workspace now?"},
	{ID: promptStartOpen, Text: "Open the created workspace now?"},
}

// SafetyGrant runs after parsing and policy selection, before request building
// or direct execution. A nil grant means the route is read-only or preview-only.
type SafetyGrant func(context.Context, Execution, *parse.Result) error

func GrantWorkState(ctx context.Context, execution Execution, invocation *parse.Result) error {
	_, err := confirmExecution(ctx, execution, invocation, true, invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkState)
	return err
}

func GrantWorkDoing(ctx context.Context, execution Execution, invocation *parse.Result) error {
	_, err := confirmExecution(ctx, execution, invocation, true, invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkDoing)
	return err
}

func GrantWorkspaceFinish(ctx context.Context, execution Execution, invocation *parse.Result) error {
	_, err := confirmExecution(ctx, execution, invocation, invocation.Values.Bool("execute"), invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkspaceFinish)
	return err
}

func GrantWorkspaceTeardown(ctx context.Context, execution Execution, invocation *parse.Result) error {
	_, err := confirmExecution(ctx, execution, invocation, invocation.Values.Bool("execute"), invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkspaceRemove)
	return err
}

func GrantWorkspacePrune(ctx context.Context, execution Execution, invocation *parse.Result) error {
	_, err := confirmExecution(ctx, execution, invocation, invocation.Values.Bool("execute"), invocation.Values.Bool("yes"), invocation.Values.Bool("json"), promptWorkspacePrune)
	return err
}

func confirmExecution(ctx context.Context, execution Execution, invocation *parse.Result, execute, approved, machine bool, promptID l10n.ID) (bool, error) {
	if !execute {
		return false, nil
	}
	if approved {
		return true, nil
	}
	if machine {
		return false, console.WithExitCode(fmt.Errorf("cli.confirmation-required:%s", invocation.Command.Key), console.ExitUsage)
	}
	if !execution.Policy.Interactive() {
		return false, console.WithExitCode(fmt.Errorf("cli.confirmation-required:%s:use--yes", invocation.Command.Key), console.ExitUsage)
	}
	defaultValue := action.ChoiceValue("false")
	response, err := NewTerminalInput(execution.Policy.Streams, execution.Localizer).Request(ctx, action.Prompt{
		ID: action.PromptID("confirm:" + invocation.Command.Key), Kind: action.PromptConfirm,
		Label: l10n.M(promptID), Default: &defaultValue,
	})
	if err != nil {
		return false, err
	}
	if !response.Accepted {
		return false, fmt.Errorf("cli.execution-canceled:%s", invocation.Command.Key)
	}
	return true, nil
}
