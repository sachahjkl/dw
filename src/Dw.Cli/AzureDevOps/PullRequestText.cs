namespace Dw.Cli.AzureDevOps;

internal static class PullRequestText
{
    public static string Title(WorkspaceManifest manifest)
        => CommitMessage.Build(manifest);

    public static string Description(
        WorkspaceManifest manifest,
        RepositoryStatus status,
        string plan,
        IReadOnlyList<VerificationResult> verificationResults,
        WorkspaceHandoffSummary handoff)
    {
        var verification = verificationResults.Count == 0
            ? "- Aucune commande configuree dans `taskFinish.verificationCommands`."
            : string.Join(Environment.NewLine, verificationResults
                .Where(result => result.Repository.Equals(status.Repository, StringComparison.OrdinalIgnoreCase))
                .Select(result => $"- `{result.Command}`: {(result.ExitCode == 0 ? "OK" : "KO")}"));

        var planSection = string.IsNullOrWhiteSpace(plan)
            ? "_Plan non trouve._"
            : plan.Trim();
        var handoffSection = StructuredHandoffSection(handoff);

        return $"""
## Résumé
- Travail réalisé pour `{manifest.Slug}`
- Dépôt concerné : `{status.Repository}`
- Work items : `{string.Join(", ", manifest.AllKnownWorkItemIds.Select(id => $"#{id}"))}`

## Plan
{planSection}

## Handoff
{handoffSection}

## Vérifications
{verification}
""";
    }

    internal static string StructuredHandoffSection(WorkspaceHandoffSummary handoff)
        => $"""
### Statut
- `{handoff.Status}`

### Travail Fait
{RenderList(handoff.Done)}

### Décisions
{RenderList(handoff.Decisions)}

### Risques
{RenderList(handoff.Risks)}

### Blockers
{RenderList(handoff.Blockers)}

### Follow-up
{RenderList(handoff.FollowUp)}

### Artifacts
- Fichiers: {RenderInline(handoff.Files)}
- Screenshots: {RenderInline(handoff.Screenshots)}
- Pièces jointes: {RenderInline(handoff.Attachments)}

### Vérifications Manuelles Déclarées
{RenderList(handoff.ManualChecks)}
""";

    private static string RenderList(IReadOnlyList<string> items)
        => items.Count == 0
            ? "- (aucun)"
            : string.Join(Environment.NewLine, items.Select(item => $"- {item}"));

    private static string RenderInline(IReadOnlyList<string> items)
        => items.Count == 0 ? "(aucun)" : string.Join(", ", items);

    public static string Description(WorkspaceManifest manifest, RepositoryStatus status)
        => $"""
## Résumé
- Travail réalisé pour `{manifest.Slug}`
- Dépôt concerné : `{status.Repository}`

## Vérifications
- À compléter avec les vérifications exécutées localement
""";
}
