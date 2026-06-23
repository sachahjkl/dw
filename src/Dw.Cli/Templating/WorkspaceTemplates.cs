namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public static string WorkspaceAgentsMd(string workItemId, string project) => $$"""
# DevWorkflow Workspace

This workspace is managed by `dw`.

Context:

- Project: `{{project}}`
- Work item: `{{workItemId}}`

Rules:

1. Run `dw task current` to identify the current task workspace.
2. Read the work item with `dw ado work-item {{workItemId}} --project {{project}}` before coding.
3. Use `dw ado context {{workItemId}} --project {{project}}` when you need the full ADO context.
4. Use `dw db schema`, `dw db describe` and `dw db query` when database context can clarify the change.
5. Fill `plan.md` before implementing.
6. Use `dw task sync --continue` before lifecycle decisions if the local ADO context may be stale.
7. Use `dw task commit` for intermediate commits.
8. Use `dw task finish` for final push/PR workflows.
9. Use `dw task teardown` or `dw task prune` for cleanup.
""";

    public static string WorkspaceClaudeMd(string workItemId, string project)
        => WorkspaceAgentsMd(workItemId, project);

    public static string WorkspaceCursorRule(string workItemId, string project) => $$"""
---
alwaysApply: true
---

{{WorkspaceAgentsMd(workItemId, project)}}
""";

    public static string WorkspaceCodexConfig => """
# Project-local Codex config placeholder.
# Shared instructions are loaded from AGENTS.md.
""";

    public static string WorkspaceCopilotInstructions(string workItemId, string project)
        => WorkspaceAgentsMd(workItemId, project);
}
