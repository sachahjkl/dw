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
4. Use one subject workspace per work item.
5. Keep front and back as separate Git repositories.
6. For API contract changes, always check both front and back.
7. Do not commit, push or open PRs unless the user explicitly asks for the finish step.
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
4. Use `dw` commands for ADO lifecycle, Git naming, worktrees, commits and PRs.
5. Keep front and back as separate Git repositories.
6. Group worktrees for the same subject under one subject workspace.
7. For API contract changes, always check both front and back.
8. Write ADO/PR/commit text in French unless a repository convention says otherwise.
9. Do not bypass `dw task finish` for commit/push/PR workflows.
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
- `dw task status` lists detected task workspaces.
- `dw task start <workItemId> --project <name> --slug <slug>` creates a task workspace.
- `dw task finish --workspace <path> --execute --create-pr` is the expected commit/push/PR flow when the user asks to finish.
- `dw db ...` is the only intended SQL entrypoint and is read-only by default.

Current configured root:

```text
{{root}}
```

Important rules:

1. Azure DevOps work items are the source of truth.
2. Use the `dw` CLI for Azure DevOps and worktree operations. Do not use Azure DevOps MCP tools.
3. Git repositories remain separate per front/back repo.
4. A subject workspace groups related worktrees under one work item.
5. Plans live as `plan.md` in the subject workspace.
6. Branches, commits and PR titles are created by `dw task start` and `dw task finish`; do not create them manually.
7. Use `dw` for every ADO, Git naming, PR and worktree operation.
""";
}
