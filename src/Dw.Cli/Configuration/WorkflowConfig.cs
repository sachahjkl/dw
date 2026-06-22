using System.Text.Json;
using System.Text.Json.Nodes;
using Dw.Cli.Agents;

namespace Dw.Cli.Configuration;

internal sealed record WorkflowConfig(
    AzureDevOpsOptions? AzureDevOps,
    AuthOptions? Auth,
    UpdateOptions? Updates,
    IReadOnlyDictionary<string, string> BranchPrefixes,
    AgentOptions? Agent = null,
    TaskStartOptions? TaskStart = null,
    TaskFinishOptions? TaskFinish = null)
{
    public static WorkflowConfig Empty { get; } =
        new(null, null, null, new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase));
}

internal sealed record AuthOptions(
    string? TenantId,
    string? ClientId,
    string[] Scopes);

internal sealed record UpdateOptions(
    string? Owner,
    string? Repository,
    bool IncludePrerelease = false,
    string AssetName = UpdateDefaults.ManifestAssetName);

internal sealed record AgentOptions(string Default = AgentDefaults.DefaultAgent);

internal sealed record TaskStartOptions(
    bool UpdateWorkItemState = true,
    bool CreateChildTasks = false,
    string UserStoryState = "En réalisation",
    string AnomalyState = "En réalisation",
    string BugState = "En développement",
    string TaskState = "En développement");

internal sealed record TaskFinishOptions(
    bool RunVerification = true,
    bool UpdateWorkItemState = true,
    string BugState = "PR en attente",
    string TaskState = "PR en attente",
    IReadOnlyDictionary<string, string[]>? VerificationCommands = null);

internal static class WorkflowConfigStore
{
    public static WorkflowConfig Load(IFileSystem fileSystem, string root)
    {
        var path = Path.Combine(root, "config", "workflow.json");
        if (!fileSystem.FileExists(path))
        {
            return WorkflowConfig.Empty;
        }

        var json = fileSystem.ReadAllText(path);
        return JsonSerializer.Deserialize(json, AppJsonContext.Default.WorkflowConfig) ?? WorkflowConfig.Empty;
    }

    public static void SetDefaultAgent(IFileSystem fileSystem, string root, string agent)
    {
        var path = Path.Combine(root, "config", "workflow.json");
        if (!fileSystem.FileExists(path))
        {
            throw new DwException($"workflow.json introuvable: {path}", 2);
        }

        _ = AgentAdapterRegistry.Resolve(agent);
        var node = JsonNode.Parse(fileSystem.ReadAllText(path))?.AsObject()
            ?? throw new DwException($"workflow.json invalide: {path}", 2);
        var agentNode = node["agent"]?.AsObject() ?? new JsonObject();
        agentNode["default"] = agent;
        node["agent"] = agentNode;
        fileSystem.WriteAllText(path, node.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
    }
}
