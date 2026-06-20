namespace Dw.Cli.AzureDevOps;

internal static class PullRequestText
{
    public static string Title(WorkspaceManifest manifest)
    {
        var prefix = manifest.Type.ToLowerInvariant() switch
        {
            "feat" => "feat",
            "fix" => "fix",
            "bug" => "bug",
            "chore" => "chore",
            _ => manifest.Type.ToLowerInvariant()
        };

        var ids = string.IsNullOrWhiteSpace(manifest.TaskId)
            ? $"#{manifest.WorkItemId}"
            : $"#{manifest.WorkItemId} #{manifest.TaskId}";

        return $"{prefix}({ids}): {manifest.Slug}";
    }

    public static string Description(
        WorkspaceManifest manifest,
        RepositoryStatus status,
        string plan,
        IReadOnlyList<VerificationResult> verificationResults)
    {
        var verification = verificationResults.Count == 0
            ? "- Aucune commande configuree dans `taskFinish.verificationCommands`."
            : string.Join(Environment.NewLine, verificationResults
                .Where(result => result.Repository.Equals(status.Repository, StringComparison.OrdinalIgnoreCase))
                .Select(result => $"- `{result.Command}`: {(result.ExitCode == 0 ? "OK" : "KO")}"));

        var planSection = string.IsNullOrWhiteSpace(plan)
            ? "_Plan non trouve._"
            : plan.Trim();

        return $"""
## Résumé
- Travail réalisé pour `{manifest.Slug}`
- Dépôt concerné : `{status.Repository}`
- Work item : `#{manifest.WorkItemId}`

## Plan
{planSection}

## Vérifications
{verification}
""";
    }

    public static string Description(WorkspaceManifest manifest, RepositoryStatus status)
        => $"""
## Résumé
- Travail réalisé pour `{manifest.Slug}`
- Dépôt concerné : `{status.Repository}`

## Vérifications
- À compléter avec les vérifications exécutées localement
""";
}
