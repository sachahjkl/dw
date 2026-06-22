using System.Text.Json.Serialization;

namespace Dw.Cli.Serialization;

[JsonSerializable(typeof(UserSettings))]
[JsonSerializable(typeof(WorkflowConfig))]
[JsonSerializable(typeof(ProjectConfig))]
[JsonSerializable(typeof(DatabasesConfig))]
[JsonSerializable(typeof(WorkspaceManifest))]
[JsonSerializable(typeof(GitHubRelease))]
[JsonSerializable(typeof(List<GitHubRelease>))]
[JsonSerializable(typeof(ReleaseManifest))]
[JsonSerializable(typeof(WiqlRequest))]
[JsonSerializable(typeof(WorkItemsBatchRequest))]
[JsonSerializable(typeof(JsonPatchOperation[]))]
[JsonSerializable(typeof(List<JsonPatchOperation>))]
[JsonSerializable(typeof(CreatePullRequestRequest))]
[JsonSerializable(typeof(ResourceRef[]))]
[JsonSerializable(typeof(WorkItemRelationRef))]
[JsonSerializable(typeof(WorkItemRelationAttributes))]
[JsonSerializable(typeof(TaskListItem[]))]
[JsonSerializable(typeof(CompletionSuggestion[]))]
[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase,
    WriteIndented = false,
    ReadCommentHandling = JsonCommentHandling.Skip,
    AllowTrailingCommas = true,
    UseStringEnumConverter = true)]
internal sealed partial class AppJsonContext : JsonSerializerContext;

internal sealed record WiqlRequest(string Query);

internal sealed record WorkItemsBatchRequest(int[] Ids, string[] Fields);

internal sealed record CompletionSuggestion(string Label, string InsertText, string Description);
