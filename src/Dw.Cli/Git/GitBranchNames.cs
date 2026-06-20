namespace Dw.Cli.Git;

internal static class GitBranchNames
{
    public static string Build(string type, string workItemId, string? taskId, string slug)
    {
        var cleanType = string.IsNullOrWhiteSpace(type) ? "feat" : type.Trim().ToLowerInvariant();
        var cleanSlug = Slug.Normalize(slug);
        var idPart = string.IsNullOrWhiteSpace(taskId)
            ? workItemId
            : $"{workItemId}-{taskId}";

        return $"{cleanType}/{idPart}-{cleanSlug}";
    }

    public static string BuildSubjectName(string type, string workItemId, string slug)
    {
        var cleanType = string.IsNullOrWhiteSpace(type) ? "feat" : type.Trim().ToLowerInvariant();
        return $"{cleanType}-{workItemId}-{Slug.Normalize(slug)}";
    }
}
