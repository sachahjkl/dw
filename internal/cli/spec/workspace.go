package spec

func workspaceGrammar(b *builder) *Command {
	status := b.command("status", "workspace.status", "List task workspaces detected under the root.", []Argument{b.option("workspace.status", "root", String, "DevWorkflow root to scan.")})
	list := b.command("list", "workspace.list", "List task workspaces with project and work item filters.", []Argument{
		b.option("workspace.list", "root", String, "DevWorkflow root to scan."),
		completion(b.option("workspace.list", "project", String, "Configured project to filter by."), CompleteProject),
		completion(b.option("workspace.list", "work-item", String, "Work item to filter by."), CompleteWorkItem),
		b.option("workspace.list", "json", Bool, "Emit the deterministic JSON list."),
	})
	current := b.command("current", "workspace.current", "Show the current task workspace from the current directory.", []Argument{b.option("workspace.current", "json", Bool, "Emit the current workspace as deterministic JSON.")})
	open := b.command("open", "workspace.open", "Open or resume a task workspace with the configured agent.", []Argument{
		completion(conflict(b.option("workspace.open", "workspace", String, "Workspace path to open directly."), "project", "work_item", "continue"), CompleteWorkspace),
		b.option("workspace.open", "root", String, "DevWorkflow root to use."),
		completion(conflict(b.option("workspace.open", "project", String, "Configured project used to resolve the workspace."), "workspace"), CompleteProject),
		providerOption(b, "workspace.open"),
		completion(b.option("workspace.open", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
		conflict(b.option("workspace.open", "pr", String, "Provider pull request used to resolve the existing workspace."), "workspace", "work_item", "continue"),
		conflict(b.option("workspace.open", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
		completion(b.option("workspace.open", "repo", String, "Repository to open in the workspace."), CompleteRepository),
		completion(b.option("workspace.open", "agent", String, "Agent to launch: opencode, cursor, claude, codex, codex-cli, or copilot."), CompleteAgent, "opencode", "cursor", "claude", "codex", "codex-cli", "copilot"),
		b.option("workspace.open", "json", Bool, "Emit resolution JSON instead of launching the agent."),
		b.positional("workspace.open", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	})
	start := b.command("start", "workspace.start", "Prepare or create a task workspace from provider work items.", []Argument{
		b.positional("workspace.start", "work_item_id", "WORK_ITEM_ID", String, false, "Parent or child work item ID to start."),
		b.option("workspace.start", "root", String, "DevWorkflow root to use."),
		completion(b.option("workspace.start", "project", String, "Configured project to use."), CompleteProject),
		providerOption(b, "workspace.start"),
		b.option("workspace.start", "task", String, "Child work item ID to add to the workspace."),
		completion(b.option("workspace.start", "type", String, "Branch/workspace type: feature, bugfix, hotfix, or chore."), CompleteWorkType, "feature", "bugfix", "hotfix", "chore"),
		completion(b.option("workspace.start", "only", String, "Repository to include; repeat through interactive selection when omitted."), CompleteRepository),
		b.option("workspace.start", "slug", String, "Explicit slug for the branch and workspace name."),
		b.option("workspace.start", "skip-provider", Bool, "Do not query the work provider; use the provided local values."),
		conflict(b.option("workspace.start", "with-active-children", Bool, "Automatically include non-final children of the selected subject."), "skip_provider"),
		conflict(b.option("workspace.start", "create-child-tasks", Bool, "Create one provider child task per included repository before creating the workspace."), "skip_provider"),
		b.option("workspace.start", "json", Bool, "Emit the plan or result as deterministic JSON."),
		b.option("workspace.start", "execute", Bool, "Actually create the workspace; without this flag, show the plan."),
	})
	pr := b.command("pr", "workspace.pr", "Manage pull-request-based workspaces.", nil,
		b.command("start", "workspace.pr.start", "Prepare or create a workspace from work items linked to a pull request.", []Argument{
			b.positional("workspace.pr.start", "pull_request_id", "PULL_REQUEST_ID", String, true, "Provider pull request ID."),
			b.option("workspace.pr.start", "root", String, "DevWorkflow root to use."),
			mandatory(completion(b.option("workspace.pr.start", "project", String, "Configured project to use."), CompleteProject)),
			providerOption(b, "workspace.pr.start"),
			completion(b.option("workspace.pr.start", "repo", String, "Local or provider repository for the pull request."), CompleteRepository),
			completion(b.option("workspace.pr.start", "type", String, "Branch/workspace type: feature, bugfix, hotfix, or chore."), CompleteWorkType, "feature", "bugfix", "hotfix", "chore"),
			b.option("workspace.pr.start", "slug", String, "Explicit slug for the branch and workspace name."),
			b.option("workspace.pr.start", "json", Bool, "Emit the plan or result as deterministic JSON."),
			b.option("workspace.pr.start", "execute", Bool, "Actually create the workspace; without this flag, show the plan."),
		}),
	)
	preflight := b.command("preflight", "workspace.preflight", "Validate blockers and warnings before implementation.", append(workspaceResolution(b, "workspace.preflight", "Workspace path to audit.", "Resume the most recent matching task workspace."),
		repeat(b.option("workspace.preflight", "ai-context-file", String, "Additional AI context file to verify; repeatable option.")),
		b.option("workspace.preflight", "json", Bool, "Emit the deterministic preflight report JSON."),
		b.positional("workspace.preflight", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	))
	sync := b.command("sync", "workspace.sync", "Synchronize task.json with provider work items.", append(workspaceResolution(b, "workspace.sync", "Workspace path to synchronize.", "Resume the most recent matching task workspace."),
		providerOption(b, "workspace.sync"),
		b.option("workspace.sync", "json", Bool, "Emit the deterministic result JSON."),
		b.positional("workspace.sync", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	))
	rename := b.command("rename", "workspace.rename", "Rename a task workspace and its branch using a new slug.", append([]Argument{b.positional("workspace.rename", "slug", "SLUG", String, true, "New slug for the workspace and branch.")}, append(workspaceResolution(b, "workspace.rename", "Workspace path to rename.", "Resume the most recent matching task workspace."),
		b.option("workspace.rename", "json", Bool, "Emit the plan/result as deterministic JSON."),
		b.option("workspace.rename", "execute", Bool, "Actually apply the rename; without this flag, show the plan."),
		b.positional("workspace.rename", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	)...))
	repo := workspaceRepoGrammar(b)
	item := workspaceItemGrammar(b)
	commit := b.command("commit", "workspace.commit", "Prepare or create an intermediate commit for workspace repositories.", []Argument{
		completion(conflict(b.option("workspace.commit", "workspace", String, "Workspace path to commit."), "continue"), CompleteWorkspace),
		conflict(b.option("workspace.commit", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
		b.option("workspace.commit", "root", String, "DevWorkflow root to use."),
		b.option("workspace.commit", "execute", Bool, "Actually create commits; without this flag, show the plan."),
		b.option("workspace.commit", "message", String, "Explicit commit message; otherwise generated from the task manifest."),
		b.option("workspace.commit", "json", Bool, "Emit the deterministic JSON report."),
	})
	finish := b.command("finish", "workspace.finish", "Verify, commit, push, and open a pull request to finish the workspace.", []Argument{
		completion(conflict(b.option("workspace.finish", "workspace", String, "Workspace path to finish."), "continue"), CompleteWorkspace),
		conflict(b.option("workspace.finish", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
		b.option("workspace.finish", "root", String, "DevWorkflow root to use."),
		providerOption(b, "workspace.finish"),
		b.option("workspace.finish", "execute", Bool, "Run commits, pushes, pull requests, and provider updates; without this flag, show the plan."),
		b.option("workspace.finish", "yes", Bool, "Confirm destructive finish with --execute."),
		b.option("workspace.finish", "message", String, "Explicit commit message; otherwise generated from the task manifest."),
		b.option("workspace.finish", "create-pr", Bool, "Create or verify provider pull requests after push."),
		require(b.option("workspace.finish", "ready", Bool, "Create pull requests as ready instead of draft."), "create_pr"),
		b.option("workspace.finish", "skip-verify", Bool, "Skip configured verification commands before the pull request."),
		b.option("workspace.finish", "skip-provider", Bool, "Do not call the work provider; incompatible with --create-pr."),
		b.option("workspace.finish", "force-with-lease", Bool, "Allow rewritten workspace branches to replace remote branches only if their remote-tracking refs have not changed."),
		b.option("workspace.finish", "json", Bool, "Emit the deterministic JSON report."),
	})
	handoff := workspaceHandoffGrammar(b)
	teardown := b.command("teardown", "workspace.teardown", "Remove worktrees and clean up a task workspace.", []Argument{
		completion(b.option("workspace.teardown", "workspace", String, "Workspace path to remove."), CompleteWorkspace),
		b.option("workspace.teardown", "root", String, "DevWorkflow root to use."),
		completion(b.option("workspace.teardown", "project", String, "Configured project used to resolve the workspace."), CompleteProject),
		completion(b.option("workspace.teardown", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
		b.option("workspace.teardown", "continue", Bool, "Resume the most recent matching task workspace."),
		b.option("workspace.teardown", "execute", Bool, "Actually remove worktrees and the workspace; without this flag, show the plan."),
		b.option("workspace.teardown", "yes", Bool, "Confirm destructive removal with --execute."),
		b.option("workspace.teardown", "json", Bool, "Emit the plan/result as deterministic JSON."),
		b.positional("workspace.teardown", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	})
	prune := b.command("prune", "workspace.prune", "Clean up workspaces whose work items are finished.", []Argument{
		b.option("workspace.prune", "root", String, "DevWorkflow root to scan."),
		completion(b.option("workspace.prune", "project", String, "Configured project to filter by."), CompleteProject),
		providerOption(b, "workspace.prune"),
		completion(b.option("workspace.prune", "work-item", String, "Work item to filter by."), CompleteWorkItem),
		b.option("workspace.prune", "execute", Bool, "Actually remove eligible workspaces; without this flag, show the plan."),
		b.option("workspace.prune", "yes", Bool, "Confirm destructive removal with --execute."),
		b.option("workspace.prune", "no-sync", Bool, "Do not synchronize provider states before determining eligibility."),
		b.option("workspace.prune", "json", Bool, "Emit the plan/result as deterministic JSON."),
	})
	return b.command("workspace", "workspace", "Manage local workspaces, worktrees, repositories, commits, and pull requests.", nil,
		status, list, current, open, start, pr, preflight, sync, rename, repo, item, commit, finish, handoff, teardown, prune,
	)
}

func workspaceResolution(b *builder, key, workspaceHelp, continueHelp string) []Argument {
	return []Argument{
		completion(conflict(b.option(key, "workspace", String, workspaceHelp), "project", "work_item", "continue"), CompleteWorkspace),
		b.option(key, "root", String, "DevWorkflow root to use."),
		completion(conflict(b.option(key, "project", String, "Configured project used to resolve the workspace."), "workspace"), CompleteProject),
		completion(b.option(key, "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
		conflict(b.option(key, "continue", Bool, continueHelp), "workspace"),
	}
}

func workspaceItemGrammar(b *builder) *Command {
	return b.command("item", "workspace.item", "Manage work items attached to a workspace.", nil,
		b.command("add", "workspace.item.add", "Add work items to the current workspace.", append([]Argument{b.positional("workspace.item.add", "work_item_ids", "WORK_ITEM_IDS", String, false, "Work item IDs to add, separated by commas.")}, append(workspaceResolution(b, "workspace.item.add", "Workspace path to modify.", "Resume the most recent workspace."),
			providerOption(b, "workspace.item.add"),
			b.option("workspace.item.add", "skip-provider", Bool, "Do not query the work provider; use the provided local values."),
			completion(b.option("workspace.item.add", "type", String, "Local type to use when provider data is skipped or incomplete."), CompleteWorkType, "feature", "bugfix", "hotfix", "chore"),
			b.option("workspace.item.add", "title", String, "Local title to use when provider data is skipped or incomplete."), b.option("workspace.item.add", "state", String, "Local state to use when provider data is skipped or incomplete."),
			b.option("workspace.item.add", "execute", Bool, "Actually apply the change; without this flag, show the plan."), b.option("workspace.item.add", "json", Bool, "Emit the plan/result as deterministic JSON."),
			b.positional("workspace.item.add", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		)...)),
		b.command("remove", "workspace.item.remove", "Remove work items from the current workspace.", []Argument{
			b.positional("workspace.item.remove", "work_item_ids", "WORK_ITEM_IDS", String, false, "Work item IDs to remove, separated by commas."),
			completion(b.option("workspace.item.remove", "workspace", String, "Workspace path to modify."), CompleteWorkspace),
			b.option("workspace.item.remove", "root", String, "DevWorkflow root to use."),
			completion(b.option("workspace.item.remove", "project", String, "Configured project used to resolve the workspace."), CompleteProject),
			completion(b.option("workspace.item.remove", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
			b.option("workspace.item.remove", "continue", Bool, "Resume the most recent workspace."),
			b.option("workspace.item.remove", "execute", Bool, "Actually apply the change; without this flag, show the plan."),
			b.option("workspace.item.remove", "json", Bool, "Emit the plan/result as deterministic JSON."),
			b.positional("workspace.item.remove", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		}),
	)
}

func workspaceRepoGrammar(b *builder) *Command {
	return b.command("repo", "workspace.repo", "Manage workspace repositories.", nil,
		b.command("add", "workspace.repo.add", "Add a repository to the workspace.", []Argument{
			completion(b.positional("workspace.repo.add", "repo", "REPO", String, false, "Configured repository to add to the workspace."), CompleteRepository), completion(b.option("workspace.repo.add", "workspace", String, "Workspace path to modify."), CompleteWorkspace),
			b.option("workspace.repo.add", "root", String, "DevWorkflow root to use."), b.option("workspace.repo.add", "execute", Bool, "Create the worktree and modify task.json; without this flag, show the plan."), b.option("workspace.repo.add", "json", Bool, "Emit the plan/result as deterministic JSON."),
		}),
		b.command("latest", "workspace.repo.latest", "Update workspace repositories from their target branch.", []Argument{
			completion(conflict(b.option("workspace.repo.latest", "workspace", String, "Workspace path to synchronize."), "continue"), CompleteWorkspace),
			conflict(b.option("workspace.repo.latest", "continue", Bool, "Resume the most recent workspace."), "workspace"),
			completion(b.option("workspace.repo.latest", "only", String, "Limit synchronization to one workspace repository."), CompleteRepository),
			b.option("workspace.repo.latest", "root", String, "DevWorkflow root to use."),
			b.option("workspace.repo.latest", "json", Bool, "Emit the plan/result as deterministic JSON."),
		}),
	)
}
func workspaceHandoffGrammar(b *builder) *Command {
	return b.command("handoff", "workspace.handoff", "Manage workspace handoff files.", nil,
		b.command("validate", "workspace.handoff.validate", "Validate handoff files before sub-agents or finishing.", append(workspaceResolution(b, "workspace.handoff.validate", "Workspace path whose handoffs must be valid.", "Resume the most recent workspace."),
			b.option("workspace.handoff.validate", "json", Bool, "Emit the deterministic JSON report."), b.positional("workspace.handoff.validate", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		)),
	)
}
