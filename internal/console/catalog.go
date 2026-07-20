package console

import "github.com/sachahjkl/dw/internal/l10n"

// EnglishEntries are owned beside their renderers. They extend, rather than
// mutate, the shared immutable English gateway.
var EnglishEntries = []l10n.Entry{
	{ID: "console.error", Text: "Error"},
	{ID: "console.error.detail", Text: "{label}: {detail}"},
	{ID: "guide.title", Text: "Dev Workflow {version}"},
	{ID: "guide.subtitle", Text: "Step-by-step getting started guide"},
	{ID: "guide.step.numbered", Text: "{number}. {title}"},
	{ID: "guide.step.installation", Text: "Check the installation"},
	{ID: "guide.step.installation.detail", Text: "Fix reported prerequisites before creating workspaces."},
	{ID: "guide.step.initialize", Text: "Initialize the DevWorkflow root"},
	{ID: "guide.step.initialize.detail", Text: "The root contains config, schemas, cache, projects, workspaces, and agent contexts."},
	{ID: "guide.step.ado", Text: "Connect Azure DevOps"},
	{ID: "guide.step.ado.detail", Text: "Without --project, dw offers configured projects when the terminal is interactive."},
	{ID: "guide.step.workspace", Text: "Create a task workspace"},
	{ID: "guide.step.workspace.detail", Text: "Preview first; execute only after reviewing the branch, repositories, worktrees, and handoffs."},
	{ID: "guide.step.daily", Text: "Daily loop"},
	{ID: "guide.step.daily.detail", Text: "Use preflight before implementation and sync to refresh task.json from ADO."},
	{ID: "guide.step.contents", Text: "Manage workspace contents"},
	{ID: "guide.step.contents.detail", Text: "Interactive commands offer local values when available."},
	{ID: "guide.step.complete", Text: "Prepare task completion"},
	{ID: "guide.step.complete.detail", Text: "Completion commands preview by default. Add --execute only after reading the plan."},
	{ID: "guide.step.cleanup", Text: "Clean up"},
	{ID: "guide.step.cleanup.detail", Text: "Teardown and prune remove only with --execute and ask for confirmation interactively."},
	{ID: "guide.step.tools", Text: "ADO, DB, and AI context"},
	{ID: "guide.step.tools.detail", Text: "Database access is protected by the read-only guard."},
	{ID: "guide.step.completion", Text: "Shell productivity"},
	{ID: "guide.step.completion.detail", Text: "Completions suggest options, projects, repositories, workspaces, databases, and descriptions."},
	{ID: "guide.diagnostics", Text: "Quick diagnostics"},
	{ID: "guide.diagnostics.detail", Text: "Refresh regenerates schemas and agent contexts without overwriting user files."},
	{ID: "changelog.title", Text: "Changelog"},
	{ID: "changelog.title.repository", Text: "Changelog ({repository})"},
	{ID: "changelog.warning.label", Text: "Warning"},
	{ID: "changelog.warning", Text: "Warning: {detail}"},
	{ID: "changelog.column.work-item", Text: "Work Item"},
	{ID: "changelog.column.type", Text: "Type"},
	{ID: "changelog.column.state", Text: "State"},
	{ID: "changelog.column.title", Text: "Title"},
	{ID: "changelog.empty.git", Text: "No work item detected in git range commit messages."},
	{ID: "changelog.empty.pr", Text: "No work item detected for the given pull requests."},
	{ID: "changelog.empty.input", Text: "No work item provided."},
	{ID: "changelog.empty.resolved", Text: "No work item resolved in Azure DevOps."},
	{ID: "db.column.result", Text: "Result"},
	{ID: "db.null", Text: "NULL"},
	{ID: "db.query.title", Text: "DB query"},
	{ID: "db.query.result", Text: "Result"},
	{ID: "db.query.rows", Text: "{count} row(s)"},
	{ID: "db.query.truncated.badge", Text: "TRUNCATED"},
	{ID: "db.query.truncated", Text: "Result truncated; rerun with a larger row limit"},
	{ID: "update.title", Text: "Dev Workflow update"},
	{ID: "update.check.badge", Text: "AVAILABLE"},
	{ID: "update.installed.badge", Text: "INSTALLED"},
	{ID: "update.invalid.badge", Text: "INVALID"},
	{ID: "update.version", Text: "Version"},
	{ID: "update.release", Text: "Release"},
	{ID: "update.commit", Text: "Commit"},
	{ID: "update.executable", Text: "Executable"},
	{ID: "update.assets", Text: "Assets"},
	{ID: "update.rid", Text: "RID"},
	{ID: "update.file", Text: "File"},
	{ID: "update.sha256", Text: "SHA-256"},
	{ID: "update.replacement", Text: "Replacement is scheduled after dw exits."},
	{ID: "result.title", Text: "Dev Workflow result"},
	{ID: "result.action", Text: "Action"},
	{ID: "result.root", Text: "Root"},
	{ID: "result.profile", Text: "Profile"},
	{ID: "result.paths", Text: "Paths"},
	{ID: "result.mode", Text: "Mode"},
	{ID: "result.key", Text: "Key"},
	{ID: "result.storage", Text: "Storage"},
	{ID: "result.masked", Text: "Value masked"},
	{ID: "result.exists", Text: "Exists"},
	{ID: "result.deleted", Text: "Deleted"},
	{ID: "result.items", Text: "Items"},
	{ID: "result.warnings", Text: "Warnings"},
	{ID: "result.workspaces", Text: "Workspaces"},
	{ID: "result.files", Text: "Files"},
	{ID: "result.saved", Text: "Saved"},
	{ID: "result.removed", Text: "Removed"},
	{ID: "result.project", Text: "Project"},
	{ID: "result.groups", Text: "Groups"},
	{ID: "result.repositories", Text: "Repositories"},
	{ID: "result.expanded", Text: "Expanded"},
	{ID: "result.state", Text: "State"},
	{ID: "result.updated", Text: "Updated"},
	{ID: "result.updates", Text: "Updates"},
	{ID: "result.executed", Text: "Executed"},
	{ID: "result.pull-request", Text: "Pull request"},
	{ID: "result.workspace", Text: "Workspace"},
	{ID: "result.repository", Text: "Repository"},
	{ID: "result.item", Text: "Work item"},
	{ID: "result.settings", Text: "Settings"},
	{ID: "result.workflow", Text: "Workflow"},
	{ID: "result.projects", Text: "Projects"},
	{ID: "result.databases", Text: "Databases"},
	{ID: "result.path", Text: "Path"},
	{ID: "result.status", Text: "Status"},
	{ID: "result.detail", Text: "Detail"},
	{ID: "result.passed", Text: "Passed"},
	{ID: "result.failed", Text: "Failed"},
	{ID: "result.check", Text: "Check"},
	{ID: "result.remediation", Text: "Remediation"},
	{ID: "result.agent", Text: "Agent"},
	{ID: "result.command", Text: "Command"},
	{ID: "result.references", Text: "References"},
	{ID: "result.allowed", Text: "Allowed"},
	{ID: "result.reason", Text: "Reason"},
	{ID: "result.environment-pat", Text: "Environment PAT"},
	{ID: "result.source", Text: "Source"},
	{ID: "result.expires", Text: "Expires"},
	{ID: "result.connected", Text: "Connected"},
	{ID: "work.event.authenticating", Text: "Authenticating with Azure DevOps..."},
	{ID: "work.event.device-login-required", Text: "Azure DevOps device sign-in is required."},
	{ID: "work.event.loading-assigned-work-items", Text: "Loading assigned work items..."},
	{ID: "work.event.grouping-assigned-work-items", Text: "Grouping assigned work items..."},
	{ID: "work.event.loading-pull-requests", Text: "Loading pull requests..."},
	{ID: "work.event.resolving-pull-request-work-items", Text: "Resolving pull request work items..."},
	{ID: "work.event.extracting-git-work-items", Text: "Extracting work items from git history..."},
	{ID: "work.event.loading-work-item", Text: "Loading work item..."},
	{ID: "work.event.loading-work-items", Text: "Loading work items..."},
	{ID: "work.event.loading-work-item-context", Text: "Loading work item context..."},
	{ID: "work.event.loading-changelog", Text: "Loading changelog..."},
	{ID: "work.event.loading-changelog-items", Text: "Loading changelog items..."},
	{ID: "work.event.updating-work-item-state", Text: "Updating work item state..."},
	{ID: "work.event.updated-work-item-state", Text: "Work item state updated."},
}

func WithConsoleMessages(localizer l10n.Localizer) l10n.Localizer {
	if localizer == nil {
		return NewEnglishLocalizer()
	}
	catalog, ok := localizer.(*l10n.Catalog)
	if !ok {
		return localizer
	}
	missing := make([]l10n.Entry, 0, len(EnglishEntries))
	for _, entry := range EnglishEntries {
		if !catalog.Has(entry.ID) {
			missing = append(missing, entry)
		}
	}
	if len(missing) == 0 {
		return catalog
	}
	extended, err := catalog.Extend(missing...)
	if err != nil {
		return catalog
	}
	return extended
}

func NewEnglishLocalizer() l10n.Localizer {
	catalog, err := l10n.NewEnglish().Extend(EnglishEntries...)
	if err != nil {
		panic("console.invalid-english-catalog:" + err.Error())
	}
	return catalog
}
