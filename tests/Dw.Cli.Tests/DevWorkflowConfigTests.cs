namespace Dw.Cli.Tests;

public sealed class DevWorkflowConfigTests
{
    [Fact]
    public void ResolveProject_merges_repositories_from_included_projects()
    {
        var config = new DevWorkflowConfig(new Dictionary<string, ProjectConfig>(StringComparer.OrdinalIgnoreCase)
        {
            ["ha"] = new(
                "HA",
                new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase)
                {
                    ["front"] = new("ha-front", "develop", Folder: "front")
                },
                new AzureDevOpsOptions("https://dev.azure.com/org", "HA")),
            ["he"] = new(
                "HE",
                new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase)
                {
                    ["back"] = new("he-back", "develop", Folder: "back")
                },
                new AzureDevOpsOptions("https://dev.azure.com/org", "HE")),
            ["cross"] = new(
                "Cross",
                new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase),
                null,
                ["ha", "he"])
        });

        var resolved = DevWorkflowConfigLoader.ResolveProject(config, "cross");

        Assert.NotNull(resolved);
        Assert.Contains("front", resolved.Repositories.Keys);
        Assert.Contains("back", resolved.Repositories.Keys);
        Assert.Null(resolved.AzureDevOps);
    }
}
