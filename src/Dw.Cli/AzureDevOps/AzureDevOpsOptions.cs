namespace Dw.Cli.AzureDevOps;

internal sealed record AzureDevOpsOptions(
    string OrganizationUrl,
    string? Project = null,
    string ApiVersion = "7.1");

internal static class AzureDevOpsUris
{
    public static Uri WorkItem(AzureDevOpsOptions options, string workItemId)
        => Build(options, $"{ProjectSegment(options)}/_apis/wit/workitems/{workItemId}?api-version={options.ApiVersion}");

    public static Uri WorkItemExpanded(AzureDevOpsOptions options, string workItemId)
        => Build(options, $"{ProjectSegment(options)}/_apis/wit/workitems/{workItemId}?$expand=all&api-version={options.ApiVersion}");

    public static Uri WorkItemApiUrl(AzureDevOpsOptions options, string workItemId)
        => Build(options, $"{ProjectSegment(options)}/_apis/wit/workItems/{workItemId}");

    public static Uri WorkItemComments(AzureDevOpsOptions options, string workItemId, int top, string? continuationToken = null)
    {
        var continuation = string.IsNullOrWhiteSpace(continuationToken)
            ? string.Empty
            : $"&continuationToken={Uri.EscapeDataString(continuationToken)}";
        return Build(options, $"{ProjectSegment(options)}/_apis/wit/workItems/{workItemId}/comments?$top={top}&order=asc&$expand=renderedText&api-version=7.1-preview.4{continuation}");
    }

    public static Uri CreateWorkItem(AzureDevOpsOptions options, string workItemType)
        => Build(options, $"{ProjectSegment(options)}/_apis/wit/workitems/${Uri.EscapeDataString(workItemType)}?api-version={options.ApiVersion}");

    public static Uri PullRequests(AzureDevOpsOptions options, string repositoryIdOrName)
        => Build(options, $"{ProjectSegment(options)}/_apis/git/repositories/{Uri.EscapeDataString(repositoryIdOrName)}/pullrequests?api-version={options.ApiVersion}");

    public static Uri PullRequestWorkItems(AzureDevOpsOptions options, string repositoryIdOrName, int pullRequestId)
        => Build(options, $"{ProjectSegment(options)}/_apis/git/repositories/{Uri.EscapeDataString(repositoryIdOrName)}/pullRequests/{pullRequestId}/workitems?api-version={options.ApiVersion}");

    private static Uri Build(AzureDevOpsOptions options, string relative)
    {
        if (string.IsNullOrWhiteSpace(options.OrganizationUrl))
        {
            throw new DwException("organizationUrl Azure DevOps manquante dans workflow.json.");
        }

        var baseUri = options.OrganizationUrl.EndsWith("/", StringComparison.Ordinal)
            ? options.OrganizationUrl
            : options.OrganizationUrl + "/";

        return new Uri(new Uri(baseUri), relative);
    }

    private static string ProjectSegment(AzureDevOpsOptions options)
    {
        if (string.IsNullOrWhiteSpace(options.Project))
        {
            throw new DwException("Projet Azure DevOps manquant. Le definir dans projects.json pour le projet courant, ou passer --project.");
        }

        return Uri.EscapeDataString(options.Project);
    }
}
