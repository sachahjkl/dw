using System.Text.Json;

namespace Dw.Cli.Workspaces;

internal sealed record WorkspaceManifest(
    int Schema,
    string WorkItemId,
    string? TaskId,
    string Project,
    string Type,
    string Slug,
    string BranchName,
    DateTimeOffset CreatedAt,
    IReadOnlyList<string> Repositories,
    string Status,
    string? WorkItemType = null,
    string? WorkItemTitle = null,
    string? WorkItemState = null,
    IReadOnlyDictionary<string, string>? ChildTaskIds = null);

internal static class WorkspaceManifestWriter
{
    public static string Serialize(WorkspaceManifest manifest)
        => JsonSerializer.Serialize(manifest, AppJsonContext.Default.WorkspaceManifest);
}

internal static class WorkspaceManifestReader
{
    public static WorkspaceManifest Read(IFileSystem fileSystem, string path)
    {
        if (!fileSystem.FileExists(path))
        {
            throw new DwException($"Manifest task introuvable: {path}");
        }

        return JsonSerializer.Deserialize(fileSystem.ReadAllText(path), AppJsonContext.Default.WorkspaceManifest)
               ?? throw new DwException($"Manifest task invalide: {path}");
    }
}
