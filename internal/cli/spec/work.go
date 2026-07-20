package spec

func workGrammar(b *builder) *Command {
	status := b.command("status", "work.status", "List task workspaces detected under the root.", []Argument{b.option("work.status", "root", String, "DevWorkflow root to scan.")})
	list := b.command("list", "work.list", "List task workspaces with project/work item filters.", []Argument{
		b.option("work.list", "root", String, "DevWorkflow root to scan."),
		completion(b.option("work.list", "project", String, "Configured project to filter by."), CompleteProject),
		completion(b.option("work.list", "work-item", String, "Work item to filter by."), CompleteWorkItem),
		b.option("work.list", "json", Bool, "Emit the deterministic JSON list."),
	})
	current := b.command("current", "work.current", "Show the current task workspace from the current directory.", []Argument{b.option("work.current", "json", Bool, "Emit the current workspace as deterministic JSON.")})
	open := b.command("open", "work.open", "Open or resume a task workspace with the configured agent.", []Argument{
		completion(conflict(b.option("work.open", "workspace", String, "Workspace path to open directly."), "project", "work_item", "continue"), CompleteWorkspace),
		b.option("work.open", "root", String, "DevWorkflow root to use."),
		completion(conflict(b.option("work.open", "project", String, "Configured project used to resolve the workspace."), "workspace"), CompleteProject),
		completion(b.option("work.open", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
		conflict(b.option("work.open", "pr", String, "Azure DevOps pull request used to resolve the existing workspace."), "workspace", "work_item", "continue"),
		conflict(b.option("work.open", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
		completion(b.option("work.open", "repo", String, "Repository to open in the workspace."), CompleteRepository),
		completion(b.option("work.open", "agent", String, "Agent to launch: opencode, cursor, claude, codex, codex-cli, or copilot."), CompleteAgent, "opencode", "cursor", "claude", "codex", "codex-cli", "copilot"),
		b.option("work.open", "json", Bool, "Emit resolution JSON instead of launching the agent."),
		b.positional("work.open", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	})
	start := b.command("start", "work.start", "Prepare or create a task workspace from ADO work items.", []Argument{
		b.positional("work.start", "work_item_id", "WORK_ITEM_ID", String, false, "Parent or child ADO work item ID to start."),
		b.option("work.start", "root", String, "DevWorkflow root to use."),
		completion(b.option("work.start", "project", String, "Configured project to use."), CompleteProject),
		b.option("work.start", "task", String, "Child task ID to add to the workspace."),
		completion(b.option("work.start", "type", String, "Branch/workspace type: feature, bugfix, hotfix, or chore."), CompleteWorkType, "feature", "bugfix", "hotfix", "chore"),
		completion(b.option("work.start", "only", String, "Repository to include; repeat through interactive selection when omitted."), CompleteRepository),
		b.option("work.start", "slug", String, "Explicit slug for the branch and workspace name."),
		b.option("work.start", "skip-ado", Bool, "Do not query Azure DevOps; use the provided local values."),
		conflict(b.option("work.start", "with-active-children", Bool, "Automatically include non-final ADO children of the selected subject."), "skip_ado"),
		conflict(b.option("work.start", "create-child-tasks", Bool, "Create one ADO child task per included repository before creating the workspace."), "skip_ado"),
		b.option("work.start", "json", Bool, "Emit the plan or result as deterministic JSON."),
		b.option("work.start", "execute", Bool, "Actually create the workspace; without this flag, show the plan."),
	})
	pr := b.command("pr", "work.pr", "Manage pull-request-based workspaces.", nil,
		b.command("start", "work.pr.start", "Prepare or create a workspace from work items linked to a pull request.", []Argument{
			b.positional("work.pr.start", "pull_request_id", "PULL_REQUEST_ID", String, true, "Azure DevOps pull request ID."),
			b.option("work.pr.start", "root", String, "DevWorkflow root to use."),
			mandatory(completion(b.option("work.pr.start", "project", String, "Configured project to use."), CompleteProject)),
			completion(b.option("work.pr.start", "repo", String, "Local or Azure DevOps repository for the PR."), CompleteRepository),
			completion(b.option("work.pr.start", "type", String, "Branch/workspace type: feature, bugfix, hotfix, or chore."), CompleteWorkType, "feature", "bugfix", "hotfix", "chore"),
			b.option("work.pr.start", "slug", String, "Explicit slug for the branch and workspace name."),
			b.option("work.pr.start", "json", Bool, "Emit the plan or result as deterministic JSON."),
			b.option("work.pr.start", "execute", Bool, "Actually create the workspace; without this flag, show the plan."),
		}),
	)
	preflight := b.command("preflight", "work.preflight", "Validate blockers and warnings before implementation.", append(workspaceResolution(b, "work.preflight", "Workspace path to audit.", "Resume the most recent matching task workspace."),
		repeat(b.option("work.preflight", "ai-context-file", String, "Additional AI context file to verify; repeatable option.")),
		b.option("work.preflight", "json", Bool, "Emit the deterministic preflight report JSON."),
		b.positional("work.preflight", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	))
	sync := b.command("sync", "work.sync", "Synchronize task.json with Azure DevOps work items.", append(workspaceResolution(b, "work.sync", "Workspace path to synchronize.", "Resume the most recent matching task workspace."),
		b.option("work.sync", "json", Bool, "Emit the deterministic result JSON."),
		b.positional("work.sync", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	))
	rename := b.command("rename", "work.rename", "Rename a task workspace and its branch using a new slug.", append([]Argument{b.positional("work.rename", "slug", "SLUG", String, true, "New slug for the workspace and branch.")}, append(workspaceResolution(b, "work.rename", "Workspace path to rename.", "Resume the most recent matching task workspace."),
		b.option("work.rename", "json", Bool, "Emit the plan/result as deterministic JSON."),
		b.option("work.rename", "execute", Bool, "Actually apply the rename; without this flag, show the plan."),
		b.positional("work.rename", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	)...))
	repo := workRepoGrammar(b)
	commit := b.command("commit", "work.commit", "Prepare or create an intermediate commit for workspace repositories.", []Argument{
		completion(conflict(b.option("work.commit", "workspace", String, "Workspace path to commit."), "continue"), CompleteWorkspace),
		conflict(b.option("work.commit", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
		b.option("work.commit", "root", String, "DevWorkflow root to use."),
		b.option("work.commit", "execute", Bool, "Actually create commits; without this flag, show the plan."),
		b.option("work.commit", "message", String, "Explicit commit message; otherwise generated from the task manifest."),
		b.option("work.commit", "json", Bool, "Emit the deterministic JSON report."),
	})
	finish := b.command("finish", "work.finish", "Verify, commit, push, and open a PR to finish the workspace.", []Argument{
		completion(conflict(b.option("work.finish", "workspace", String, "Workspace path to finish."), "continue"), CompleteWorkspace),
		conflict(b.option("work.finish", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
		b.option("work.finish", "root", String, "DevWorkflow root to use."),
		b.option("work.finish", "execute", Bool, "Run commits, pushes, PRs, and ADO updates; without this flag, show the plan."),
		b.option("work.finish", "yes", Bool, "Confirm destructive finish with --execute."),
		b.option("work.finish", "message", String, "Explicit commit message; otherwise generated from the task manifest."),
		b.option("work.finish", "create-pr", Bool, "Create or verify Azure DevOps pull requests after push."),
		require(b.option("work.finish", "ready", Bool, "Create PRs as ready instead of draft."), "create_pr"),
		b.option("work.finish", "skip-verify", Bool, "Skip configured verification commands before PR."),
		b.option("work.finish", "skip-ado", Bool, "Do not call Azure DevOps; incompatible with --create-pr."),
		b.option("work.finish", "force-with-lease", Bool, "Allow rewritten workspace branches to replace remote branches only if their remote-tracking refs have not changed."),
		b.option("work.finish", "json", Bool, "Emit the deterministic JSON report."),
	})
	handoff := workHandoffGrammar(b)
	teardown := b.command("teardown", "work.teardown", "Remove worktrees and clean up a task workspace.", []Argument{
		completion(b.option("work.teardown", "workspace", String, "Workspace path to remove."), CompleteWorkspace),
		b.option("work.teardown", "root", String, "DevWorkflow root to use."),
		completion(b.option("work.teardown", "project", String, "Configured project used to resolve the workspace."), CompleteProject),
		completion(b.option("work.teardown", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
		b.option("work.teardown", "continue", Bool, "Resume the most recent matching task workspace."),
		b.option("work.teardown", "execute", Bool, "Actually remove worktrees and the workspace; without this flag, show the plan."),
		b.option("work.teardown", "yes", Bool, "Confirm destructive removal with --execute."),
		b.option("work.teardown", "json", Bool, "Emit the plan/result as deterministic JSON."),
		b.positional("work.teardown", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
	})
	prune := b.command("prune", "work.prune", "Clean up workspaces whose work items are finished.", []Argument{
		b.option("work.prune", "root", String, "DevWorkflow root to scan."),
		completion(b.option("work.prune", "project", String, "Configured project to filter by."), CompleteProject),
		completion(b.option("work.prune", "work-item", String, "Work item to filter by."), CompleteWorkItem),
		b.option("work.prune", "execute", Bool, "Actually remove eligible workspaces; without this flag, show the plan."),
		b.option("work.prune", "yes", Bool, "Confirm destructive removal with --execute."),
		b.option("work.prune", "no-sync", Bool, "Do not synchronize ADO states before determining eligibility."),
		b.option("work.prune", "json", Bool, "Emit the plan/result as deterministic JSON."),
	})
	return b.command("work", "work", "Manage workspaces, worktrees, commits, PRs, and work items.", nil,
		status, list, current, workItemGrammar(b), workTaskGrammar(b), open, start, pr, preflight, sync, rename, repo, commit, finish, handoff, teardown, prune,
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

func workItemGrammar(b *builder) *Command {
	return b.command("item", "work.item", "Manage work items attached to a workspace.", nil,
		b.command("doing", "work.item.doing", "Move ADO work items to their configured in-progress state.", []Argument{
			b.positional("work.item.doing", "id", "ID", String, true, "Azure DevOps work item ID, or comma-separated list."),
			b.option("work.item.doing", "root", String, "DevWorkflow root to use."), completion(b.option("work.item.doing", "project", String, "Configured project to use."), CompleteProject),
			b.option("work.item.doing", "yes", Bool, "Confirm the work item state changes."), b.option("work.item.doing", "json", Bool, "Emit the deterministic report JSON; requires --yes."),
		}),
		b.command("add", "work.item.add", "Add work items to the current workspace.", append([]Argument{b.positional("work.item.add", "work_item_ids", "WORK_ITEM_IDS", String, false, "Work item IDs to add, separated by commas.")}, append(workspaceResolution(b, "work.item.add", "Workspace path to modify.", "Resume the most recent workspace."),
			b.option("work.item.add", "skip-ado", Bool, "Do not query Azure DevOps; use the provided local values."),
			completion(b.option("work.item.add", "type", String, "Local type to use when ADO is skipped or incomplete."), CompleteWorkType, "feature", "bugfix", "hotfix", "chore"),
			b.option("work.item.add", "title", String, "Local title to use when ADO is skipped or incomplete."), b.option("work.item.add", "state", String, "Local state to use when ADO is skipped or incomplete."),
			b.option("work.item.add", "execute", Bool, "Actually apply the change; without this flag, show the plan."), b.option("work.item.add", "json", Bool, "Emit the plan/result as deterministic JSON."),
			b.positional("work.item.add", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		)...)),
		b.command("remove", "work.item.remove", "Remove work items from the current workspace.", []Argument{
			b.positional("work.item.remove", "work_item_ids", "WORK_ITEM_IDS", String, false, "Work item IDs to remove, separated by commas."),
			completion(b.option("work.item.remove", "workspace", String, "Workspace path to modify."), CompleteWorkspace), b.option("work.item.remove", "root", String, "DevWorkflow root to use."),
			completion(b.option("work.item.remove", "project", String, "Configured project used to resolve the workspace."), CompleteProject), completion(b.option("work.item.remove", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
			b.option("work.item.remove", "continue", Bool, "Resume the most recent workspace."), b.option("work.item.remove", "execute", Bool, "Actually apply the change; without this flag, show the plan."), b.option("work.item.remove", "json", Bool, "Emit the plan/result as deterministic JSON."),
			b.positional("work.item.remove", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		}),
	)
}

func workRepoGrammar(b *builder) *Command {
	return b.command("repo", "work.repo", "Manage workspace repositories.", nil,
		b.command("add", "work.repo.add", "Add a repository to the workspace.", []Argument{
			completion(b.positional("work.repo.add", "repo", "REPO", String, false, "Configured repository to add to the workspace."), CompleteRepository), completion(b.option("work.repo.add", "workspace", String, "Workspace path to modify."), CompleteWorkspace),
			b.option("work.repo.add", "root", String, "DevWorkflow root to use."), b.option("work.repo.add", "execute", Bool, "Create the worktree and modify task.json; without this flag, show the plan."), b.option("work.repo.add", "json", Bool, "Emit the plan/result as deterministic JSON."),
		}),
		b.command("latest", "work.repo.latest", "Update workspace repositories from their target branch.", []Argument{
			completion(conflict(b.option("work.repo.latest", "workspace", String, "Workspace path to synchronize."), "continue"), CompleteWorkspace), conflict(b.option("work.repo.latest", "continue", Bool, "Resume the most recent workspace."), "workspace"),
			completion(b.option("work.repo.latest", "only", String, "Limit synchronization to one workspace repository."), CompleteRepository), b.option("work.repo.latest", "root", String, "DevWorkflow root to use."), b.option("work.repo.latest", "json", Bool, "Emit the plan/result as deterministic JSON."),
		}),
	)
}

func workHandoffGrammar(b *builder) *Command {
	return b.command("handoff", "work.handoff", "Manage workspace handoff files.", nil,
		b.command("validate", "work.handoff.validate", "Validate handoff files before sub-agents or finishing.", append(workspaceResolution(b, "work.handoff.validate", "Workspace path whose handoffs must be valid.", "Resume the most recent workspace."),
			b.option("work.handoff.validate", "json", Bool, "Emit the deterministic JSON report."), b.positional("work.handoff.validate", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		)),
	)
}

func workTaskGrammar(b *builder) *Command {
	return b.command("task", "work.task", "Manage ADO tasks linked to workspace work.", nil,
		b.command("child", "work.task.child", "Manage child tasks.", nil,
			b.command("create", "work.task.child.create", "Create an ADO child task and add it to the repository handoff.", []Argument{
				mandatory(completion(b.option("work.task.child.create", "repo", String, "Workspace repository that will carry the task handoff."), CompleteRepository)), mandatory(b.option("work.task.child.create", "title", String, "Title of the ADO child task to create.")),
				completion(b.option("work.task.child.create", "workspace", String, "Workspace path to modify."), CompleteWorkspace), b.option("work.task.child.create", "root", String, "DevWorkflow root to use."), completion(b.option("work.task.child.create", "project", String, "Configured project used to resolve the workspace."), CompleteProject), completion(b.option("work.task.child.create", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
				b.option("work.task.child.create", "continue", Bool, "Resume the most recent workspace."), b.option("work.task.child.create", "json", Bool, "Emit the deterministic result JSON."), b.positional("work.task.child.create", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
			}),
		),
	)
}
