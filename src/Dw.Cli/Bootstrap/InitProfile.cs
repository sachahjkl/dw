namespace Dw.Cli.Bootstrap;

internal sealed record InitProfile(
    string Name,
    string ProjectsJson,
    string WorkflowJson,
    string DatabasesJson,
    string AgentsMd,
    string OpenCodeJsonc)
{
    public static InitProfile Detect(IFileSystem fileSystem, string root)
    {
        var projectsPath = Path.Combine(root, "config", "projects.json");
        if (fileSystem.FileExists(projectsPath))
        {
            var content = fileSystem.ReadAllText(projectsPath);
            if (content.Contains("\"ha\"", StringComparison.OrdinalIgnoreCase) ||
                content.Contains("digital-factory-ogf", StringComparison.OrdinalIgnoreCase) ||
                content.Contains("HOMMAGE", StringComparison.OrdinalIgnoreCase))
            {
                return Resolve("ogf");
            }
        }

        return Resolve("default");
    }

    public static InitProfile Resolve(string? name)
    {
        var normalized = string.IsNullOrWhiteSpace(name) ? "ogf" : name.Trim().ToLowerInvariant();
        return normalized switch
        {
            "default" => new InitProfile("default", Templates.DefaultProjectsJson, Templates.DefaultWorkflowJson, Templates.DefaultDatabasesJson, Templates.AgentsMd, Templates.OpenCodeJsonc),
            "ogf" => new InitProfile("ogf", Templates.OgfProjectsJson, Templates.OgfWorkflowJson, Templates.OgfDatabasesJson, Templates.OgfAgentsMd, Templates.OgfOpenCodeJsonc),
            _ => throw new DwException($"Profil init inconnu: {name}. Profils disponibles: ogf, default.", 2)
        };
    }
}
