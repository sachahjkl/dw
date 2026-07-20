package tui

import "github.com/sachahjkl/dw/internal/l10n"

const (
	msgAppTitle           l10n.ID = "tui.app.title"
	msgReady              l10n.ID = "tui.ready"
	msgDashboard          l10n.ID = "tui.view.dashboard"
	msgWorkspaces         l10n.ID = "tui.view.workspaces"
	msgADO                l10n.ID = "tui.view.ado"
	msgPRs                l10n.ID = "tui.view.pull-requests"
	msgDB                 l10n.ID = "tui.view.databases"
	msgComposer           l10n.ID = "tui.view.composer"
	msgMenu               l10n.ID = "tui.modal.menu"
	msgHelp               l10n.ID = "tui.modal.help"
	msgState              l10n.ID = "tui.modal.state"
	msgJournal            l10n.ID = "tui.modal.journal"
	msgDetail             l10n.ID = "tui.modal.detail"
	msgProgress           l10n.ID = "tui.modal.progress"
	msgConfirm            l10n.ID = "tui.modal.confirm"
	msgExternalConfirm    l10n.ID = "tui.modal.external-confirm"
	msgDestructiveConfirm l10n.ID = "tui.modal.destructive-confirm"
	msgActionInput        l10n.ID = "tui.modal.action-input"
	msgForms              l10n.ID = "tui.forms.title"
)

var tuiEnglishEntries = []l10n.Entry{
	{ID: msgAppTitle, Text: "DevWorkflow cockpit"},
	{ID: msgReady, Text: "Ready"},
	{ID: msgDashboard, Text: "Dashboard"},
	{ID: msgWorkspaces, Text: "Workspaces"},
	{ID: msgADO, Text: "ADO"},
	{ID: msgPRs, Text: "PRs"},
	{ID: msgDB, Text: "DB"},
	{ID: msgComposer, Text: "Composer"},
	{ID: msgMenu, Text: "Menu"},
	{ID: msgHelp, Text: "Help and accessibility"},
	{ID: msgState, Text: "State and messages"},
	{ID: msgJournal, Text: "Journal"},
	{ID: msgDetail, Text: "Operation detail"},
	{ID: msgProgress, Text: "Creating workspace"},
	{ID: msgConfirm, Text: "Confirmation"},
	{ID: msgExternalConfirm, Text: "Open external process"},
	{ID: msgDestructiveConfirm, Text: "Destructive confirmation"},
	{ID: msgActionInput, Text: "Action input"},
	{ID: msgForms, Text: "Action composer"},
	{ID: "tui.status.loading", Text: "loading"},
	{ID: "tui.status.waiting", Text: "waiting"},
	{ID: "tui.status.ready", Text: "ready"},
	{ID: "tui.status.running", Text: "running"},
	{ID: "tui.status.ok", Text: "ok"},
	{ID: "tui.status.error", Text: "error"},
	{ID: "tui.status.none", Text: "none"},
	{ID: "tui.status.queue", Text: "queue {count}"},
	{ID: "tui.status.root", Text: "Root: {root}"},
	{ID: "tui.status.snapshot", Text: "Snapshot"},
	{ID: "tui.status.assigned", Text: "My work items"},
	{ID: "tui.status.prs", Text: "Pull requests"},
	{ID: "tui.status.action", Text: "Action"},
	{ID: "tui.status.elapsed", Text: "{label}: {state} · {elapsed}"},
	{ID: "tui.status.count", Text: "{label}: {count}"},
	{ID: "tui.status.next", Text: "Next: {label}"},
	{ID: "tui.status.then", Text: "Then: {count} other action(s)"},
	{ID: "tui.message.loading", Text: "Loading snapshot, work items and pull requests in the background."},
	{ID: "tui.message.loaded", Text: "{label} loaded: {count} item(s)."},
	{ID: "tui.message.load-failed", Text: "{label} failed: {error}"},
	{ID: "tui.message.reload", Text: "Reloading data in the background."},
	{ID: "tui.message.queued", Text: "Action queued #{position}: {label}"},
	{ID: "tui.message.started", Text: "Background launch: {label}"},
	{ID: "tui.message.done", Text: "Done: {label} · {status}"},
	{ID: "tui.message.failed", Text: "Failed: {label} · {error}"},
	{ID: "tui.message.canceled", Text: "Action canceled."},
	{ID: "tui.message.confirmation", Text: "Confirmation required: {label}"},
	{ID: "tui.message.input", Text: "Input required: {label}"},
	{ID: "tui.message.input-sent", Text: "Input sent."},
	{ID: "tui.message.input-canceled", Text: "Input canceled."},
	{ID: "tui.message.unavailable", Text: "This operation is unavailable for the selected row."},
	{ID: "tui.message.no-selection", Text: "No operation is selected."},
	{ID: "tui.message.form-invalid", Text: "Complete or correct the highlighted fields before running."},
	{ID: "tui.message.suggestion", Text: "Suggestion applied: {value}"},
	{ID: "tui.message.no-suggestion", Text: "No suggestion is available for this field."},
	{ID: "tui.message.external-finished", Text: "External process finished: {label}"},
	{ID: "tui.message.external-failed", Text: "External process failed: {label} · {error}"},
	{ID: "tui.panel.readiness", Text: "Readiness"},
	{ID: "tui.panel.cockpit", Text: "Cockpit · Enter runs the primary operation"},
	{ID: "tui.panel.recent", Text: "Recent workspaces"},
	{ID: "tui.panel.selection", Text: "Selection"},
	{ID: "tui.panel.operations", Text: "Available operations"},
	{ID: "tui.panel.preview", Text: "Preview"},
	{ID: "tui.panel.messages", Text: "Messages"},
	{ID: "tui.panel.loads", Text: "Loads"},
	{ID: "tui.panel.queue", Text: "Action queue"},
	{ID: "tui.empty", Text: "No data is available."},
	{ID: "tui.empty.workspaces", Text: "No task workspace detected."},
	{ID: "tui.empty.ado", Text: "No assigned work item outside final states."},
	{ID: "tui.empty.prs", Text: "No active pull request is available."},
	{ID: "tui.empty.db", Text: "No database is configured."},
	{ID: "tui.empty.actions", Text: "No operation is available for this filter."},
	{ID: "tui.column.section", Text: "Section"},
	{ID: "tui.column.subject", Text: "Subject"},
	{ID: "tui.column.status", Text: "Status"},
	{ID: "tui.column.operation", Text: "Operation"},
	{ID: "tui.column.context", Text: "Context"},
	{ID: "tui.column.project", Text: "Project"},
	{ID: "tui.column.work-items", Text: "Work items"},
	{ID: "tui.column.type", Text: "Type"},
	{ID: "tui.column.slug", Text: "Slug"},
	{ID: "tui.column.repositories", Text: "Repositories"},
	{ID: "tui.column.id", Text: "ID"},
	{ID: "tui.column.state", Text: "State"},
	{ID: "tui.column.title", Text: "Title"},
	{ID: "tui.column.workspace", Text: "Workspace"},
	{ID: "tui.column.branch", Text: "Branch"},
	{ID: "tui.column.database", Text: "Database"},
	{ID: "tui.column.scope", Text: "Scope"},
	{ID: "tui.column.field", Text: "Field"},
	{ID: "tui.column.value", Text: "Value"},
	{ID: "tui.column.help", Text: "Help"},
	{ID: "tui.label.projects", Text: "Projects"},
	{ID: "tui.label.repositories", Text: "Repositories"},
	{ID: "tui.label.workspaces", Text: "Workspaces"},
	{ID: "tui.label.work-items", Text: "Work items"},
	{ID: "tui.label.pull-requests", Text: "Active PRs"},
	{ID: "tui.label.cleanup", Text: "Cleanup"},
	{ID: "tui.label.databases", Text: "Databases"},
	{ID: "tui.label.agent", Text: "Agent"},
	{ID: "tui.label.color", Text: "Color"},
	{ID: "tui.label.operation", Text: "Operation"},
	{ID: "tui.label.effect", Text: "Effect"},
	{ID: "tui.label.risk", Text: "Risk"},
	{ID: "tui.label.prompt", Text: "Prompt"},
	{ID: "tui.label.default", Text: "Default"},
	{ID: "tui.label.yes", Text: "yes"},
	{ID: "tui.label.no", Text: "no"},
	{ID: "tui.label.global", Text: "global"},
	{ID: "tui.risk.safe", Text: "read or inspect"},
	{ID: "tui.risk.preview", Text: "preview; no expected modification"},
	{ID: "tui.risk.external", Text: "opens an external or interactive process"},
	{ID: "tui.risk.destructive", Text: "modifies or deletes local or remote state"},
	{ID: "tui.confirm.safe", Text: "This operation reads or inspects data."},
	{ID: "tui.confirm.preview", Text: "Review the returned plan before choosing an execute action."},
	{ID: "tui.confirm.external", Text: "The terminal will be restored while the external process runs and recaptured after it exits."},
	{ID: "tui.confirm.destructive", Text: "Review the operation and effect before confirming."},
	{ID: "tui.help.navigation", Text: "Navigation"},
	{ID: "tui.help.actions", Text: "Actions"},
	{ID: "tui.help.accessibility", Text: "Accessibility and terminal behavior"},
	{ID: "tui.help.nav-lines", Text: "Tab / Right: next view    Shift-Tab / Left: previous view    1-6: select view\nj / Down: next row    k / Up: previous row    J/K: workspace or ADO project\nMouse wheel scrolls only; clicks, motion and releases are ignored."},
	{ID: "tui.help.action-lines", Text: "Enter: run selected operation    n: open composer    m: menu    ?: help\nr: generation-safe reload    /: filter operations    q / Esc: quit when no modal is open\nUppercase actions are distinct: F executes finish, E opens state form, N opens PR form."},
	{ID: "tui.help.accessibility-lines", Text: "All functions are keyboard reachable and every active view ends with a key legend.\nFocus is shown with both a marker and color; status never depends on color alone.\nKey releases are ignored. Key repeats are accepted for navigation and editing.\nSecret input is masked. External processes suspend and restore terminal modes.\nCtrl+C is always an emergency exit, including blocking progress."},
	{ID: "tui.keys.global", Text: "[Tab/Shift-Tab] views  [1-6] jump  [m] menu  [?] help  [r] reload  [q] quit"},
	{ID: "tui.keys.dashboard", Text: "[j/k] select  [Enter] decide"},
	{ID: "tui.keys.workspaces", Text: "[o] open  [p] check  [s] sync  [l] latest  [v] handoff  [c] commit  [f/F] finish  [t/x] remove"},
	{ID: "tui.keys.ado", Text: "[J/K or [/]] project  [j/k] item  [n/x] prepare/create  [e/E] state  [c] context  [w] card  [o] agent  [u] URL"},
	{ID: "tui.keys.prs", Text: "[n/x] prepare/create  [N] form  [f/F] finish  [c] changes  [d] diff  [o] agent  [u] URL"},
	{ID: "tui.keys.db", Text: "[Enter/s] schema  [d] describe  [e] query"},
	{ID: "tui.keys.composer", Text: "[j/k] select  [Enter] edit/run  [Tab/Shift-Tab] field  [Ctrl+Space] suggest  [Space] toggle  [Esc] flows"},
	{ID: "tui.keys.modal", Text: "[Esc] close  [j/k] scroll  [Home/End] top/bottom"},
	{ID: "tui.keys.journal", Text: "[Esc/h] close  [f] fullscreen  [e/w/i/d/o] levels  [a] all  [j/k] scroll  [[/]] run"},
	{ID: "tui.keys.confirm", Text: "[Enter/y] confirm  [Esc/n] cancel"},
	{ID: "tui.keys.input", Text: "[Enter] submit  [Esc] cancel  [j/k] select  [Space] toggle"},
	{ID: "tui.keys.progress", Text: "Please wait until the action finishes. [Ctrl+C] force quit"},
	{ID: "tui.filter", Text: "Search: {value}"},
	{ID: "tui.filter.empty", Text: "Search: /"},
	{ID: "tui.history.run", Text: "Run {current}/{total} · {label} · {status}"},
	{ID: "tui.history.waiting", Text: "Waiting for the first event."},
	{ID: "tui.history.empty", Text: "No operation in the journal."},
	{ID: "tui.history.hidden", Text: "… {count} previous lines hidden …"},
	{ID: "tui.history.levels", Text: "Levels: {levels}"},
	{ID: "tui.progress.live", Text: "Live progress"},
	{ID: "tui.progress.waiting", Text: "Waiting for action output."},
	{ID: "tui.init.title", Text: "Initialize DevWorkflow"},
	{ID: "tui.init.body", Text: "This root is not initialized. The TUI is locked until configuration, schemas, cache and project directories exist."},
	{ID: "tui.init.keys", Text: "[Enter/i] initialize  [q] quit"},
	{ID: "tui.small", Text: "Terminal too small. Resize to at least 60 × 16; all commands remain available after resize."},
	{ID: "tui.form.task-start", Text: "Create workspace"},
	{ID: "tui.form.task-start.desc", Text: "Create or preview a task workspace"},
	{ID: "tui.form.task-start-pr", Text: "Create workspace from PR"},
	{ID: "tui.form.task-start-pr.desc", Text: "Create or preview a workspace from a pull request"},
	{ID: "tui.form.task-finish", Text: "Finish workspace"},
	{ID: "tui.form.task-finish.desc", Text: "Preview or execute workspace finish"},
	{ID: "tui.form.task-teardown", Text: "Remove workspace"},
	{ID: "tui.form.task-teardown.desc", Text: "Preview or remove a workspace"},
	{ID: "tui.form.task-prune", Text: "Clean workspaces"},
	{ID: "tui.form.task-prune.desc", Text: "Clean finished workspaces"},
	{ID: "tui.form.task-add-work-item", Text: "Add work item"},
	{ID: "tui.form.task-add-work-item.desc", Text: "Add work items to the workspace"},
	{ID: "tui.form.task-remove-work-item", Text: "Remove work item"},
	{ID: "tui.form.task-remove-work-item.desc", Text: "Remove work items from the workspace"},
	{ID: "tui.form.task-add-repo", Text: "Add repository"},
	{ID: "tui.form.task-add-repo.desc", Text: "Add a repository to the workspace"},
	{ID: "tui.form.task-rename", Text: "Rename workspace"},
	{ID: "tui.form.task-rename.desc", Text: "Rename workspace and branch"},
	{ID: "tui.form.ado-assigned", Text: "My work items"},
	{ID: "tui.form.ado-assigned.desc", Text: "List assigned work items with filters"},
	{ID: "tui.form.ado-set-state", Text: "Move work item state"},
	{ID: "tui.form.ado-set-state.desc", Text: "Move selected ADO work items to a destination state"},
	{ID: "tui.form.db-schema", Text: "Explore database structure"},
	{ID: "tui.form.db-schema.desc", Text: "List tables and views from a database"},
	{ID: "tui.form.db-describe", Text: "Describe database table"},
	{ID: "tui.form.db-describe.desc", Text: "Describe table columns"},
	{ID: "tui.form.db-query", Text: "Guided database query"},
	{ID: "tui.form.db-query.desc", Text: "Run a read-only SQL query"},
	{ID: "tui.form.agent-open", Text: "Open agent"},
	{ID: "tui.form.agent-open.desc", Text: "Open a workspace with an AI agent"},
	{ID: "tui.form.secret", Text: "Secret"},
	{ID: "tui.form.secret.desc", Text: "Check, remove or populate a secret"},
	{ID: "tui.form.config-root", Text: "Change root"},
	{ID: "tui.form.config-root.desc", Text: "Change the user DevWorkflow root"},
	{ID: "tui.field.work-item", Text: "Work item"},
	{ID: "tui.field.work-items", Text: "Work items"},
	{ID: "tui.field.work-item-ids", Text: "Work item IDs"},
	{ID: "tui.field.workspace-work-item", Text: "Workspace work item"},
	{ID: "tui.field.pull-request", Text: "Pull request"},
	{ID: "tui.field.project", Text: "Project"},
	{ID: "tui.field.repository", Text: "Repository"},
	{ID: "tui.field.type", Text: "Type"},
	{ID: "tui.field.slug", Text: "Slug"},
	{ID: "tui.field.workspace", Text: "Workspace"},
	{ID: "tui.field.continue", Text: "Continue"},
	{ID: "tui.field.execute", Text: "Execute"},
	{ID: "tui.field.skip-ado", Text: "Skip ADO"},
	{ID: "tui.field.message", Text: "Message"},
	{ID: "tui.field.create-pr", Text: "Create PR"},
	{ID: "tui.field.ready", Text: "Ready"},
	{ID: "tui.field.skip-verify", Text: "Skip verification"},
	{ID: "tui.field.no-sync", Text: "No sync"},
	{ID: "tui.field.title", Text: "Title"},
	{ID: "tui.field.state", Text: "State"},
	{ID: "tui.field.top", Text: "Top"},
	{ID: "tui.field.include-final", Text: "Include final states"},
	{ID: "tui.field.group-parent", Text: "Group by parent"},
	{ID: "tui.field.destination-state", Text: "Destination state"},
	{ID: "tui.field.ado-note", Text: "ADO note"},
	{ID: "tui.field.database", Text: "Database"},
	{ID: "tui.field.table", Text: "Table"},
	{ID: "tui.field.sql", Text: "SQL"},
	{ID: "tui.field.max-rows", Text: "Max rows"},
	{ID: "tui.field.agent", Text: "Agent"},
	{ID: "tui.field.key", Text: "Key"},
	{ID: "tui.field.set-env", Text: "Set from environment"},
	{ID: "tui.field.from-env", Text: "From environment"},
	{ID: "tui.field.delete", Text: "Delete"},
	{ID: "tui.field.root", Text: "Root"},
	{ID: "tui.field.help.optional", Text: "Optional"},
	{ID: "tui.field.help.required", Text: "Required"},
	{ID: "tui.field.help.ids", Text: "Comma-separated identifiers"},
	{ID: "tui.field.help.suggest", Text: "Ctrl+Space cycles loaded suggestions"},
	{ID: "tui.field.help.toggle", Text: "Space toggles this option"},
	{ID: "tui.validation.required", Text: "{field} is required."},
	{ID: "tui.validation.integer", Text: "{field} must be a whole number."},
	{ID: "tui.validation.agent", Text: "Agent must be opencode, cursor, claude, codex, codex-cli or copilot."},
	{ID: "tui.form.risk", Text: "Risk: {risk}"},
	{ID: "tui.form.incomplete", Text: "Incomplete operation"},
	{ID: "tui.menu.information", Text: "Information"},
	{ID: "tui.menu.configuration", Text: "Configuration"},
	{ID: "tui.menu.default-agent", Text: "Default agent"},
	{ID: "tui.menu.terminal-color", Text: "Terminal color"},
}

type tuiLocalizer struct {
	base l10n.Localizer
	tui  *l10n.Catalog
}

func (localizer tuiLocalizer) Text(id l10n.ID) string {
	if localizer.tui.Has(id) {
		return localizer.tui.Text(id)
	}
	return localizer.base.Text(id)
}

func (localizer tuiLocalizer) Render(message l10n.Message) string {
	if localizer.tui.Has(message.ID) {
		return localizer.tui.Render(message)
	}
	return localizer.base.Render(message)
}

func newTUILocalizer(base l10n.Localizer) l10n.Localizer {
	if base == nil {
		base = l10n.NewEnglish()
	}
	catalog, err := l10n.NewCatalog(tuiEnglishEntries)
	if err != nil {
		panic(err)
	}
	return tuiLocalizer{base: base, tui: catalog}
}
