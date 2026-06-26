namespace Dw.Cli.Git;

internal static class GitBranchNames
{
    public static string Build(string type, string workItemId, string? taskId, string slug)
        => Build(type, string.IsNullOrWhiteSpace(taskId) ? [workItemId] : [workItemId, taskId], slug);

    public static string Build(string type, IReadOnlyList<string> workItemIds, string slug)
    {
        var cleanType = string.IsNullOrWhiteSpace(type) ? "feat" : type.Trim().ToLowerInvariant();
        var cleanSlug = Slug.Normalize(slug);
        var idPart = string.Join('-', workItemIds.Where(id => !string.IsNullOrWhiteSpace(id)).Distinct(StringComparer.OrdinalIgnoreCase));

        return $"{cleanType}/{idPart}-{cleanSlug}";
    }

    public static string BuildSubjectName(string type, string workItemId, string slug)
        => BuildSubjectName(type, [workItemId], slug);

    public static string BuildSubjectName(string type, IReadOnlyList<string> workItemIds, string slug)
    {
        var cleanType = string.IsNullOrWhiteSpace(type) ? "feat" : type.Trim().ToLowerInvariant();
        var idPart = string.Join('-', workItemIds.Where(id => !string.IsNullOrWhiteSpace(id)).Distinct(StringComparer.OrdinalIgnoreCase));
        return $"{cleanType}-{idPart}-{Slug.Normalize(slug)}";
    }
}
