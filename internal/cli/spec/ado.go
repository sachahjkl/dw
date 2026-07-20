package spec

func adoGrammar(b *builder) *Command {
	assigned := b.command("assigned", "ado.assigned", "List Azure DevOps work items assigned to the current user.", []Argument{
		b.option("ado.assigned", "root", String, "DevWorkflow root to use."),
		completion(b.option("ado.assigned", "project", String, "Configured project to query; opens an interactive picker when omitted."), CompleteProject),
		defaultInt(b.option("ado.assigned", "top", Int, "Maximum number of work items to load."), 20),
		b.option("ado.assigned", "all", Bool, "Also include work items in a final state."),
		b.option("ado.assigned", "group-by-parent", Bool, "Group work items by ADO parent."),
		b.option("ado.assigned", "json", Bool, "Emit the deterministic JSON response."),
	})
	prs := b.command("prs", "ado.prs", "List active Azure DevOps pull requests from configured repositories.", []Argument{
		b.option("ado.prs", "root", String, "DevWorkflow root to use."),
		mandatory(completion(b.option("ado.prs", "project", String, "Configured project to query."), CompleteProject)),
		completion(b.option("ado.prs", "repo", String, "Local or Azure DevOps repository to query; repeat with commas."), CompleteRepository),
		b.option("ado.prs", "json", Bool, "Emit the deterministic JSON response."),
	})
	changelog := b.command("changelog", "ado.changelog", "Build a changelog from PRs, a git range, or work items.", []Argument{
		b.positional("ado.changelog", "ids", "IDS", String, true, "Work item IDs, PRs, or git range depending on the selected mode."),
		b.option("ado.changelog", "root", String, "DevWorkflow root to use."),
		completion(b.option("ado.changelog", "project", String, "Configured project to use."), CompleteProject),
		conflict(b.option("ado.changelog", "from-pr", Bool, "Interpret IDs as Azure DevOps pull requests."), "from_git"),
		conflict(b.option("ado.changelog", "from-git", Bool, "Extract work items from git commits."), "from_pr"),
		completion(b.option("ado.changelog", "repo", String, "Configured repository key/name or local path; omit to scan every configured repository."), CompleteRepository),
		b.option("ado.changelog", "group-by-parent", Bool, "Group the changelog by ADO parent."),
		completion(choices(b.option("ado.changelog", "format", String, "Output format."), "raw", "markdown", "html"), CompleteFormat, "raw", "markdown", "html"),
		require(b.option("ado.changelog", "table", Bool, "Render the markdown/html changelog as a table."), "format"),
		b.option("ado.changelog", "ids-only", Bool, "Show only resolved IDs, separated by spaces."),
		require(b.option("ado.changelog", "git-to", String, "Ending revision for the git range."), "from_git"),
	})
	item := b.command("item", "ado.item", "Inspect Azure DevOps work items.", nil,
		b.command("show", "ado.item.show", "Show a readable summary of Azure DevOps work items.", []Argument{
			b.positional("ado.item.show", "id", "ID", String, true, "Azure DevOps work item ID, or comma-separated list."),
			b.option("ado.item.show", "root", String, "DevWorkflow root to use."),
			completion(b.option("ado.item.show", "project", String, "Configured project to use."), CompleteProject),
			b.option("ado.item.show", "json", Bool, "Emit the deterministic JSON response."),
		}),
	)
	state := b.command("state", "ado.state", "Manage Azure DevOps work item state.", nil,
		b.command("set", "ado.state.set", "Change the state of one or more Azure DevOps work items.", []Argument{
			b.positional("ado.state.set", "id", "ID", String, true, "Azure DevOps work item ID, or comma-separated list."),
			b.option("ado.state.set", "root", String, "DevWorkflow root to use."),
			completion(b.option("ado.state.set", "project", String, "Configured project to use."), CompleteProject),
			mandatory(completion(b.option("ado.state.set", "state", String, "Exact new ADO state to apply."), CompleteADOState)),
			b.option("ado.state.set", "history", String, "ADO history message; default: dw ado state set."),
			b.option("ado.state.set", "yes", Bool, "Confirm the destructive state change."),
			b.option("ado.state.set", "json", Bool, "Emit the deterministic JSON response; requires --yes."),
		}),
	)
	context := b.command("context", "ado.context", "Build human-readable or AI-ready work item context.", nil,
		b.command("show", "ado.context.show", "Show detailed context in a human-readable format.", []Argument{
			b.positional("ado.context.show", "id", "ID", String, true, "Azure DevOps work item ID, or comma-separated list."),
			b.option("ado.context.show", "root", String, "DevWorkflow root to use."),
			completion(b.option("ado.context.show", "project", String, "Configured project to use."), CompleteProject),
			b.option("ado.context.show", "summary", Bool, "Limit context to essential fields."),
			defaultInt(b.option("ado.context.show", "comments", Int, "Maximum number of comments to show; 0 for none."), 200),
			b.option("ado.context.show", "json", Bool, "Emit the deterministic JSON response."),
		}),
		b.command("ai", "ado.context.ai", "Emit structured, deterministic AI context.", []Argument{
			b.positional("ado.context.ai", "id", "ID", String, true, "Azure DevOps work item ID, or comma-separated list."),
			b.option("ado.context.ai", "root", String, "DevWorkflow root to use."),
			b.option("ado.context.ai", "organization", String, "Explicit Azure DevOps organization."),
			completion(b.option("ado.context.ai", "project", String, "Configured project or explicit Azure DevOps project."), CompleteProject),
			b.option("ado.context.ai", "summary", Bool, "Limit the contract to essential fields."),
			defaultInt(b.option("ado.context.ai", "comments", Int, "Maximum number of comments to include."), 200),
			b.option("ado.context.ai", "include-comments", Bool, "Include comments in the AI context."),
		}),
	)
	return b.command("ado", "ado", "Azure DevOps commands.", nil, assigned, prs, changelog, item, state, context)
}
