using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Dw.Cli.AzureDevOps;

internal sealed class AzureDevOpsClient(HttpClient httpClient, AzureDevOpsOptions options)
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true
    };

    public async Task<JsonDocument> GetWorkItemAsync(
        string workItemId,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Get, AzureDevOpsUris.WorkItem(options, workItemId), token);
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    public async Task<JsonDocument> GetWorkItemExpandedAsync(
        string workItemId,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Get, AzureDevOpsUris.WorkItemExpanded(options, workItemId), token);
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    public async Task<JsonDocument> GetWorkItemCommentsAsync(
        string workItemId,
        int top,
        string? continuationToken,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Get, AzureDevOpsUris.WorkItemComments(options, workItemId, top, continuationToken), token);
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    public async Task<WorkItemSnapshot> GetWorkItemSnapshotAsync(
        string workItemId,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var document = await GetWorkItemAsync(workItemId, token, cancellationToken);
        return WorkItemSnapshot.From(document.RootElement);
    }

    public async Task<JsonDocument> UpdateWorkItemAsync(
        string workItemId,
        IReadOnlyList<JsonPatchOperation> operations,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Patch, AzureDevOpsUris.WorkItem(options, workItemId), token);
        request.Content = new StringContent(JsonSerializer.Serialize(operations, JsonOptions), Encoding.UTF8, "application/json-patch+json");
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    public async Task<JsonDocument> CreateWorkItemAsync(
        string workItemType,
        IReadOnlyList<JsonPatchOperation> operations,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Post, AzureDevOpsUris.CreateWorkItem(options, workItemType), token);
        request.Content = new StringContent(JsonSerializer.Serialize(operations, JsonOptions), Encoding.UTF8, "application/json-patch+json");
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    public async Task<JsonDocument> CreatePullRequestAsync(
        string repositoryIdOrName,
        CreatePullRequestRequest pullRequest,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Post, AzureDevOpsUris.PullRequests(options, repositoryIdOrName), token);
        request.Content = new StringContent(JsonSerializer.Serialize(pullRequest, JsonOptions), Encoding.UTF8, "application/json");
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    public async Task<JsonDocument> LinkWorkItemToPullRequestAsync(
        string repositoryIdOrName,
        int pullRequestId,
        string workItemId,
        TokenResult token,
        CancellationToken cancellationToken = default)
    {
        using var request = CreateRequest(HttpMethod.Patch, AzureDevOpsUris.PullRequestWorkItems(options, repositoryIdOrName, pullRequestId), token);
        request.Content = new StringContent(
            JsonSerializer.Serialize(new[] { new ResourceRef(workItemId) }, JsonOptions),
            Encoding.UTF8,
            "application/json");
        using var response = await httpClient.SendAsync(request, cancellationToken);
        return await ReadJsonOrThrow(response, cancellationToken);
    }

    private static HttpRequestMessage CreateRequest(HttpMethod method, Uri uri, TokenResult token)
    {
        var request = new HttpRequestMessage(method, uri);
        request.Headers.Authorization = token.Scheme == AzureDevOpsAuthenticationScheme.Basic
            ? new AuthenticationHeaderValue("Basic", Convert.ToBase64String(Encoding.ASCII.GetBytes($":{token.AccessToken}")))
            : new AuthenticationHeaderValue("Bearer", token.AccessToken);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        return request;
    }

    private static async Task<JsonDocument> ReadJsonOrThrow(HttpResponseMessage response, CancellationToken cancellationToken)
    {
        var content = await response.Content.ReadAsStringAsync(cancellationToken);
        if (!response.IsSuccessStatusCode)
        {
            throw new DwException($"Azure DevOps HTTP {(int)response.StatusCode}: {content}");
        }

        return JsonDocument.Parse(content);
    }
}

internal sealed record CreatePullRequestRequest(
    string SourceRefName,
    string TargetRefName,
    string Title,
    string Description,
    bool IsDraft,
    IReadOnlyList<ResourceRef>? WorkItemRefs = null);

internal sealed record ResourceRef(string Id, string? Url = null);

internal sealed record JsonPatchOperation(
    [property: JsonPropertyName("op")] string Operation,
    [property: JsonPropertyName("path")] string Path,
    [property: JsonPropertyName("value")] object? Value = null,
    [property: JsonPropertyName("from")] string? From = null);

internal sealed record WorkItemSnapshot(
    string Id,
    string? Type,
    string? State,
    string? Title,
    string? Url)
{
    public static WorkItemSnapshot From(JsonElement element)
    {
        var id = element.TryGetProperty("id", out var idProperty)
            ? idProperty.GetRawText().Trim('"')
            : string.Empty;
        var url = element.TryGetProperty("url", out var urlProperty) ? urlProperty.GetString() : null;
        string? type = null;
        string? state = null;
        string? title = null;

        if (element.TryGetProperty("fields", out var fields))
        {
            type = GetField(fields, "System.WorkItemType");
            state = GetField(fields, "System.State");
            title = GetField(fields, "System.Title");
        }

        return new WorkItemSnapshot(id, type, state, title, url);
    }

    private static string? GetField(JsonElement fields, string name)
        => fields.TryGetProperty(name, out var value) ? value.GetString() : null;
}
