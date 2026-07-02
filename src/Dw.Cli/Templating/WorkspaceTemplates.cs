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
3. Read `dw ado ai-context <id> --project {{project}}` before acting on ADO context.
4. Use `dw db schema`, `dw db describe` and `dw db query` when database context can clarify the change.
5. Before working, make sure the initial project setup required by the environment is in place: install or restore dependencies, approve required build scripts, and initialize the basic local prerequisites.
6. Fill `plan.md` before implementing.
7. Run `dw task preflight --continue` before implementation, child-task creation, or other irreversible work. If it reports blockers or warnings, surface them to the user before forcing ahead.
8. Run `dw task handoff-validate --continue` before launching sub-agents and before `dw task finish`. If it fails, complete or correct the handoffs first.
9. If the primary work item is a `User Story` or an `Anomalie`, once `plan.md` is complete and before implementation starts, create at least one ADO child task, then as many as needed from the plan, with `dw task create-child-task --continue --repo <front|back|db|foo> --title "<action explicite>"`.
10. Multiple child tasks can target the same domain/repo when the plan needs it, for example several `front` tasks.
11. For these child tasks, use explicit French titles without the prefix in the command. `dw` adds the prefix automatically, for example `[FRONT] Ajouter le formulaire`, `[BACK] Ajouter endpoint`, `[DB] Ajouter vue`, `[FOO] Faire ...`.
12. Write all user-facing and project-facing text in French: plans, comments, commit/PR text, task titles, progress summaries and final explanations. Internal reasoning can stay in any language.
13. Do not skip this child-task creation step for `User Story` / `Anomalie`: the plan must drive the child-task breakdown before implementation.
14. Structure the plan explicitly by domain when possible: front, back, db or other repos. Use sub-agents for independent tracks whenever possible.
15. Use `dw task sync --continue` before lifecycle decisions if the local ADO context may be stale.
16. Use `dw task commit` for intermediate commits.
17. Use `dw task finish` for final push/PR workflows.
18. Use `dw task teardown` or `dw task prune` for cleanup.
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
# Primary execution instructions are loaded from AGENTS.md in this workspace.
""";

    public static string HandoffMd(WorkspaceManifest manifest, string repository) => $$"""
# Handoff {{repository}}

## Contexte

- Projet: `{{manifest.Project}}`
- Repository: `{{repository}}`
- Branche: `{{manifest.BranchName}}`
- Work items parents: {{string.Join(", ", manifest.ParentWorkItems.Select(item => $"`#{item.Id}`"))}}
- Child tasks connus: {{FormatChildTasks(manifest, repository)}}

## Entrées déterministes à relire

1. `task.json`
2. `plan.md`
3. `AGENTS.md`
4. `dw ado ai-context <id> --project {{manifest.Project}}` pour chaque work item parent
5. `dw task preflight --continue`

## Objectif du lot

Décrire ici, dans `plan.md`, ce qui relève de `{{repository}}` et ce qui doit être traité par ce handoff.

## Contraintes

- Préserver les labels métier exacts
- Tout texte user/projet en français
- Traiter screenshots / maquettes / pièces jointes comme source factuelle
- Demander au user au lieu de deviner si le contexte manque
- Vérifier les impacts API contrat front/back quand pertinent

## Travail attendu

- Limiter le travail à `{{repository}}`
- Lister clairement les fichiers/zonings impactés
- Signaler les dépendances vers d'autres domaines
- Mettre à jour la synthèse structurée ci-dessous

## Synthèse structurée attendue

Remplir ce bloc sans changer les labels.

```yaml
status: todo
repository: {{repository}}
summary:
  done: []
  decisions: []
  risks: []
  blockers: []
  follow_up: []
verification:
  commands: []
  manual_checks: []
artifacts:
  files: []
  screenshots: []
  attachments: []
```
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

    private static string FormatChildTasks(WorkspaceManifest manifest, string repository)
    {
        var matching = manifest.NormalizedChildTasks
            .Where(task => string.Equals(task.Repository, repository, StringComparison.OrdinalIgnoreCase))
            .Select(task => string.IsNullOrWhiteSpace(task.Title)
                ? $"`#{task.Id}`"
                : $"`#{task.Id}` {task.Title}")
            .ToArray();
        return matching.Length == 0 ? "(aucune)" : string.Join(", ", matching);
    }
}
