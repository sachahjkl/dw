namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public const string AgentsMd = """
# DevWorkflow Rules

This workspace is managed by `dw`.

Mandatory rules:

1. Use `dw agent context` before starting an AI workflow.
2. Use Azure DevOps work items as the source of truth for task state.
3. Use one subject workspace per work item.
4. Keep front and back as separate Git repositories.
5. For API contract changes, always check both front and back.
6. Do not commit, push or open PRs unless the user explicitly asks for the finish step.
""";

    public const string OpenCodeJsonc = """
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "permission": {
    "bash": "ask",
    "edit": "ask"
  }
}
""";

    public const string OgfAgentsMd = """
# DevWorkflow OGF Rules

This workspace is managed by `dw`.

Mandatory rules:

1. Run `dw agent context` before starting an AI workflow.
2. Use Azure DevOps work items as the source of truth.
3. Use the skills in the repository references for ADO, Git naming, PRs and HA/HE conventions.
4. Keep front and back as separate Git repositories.
5. Group worktrees for the same subject under one subject workspace.
6. For API contract changes, always check both front and back.
7. Write ADO/PR/commit text in French unless a repository convention says otherwise.
8. Do not bypass `dw task finish` for commit/push/PR workflows.
""";

    public const string OgfOpenCodeJsonc = """
{
  "$schema": "https://opencode.ai/config.json",
  "instructions": [
    "AGENTS.md"
  ],
  "permission": {
    "bash": "ask",
    "edit": "ask"
  },
  "mcp": {
    "ado": {
      "type": "local",
      "command": [
        "npx",
        "-y",
        "@azure-devops/mcp@next",
        "digital-factory-ogf"
      ],
      "environment": {
        "LOG_LEVEL": "debug"
      }
    }
  }
}
""";

    public static string AgentContext(string root) => $$"""
# DevWorkflow agent context

You are working inside a DevWorkflow-managed environment.

Use `dw` for workflow operations:

- `dw doctor` checks local prerequisites.
- `dw task status` lists detected task workspaces.
- `dw task start <workItemId> --project <name> --slug <slug>` creates a task workspace.
- `dw db ...` is the only intended SQL entrypoint and is read-only by default.

Current configured root:

```text
{{root}}
```

Important rules:

1. Azure DevOps work items are the source of truth.
2. Git repositories remain separate per front/back repo.
3. A subject workspace groups related worktrees under one work item.
4. Plans live as `plan.md` in the subject workspace.
5. Branches, commits and PR titles must follow the loaded skills.
6. Never bypass skills when ADO, Git naming, PRs or worktrees are involved.
""";
}
