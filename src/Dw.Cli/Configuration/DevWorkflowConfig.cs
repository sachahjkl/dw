using System.Text.Json;
using System.Text.Json.Serialization;

namespace Dw.Cli.Configuration;

internal sealed record DevWorkflowConfig(IReadOnlyDictionary<string, ProjectConfig> Projects)
{
    public static DevWorkflowConfig Empty { get; } = new(new Dictionary<string, ProjectConfig>(StringComparer.OrdinalIgnoreCase));
}

internal sealed record ProjectConfig(
    string DisplayName,
    IReadOnlyDictionary<string, RepositoryConfig> Repositories,
    AzureDevOpsOptions? AzureDevOps = null);

internal sealed record RepositoryConfig(
    string Url,
    string DefaultBranch,
    string? PullRequestTargetBranch = null,
    string? AzureDevOpsRepository = null,
    string? AnchorName = null,
    string? Folder = null);

internal static class DevWorkflowConfigLoader
{
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        ReadCommentHandling = JsonCommentHandling.Skip,
        AllowTrailingCommas = true,
        Converters = { new JsonStringEnumConverter() }
    };

    public static DevWorkflowConfig Load(IFileSystem fileSystem, string root)
    {
        var path = Path.Combine(root, "config", "projects.json");
        if (!fileSystem.FileExists(path))
        {
            return DevWorkflowConfig.Empty;
        }

        using var document = JsonDocument.Parse(fileSystem.ReadAllText(path), new JsonDocumentOptions
        {
            AllowTrailingCommas = true,
            CommentHandling = JsonCommentHandling.Skip
        });

        if (!document.RootElement.TryGetProperty("projects", out var projectsElement) ||
            projectsElement.ValueKind != JsonValueKind.Object)
        {
            return DevWorkflowConfig.Empty;
        }

        var projects = new Dictionary<string, ProjectConfig>(StringComparer.OrdinalIgnoreCase);
        foreach (var projectProperty in projectsElement.EnumerateObject())
        {
            var project = projectProperty.Value.Deserialize<ProjectConfig>(Options);
            if (project is not null)
            {
                projects[projectProperty.Name] = project;
            }
        }

        return new DevWorkflowConfig(projects);
    }
}
