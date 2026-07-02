namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public const string AgentsMd = """
# DevWorkflow Global Rules

This root is managed by `dw`.

Global rules:

1. Use Azure DevOps work items as the source of truth.
2. Use only `dw ado ...`, `dw auth ...` and `dw task ...` for Azure DevOps/worktree operations; do not use Azure DevOps MCP tools.
3. Once inside a task workspace, follow the local `AGENTS.md` there as the primary execution contract.
4. Write all user-facing and project-facing text in French unless a repository convention says otherwise.
5. Do not normalize business labels or domain wording from ADO, screenshots, mockups, attachments or project text. Preserve the exact terms unless the user explicitly asks to rename them.
6. Treat screenshots, mockups and attachments as factual source material. If something is ambiguous, ask the user instead of guessing.
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

    public const string BusinessAgentsMd = """
# DevWorkflow BUSINESS Global Rules

This root is managed by `dw`.

Global rules:

1. Use Azure DevOps work items as the source of truth.
2. Use only `dw ado ...`, `dw auth ...` and `dw task ...` for Azure DevOps/worktree operations; do not use Azure DevOps MCP tools.
3. Once inside a task workspace, follow the local `AGENTS.md` there as the primary execution contract.
4. Write all user-facing and project-facing text in French unless a repository convention says otherwise.
5. Do not normalize business labels or domain wording from ADO, screenshots, mockups, attachments or project text. Preserve the exact terms unless the user explicitly asks to rename them.
6. Treat screenshots, mockups and attachments as factual source material. If something is ambiguous, ask the user instead of guessing.
""";

    public const string BusinessOpenCodeJsonc = """
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
- `dw ado ai-context <workItemId> --project <name>` reads the deterministic structured work item context for AI consumption.
- `dw task preflight --continue` checks deterministic blockers/warnings before implementation or child-task decomposition.
- `dw task handoff-validate --continue` validates handoff contracts before `task finish` or sub-agent execution.
- `dw db schema --project <name> --database <name>` lists database objects when SQL context matters.
- `dw db describe --project <name> --database <name> <table>` shows table columns.
- `dw db query --project <name> --database <name> --max-rows <n> select ...` runs read-only SQL queries.
- `pnpm install` installs Node dependencies when a workspace uses Node and `pnpm` is available.
- `pnpm approve-builds --all` approves required build scripts for Node workspaces when needed.
- `npm install` is the fallback only when `pnpm` is unavailable.
- `dotnet restore` restores .NET dependencies.
- `dw task current` prints the active task workspace and branch.
- `dw task sync --continue` refreshes `task.json` from ADO when the local context may be stale.
- `dw task status` lists detected task workspaces.
- `dw task start <workItemId> --project <name> --slug <slug>` creates a task workspace.
- `dw task open --workspace <path>` opens a new agent session for a workspace.
- `dw task open --continue` resumes an existing agent session on the latest workspace.
- `dw task create-child-task --continue --repo <front|back|db|foo> --title "<action explicite>"` creates one ADO child task for a `User Story` or `Anomalie` after the plan is written; run it multiple times when the plan needs several tasks, including several on the same domain.
- `dw task commit --continue --execute` creates an intermediate commit without push or PR.
- `dw task finish --continue --execute --create-pr` is the expected commit/push/PR flow when the user asks to finish.
- `dw db ...` is the only intended SQL entrypoint and is read-only by default.

Current configured root:

```text
{{root}}
```

Important rules:

1. Azure DevOps work items are the source of truth.
2. Read the work item with `dw ado work-item` before coding, then run `dw ado ai-context` before acting on ADO context.
3. Use `dw db schema`, `dw db describe` and `dw db query` when database context can clarify the change.
4. Before working, make sure the initial project setup required by the environment is in place: install or restore dependencies, approve required build scripts, and initialize the basic local prerequisites.
5. Run `dw task current` before lifecycle actions and before committing to confirm the active workspace.
6. Update `plan.md` in the task workspace before implementing.
7. For `User Story` and `Anomalie`, once `plan.md` is complete and before implementation starts, create at least one ADO child task, then as many as needed from the plan, with `dw task create-child-task --continue --repo <front|back|db|foo> --title "<action explicite>"`.
8. Multiple child tasks can target the same domain/repo when the plan needs it, for example several `front` tasks.
9. Child-task titles must be explicit and written without the prefix in the command; `dw` adds `[FRONT]`, `[BACK]`, `[DB]`, `[FOO]` automatically.
10. Write all user-facing and project-facing text in French: plans, comments, commit/PR text, task titles, progress summaries and final explanations. Internal reasoning can stay in any language.
11. Run `dw task preflight --continue` before implementation, child-task creation, or other irreversible work. If it reports blockers or warnings, surface them to the user before forcing ahead.
12. Use `dw task sync --continue` if the local ADO context may be stale.
13. Use the `dw` CLI for Azure DevOps and worktree operations. Do not use Azure DevOps MCP tools.
14. Commits are created by `dw task commit` or `dw task finish`; do not create them manually.
15. Branches and PR titles are created by `dw task start` and `dw task finish`; do not create them manually.
16. Use `dw` for every ADO, Git naming, PR and worktree operation.
17. Do not normalize business labels or domain wording from ADO, screenshots, mockups or project text. Preserve the exact terms unless the user explicitly asks to rename them.
18. Treat screenshots, mockups and attachments as factual source material. If something is ambiguous, ask the user instead of guessing.
19. When the plan can be split by domain, structure it explicitly for front, back, db or other repos, then use sub-agents for independent tracks whenever possible.
20. Use proportionate sub-agents/models: small tasks on lighter agents, cross-repo or ambiguous tasks on stronger agents.
""";
}
