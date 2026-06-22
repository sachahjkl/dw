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
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true
    };

    public static string Serialize(WorkspaceManifest manifest)
        => JsonSerializer.Serialize(manifest, Options);
}

internal static class WorkspaceManifestReader
{
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web);

    public static WorkspaceManifest Read(IFileSystem fileSystem, string path)
    {
        if (!fileSystem.FileExists(path))
        {
            throw new DwException($"Manifest task introuvable: {path}");
        }

        return JsonSerializer.Deserialize<WorkspaceManifest>(fileSystem.ReadAllText(path), Options)
               ?? throw new DwException($"Manifest task invalide: {path}");
    }
}
