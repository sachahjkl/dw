using System.Text.Json;

namespace Dw.Cli.Configuration;

internal sealed record WorkflowConfig(
    AzureDevOpsOptions? AzureDevOps,
    AuthOptions? Auth,
    UpdateOptions? Updates,
    IReadOnlyDictionary<string, string> BranchPrefixes,
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

internal static class WorkflowConfigLoader
{
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        ReadCommentHandling = JsonCommentHandling.Skip,
        AllowTrailingCommas = true
    };

    public static WorkflowConfig Load(IFileSystem fileSystem, string root)
    {
        var path = Path.Combine(root, "config", "workflow.json");
        if (!fileSystem.FileExists(path))
        {
            return WorkflowConfig.Empty;
        }

        var json = fileSystem.ReadAllText(path);
        return JsonSerializer.Deserialize<WorkflowConfig>(json, Options) ?? WorkflowConfig.Empty;
    }
}
