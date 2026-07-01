namespace Dw.Cli.Templating;

internal static partial class Templates
{
    public static string WorkspaceAgentsMd(IReadOnlyList<WorkspaceWorkItem> workItems, string project) => $$"""
# DevWorkflow Workspace

This workspace is managed by `dw`.

Context:

- Project: `{{project}}`
- Work items:
{{FormatWorkItems(workItems)}}

Rules:

1. Run `dw task current` to identify the current task workspace.
2. Read each work item with `dw ado work-item <id> --project {{project}}` before coding.
3. Use `dw ado context <id> --project {{project}}` when you need the full ADO context.
4. Use `dw db schema`, `dw db describe` and `dw db query` when database context can clarify the change.
5. Before working, make sure the initial project setup required by the environment is in place: install or restore dependencies, approve required build scripts, and initialize the basic local prerequisites.
6. Fill `plan.md` before implementing.
7. If the primary work item is a `User Story` or an `Anomalie`, once `plan.md` is complete and before implementation starts, create the required ADO child tasks with `dw task create-child-task --continue --repo <front|back|db|foo> --title "<action explicite>"`.
8. For these child tasks, use explicit French titles without the prefix in the command. `dw` adds the prefix automatically, for example `[FRONT] Ajouter le formulaire`, `[BACK] Ajouter endpoint`, `[DB] Ajouter vue`, `[FOO] Faire ...`.
9. Do not skip this child-task creation step for `User Story` / `Anomalie`: the plan must drive the child-task breakdown before implementation.
10. Use `dw task sync --continue` before lifecycle decisions if the local ADO context may be stale.
11. Use `dw task commit` for intermediate commits.
12. Use `dw task finish` for final push/PR workflows.
13. Use `dw task teardown` or `dw task prune` for cleanup.
""";

    public static string WorkspaceClaudeMd(IReadOnlyList<WorkspaceWorkItem> workItems, string project)
        => WorkspaceAgentsMd(workItems, project);

    public static string WorkspaceCursorRule(IReadOnlyList<WorkspaceWorkItem> workItems, string project) => $$"""
---
alwaysApply: true
---

{{WorkspaceAgentsMd(workItems, project)}}
""";

    public static string WorkspaceCodexConfig => """
# Project-local Codex config placeholder.
# Shared instructions are loaded from AGENTS.md.
""";

    public static string WorkspaceCopilotInstructions(IReadOnlyList<WorkspaceWorkItem> workItems, string project)
        => WorkspaceAgentsMd(workItems, project);

    private static string FormatWorkItems(IReadOnlyList<WorkspaceWorkItem> workItems)
        => string.Join(Environment.NewLine, workItems.Select(item =>
        {
            var suffix = string.IsNullOrWhiteSpace(item.Type) && string.IsNullOrWhiteSpace(item.Title)
                ? string.Empty
                : $" [{item.Type ?? "?"}] {item.Title ?? string.Empty}".TrimEnd();
            return $"  - `#{item.Id}`{suffix}";
        }));
}
