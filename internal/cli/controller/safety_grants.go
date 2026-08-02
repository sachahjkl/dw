package controller

import "github.com/sachahjkl/dw/internal/l10n"

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
	errorProjectRequired  l10n.ID = "cli.error.work-item-project-required"
	errorNoProjects       l10n.ID = "cli.error.work-item-no-projects"
)

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
	{ID: errorProjectRequired, Text: "Project selection is required in non-interactive mode.\nAvailable projects: {projects}\nNext: dw work item list --project <project>"},
	{ID: errorNoProjects, Text: "No projects are configured.\nNext: add a project to config/projects.json, then rerun dw work item list."},
	{ID: promptFinishMode, Text: "Finish mode"},
	{ID: promptFinishPush, Text: "Push only, no provider updates"},
	{ID: promptFinishDraft, Text: "Push + draft provider pull request"},
	{ID: promptFinishReady, Text: "Push + ready provider pull request"},
	{ID: promptFinishKeep, Text: "Keep current flags"},
	{ID: promptStartCreate, Text: "Create this workspace now?"},
	{ID: promptStartOpen, Text: "Open the created workspace now?"},
}
