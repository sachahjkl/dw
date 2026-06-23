namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public const string AgentsMd = """
# DevWorkflow Rules

This workspace is managed by `dw`.

Mandatory rules:

1. Use `dw agent context` before starting an AI workflow.
2. Use Azure DevOps work items as the source of truth for task state.
3. Use `dw ado ...` and `dw task ...` for Azure DevOps/worktree operations; do not use Azure DevOps MCP tools.
4. Read the work item with `dw ado work-item <id> --project <name>` before coding, then use `dw ado context <id> --project <name>` when more detail is needed.
5. Use `dw db schema`, `dw db describe <table>` and `dw db query ...` whenever database context can clarify the change.
6. Run `dw task current` before lifecycle actions and before committing to confirm the active workspace.
7. Update `plan.md` in the task workspace before implementing.
8. If the local ADO context may be stale, use `dw task sync --continue` before acting on ADO state.
9. For API contract changes, always check both front and back.
10. Use `dw task commit` for intermediate commits and `dw task finish` for final push/PR.
""";

    public const string OpenCodeJsonc = """
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "lsp": true,
  "permission": {
    "bash": "allow",
    "edit": "allow"
  }
}
""";

    public const string OgfAgentsMd = """
# DevWorkflow OGF Rules

This workspace is managed by `dw`.

Mandatory rules:

1. Run `dw agent context` before starting an AI workflow.
2. Use Azure DevOps work items as the source of truth.
3. Use only `dw ado ...`, `dw auth ...` and `dw task ...` for Azure DevOps/worktree operations; do not use Azure DevOps MCP tools.
4. Read the work item with `dw ado work-item <id> --project <name>` before coding, then use `dw ado context <id> --project <name>` for the full context.
5. Use `dw db schema`, `dw db describe <table>` and `dw db query ...` whenever database context can clarify the change.
6. Run `dw task current` before lifecycle actions and before committing to confirm the active workspace.
7. Fill `plan.md` in the task workspace before implementing.
8. If the local ADO context may be stale, use `dw task sync --continue` before acting on ADO state.
9. Use `dw` commands for ADO lifecycle, Git naming, worktrees, commits and PRs.
10. For API contract changes, always check both front and back.
11. Write ADO/PR/commit text in French unless a repository convention says otherwise.
12. Use `dw task commit` for intermediate commits and `dw task finish` for final push/PR.
""";

    public const string OgfOpenCodeJsonc = """
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "lsp": true,
  "permission": {
    "bash": "allow",
    "edit": "allow"
  }
}
""";

    public static string AgentContext(string root) => $$"""
# DevWorkflow agent context

You are working inside a DevWorkflow-managed environment.

Use `dw` for workflow operations:

- `dw doctor` checks local prerequisites.
- `dw auth login` connects Azure DevOps when the silent token is unavailable.
- `dw ado assigned --project <name>` lists assigned work items.
- `dw ado work-item <workItemId> --project <name>` reads a work item summary.
- `dw ado context <workItemId> --project <name>` reads the full work item context.
- `dw db schema --project <name> --database <name>` lists database objects when SQL context matters.
- `dw db describe --project <name> --database <name> <table>` shows table columns.
- `dw db query --project <name> --database <name> --max-rows <n> select ...` runs read-only SQL queries.
- `dw task current` prints the active task workspace and branch.
- `dw task sync --continue` refreshes `task.json` from ADO when the local context may be stale.
- `dw task status` lists detected task workspaces.
- `dw task start <workItemId> --project <name> --slug <slug>` creates a task workspace.
- `dw task open --workspace <path>` opens a new agent session for a workspace.
- `dw task open --continue` resumes an existing agent session on the latest workspace.
- `dw task commit --continue --execute` creates an intermediate commit without push or PR.
- `dw task finish --continue --execute --create-pr` is the expected commit/push/PR flow when the user asks to finish.
- `dw db ...` is the only intended SQL entrypoint and is read-only by default.

Current configured root:

```text
{{root}}
```

Important rules:

1. Azure DevOps work items are the source of truth.
2. Read the work item with `dw ado work-item` before coding and use `dw ado context` when you need the full context.
3. Use `dw db schema`, `dw db describe` and `dw db query` when database context can clarify the change.
4. Run `dw task current` before lifecycle actions and before committing to confirm the active workspace.
5. Update `plan.md` in the task workspace before implementing.
6. Use `dw task sync --continue` if the local ADO context may be stale.
7. Use the `dw` CLI for Azure DevOps and worktree operations. Do not use Azure DevOps MCP tools.
8. Commits are created by `dw task commit` or `dw task finish`; do not create them manually.
9. Branches and PR titles are created by `dw task start` and `dw task finish`; do not create them manually.
10. Use `dw` for every ADO, Git naming, PR and worktree operation.
""";
}
