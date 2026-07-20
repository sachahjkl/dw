package spec

func agentGrammar(b *builder) *Command {
	agents := []string{"opencode", "cursor", "claude", "codex", "codex-cli", "copilot"}
	return b.command("agent", "agent", "Show AI workflow context, open an agent, or manage agent configuration.", nil,
		b.command("context", "agent.context", "Show the DevWorkflow context injected into AI agents.", nil),
		b.command("open", "agent.open", "Open or resume an agent on a task workspace.", []Argument{
			completion(conflict(b.option("agent.open", "workspace", String, "Workspace path to open directly."), "project", "work_item", "continue"), CompleteWorkspace),
			b.option("agent.open", "root", String, "DevWorkflow root to use."),
			completion(conflict(b.option("agent.open", "project", String, "Configured project used to resolve a workspace."), "workspace"), CompleteProject),
			completion(b.option("agent.open", "work-item", String, "Work item used to resolve the workspace."), CompleteWorkItem),
			conflict(b.option("agent.open", "continue", Bool, "Resume the most recent matching task workspace."), "workspace"),
			completion(b.option("agent.open", "repo", String, "Repository to open in the workspace, when applicable."), CompleteRepository),
			completion(b.option("agent.open", "agent", String, "Agent to launch: opencode, cursor, claude, codex, codex-cli, or copilot."), CompleteAgent, agents...),
			b.positional("agent.open", "positional_work_item", "WORK_ITEM", String, false, "Positional work item alias used to resolve the workspace."),
		}),
		b.command("config", "agent.config", "Show the effective agent configuration.", []Argument{b.option("agent.config", "root", String, "DevWorkflow root to read.")}),
		b.command("show", "agent.show", "Show the effective agent configuration.", []Argument{b.option("agent.show", "root", String, "DevWorkflow root to read.")}),
		b.command("default", "agent.default", "Manage the default agent.", nil,
			b.command("set", "agent.default.set", "Set the default agent for the DevWorkflow root.", []Argument{
				completion(b.positional("agent.default.set", "agent", "AGENT", String, true, "Agent to use by default: opencode, cursor, claude, codex, codex-cli, or copilot."), CompleteAgent, agents...),
				b.option("agent.default.set", "root", String, "DevWorkflow root to modify."),
			}),
		),
		b.command("doctor", "agent.doctor", "Diagnose installed agent availability.", []Argument{
			completion(b.option("agent.doctor", "agent", String, "Limit diagnostics to one agent."), CompleteAgent, agents...),
		}),
	)
}
