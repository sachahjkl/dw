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
2. Use `dw task sync` before making lifecycle decisions from ADO state.
3. Use `dw task finish` for commit/push/PR workflows.
4. Use `dw task teardown` or `dw task prune` for cleanup.
5. Keep repository worktrees separate under this subject workspace.
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
