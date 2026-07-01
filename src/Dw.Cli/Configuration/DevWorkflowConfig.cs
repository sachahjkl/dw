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
    AzureDevOpsOptions? AzureDevOps = null,
    string[]? IncludedProjects = null,
    AgentOptions? Agent = null);

internal sealed record RepositoryConfig(
    string Url,
    string DefaultBranch,
    string? PullRequestTargetBranch = null,
    string? AzureDevOpsRepository = null,
    string? AnchorName = null,
    string? Folder = null);

internal static class DevWorkflowConfigLoader
{
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
            var project = projectProperty.Value.Deserialize(AppJsonContext.Default.ProjectConfig);
            if (project is not null)
            {
                projects[projectProperty.Name] = project;
            }
        }

        return new DevWorkflowConfig(projects);
    }

    public static ProjectConfig? ResolveProject(DevWorkflowConfig config, string project)
        => ResolveProject(config, project, new HashSet<string>(StringComparer.OrdinalIgnoreCase));

    private static ProjectConfig? ResolveProject(DevWorkflowConfig config, string project, HashSet<string> visited)
    {
        if (!visited.Add(project))
        {
            throw new DwException($"Boucle de composition detectee dans projects.json pour {project}.");
        }

        if (!config.Projects.TryGetValue(project, out var projectConfig))
        {
            return null;
        }

        var repositories = new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase);
        foreach (var includedProject in projectConfig.IncludedProjects ?? [])
        {
            var includedConfig = ResolveProject(config, includedProject, visited)
                ?? throw new DwException($"Projet inclus introuvable dans projects.json: {includedProject}");

            foreach (var repository in includedConfig.Repositories)
            {
                repositories[repository.Key] = repository.Value;
            }
        }

        foreach (var repository in projectConfig.Repositories)
        {
            repositories[repository.Key] = repository.Value;
        }

        return projectConfig with { Repositories = repositories };
    }
}
