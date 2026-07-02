using System.Net;

namespace Dw.Cli.Tests;

public sealed class AdoCommandTests
{
#pragma warning disable xUnit1031
    private static readonly AzureDevOpsOptions DefaultOptions = new("https://dev.azure.com/org", "Project");
    private static readonly TokenResult DefaultToken = new("test-token", null, "test", AzureDevOpsAuthenticationScheme.Basic);

    [Fact]
    public void FilterAssignedItems_excludes_final_states_by_default()
    {
        var items = new[]
        {
            new WorkItemSnapshot("1", "Bug", "En développement", "Actif", null),
            new WorkItemSnapshot("2", "Bug", "Clôturé", "Clos", null),
            new WorkItemSnapshot("3", "Activité", "Abandonné", "Abandon", null)
        };

        var filtered = AdoCommand.FilterAssignedItems(items, includeFinalStates: false);

        Assert.Collection(filtered, item => Assert.Equal("1", item.Id));
    }

    [Fact]
    public void FilterAssignedItems_keeps_final_states_with_all_flag()
    {
        var items = new[]
        {
            new WorkItemSnapshot("1", "Bug", "En développement", "Actif", null),
            new WorkItemSnapshot("2", "Bug", "Clôturé", "Clos", null)
        };

        var filtered = AdoCommand.FilterAssignedItems(items, includeFinalStates: true);

        Assert.Equal(["1", "2"], filtered.Select(item => item.Id).ToArray());
    }

    [Fact]
    public void ExtractWorkItemIdsFromCommitMessages_reads_ids_in_order_and_dedupes()
    {
        var commitLog = "fix(#53115 #53312): corriger le calcul\u001erefactor(#53115 #54000): simplifier\u001echore: sans wi";

        var ids = AdoCommand.ExtractWorkItemIdsFromCommitMessages(commitLog);

        Assert.Equal(["53115", "53312", "54000"], ids);
    }

    [Fact]
    public void ExtractWorkItemIdsFromCommitMessages_can_be_joined_as_space_separated_list()
    {
        var commitLog = "fix(#53115 #53312): corriger le calcul\u001erefactor(#53115 #54000): simplifier";

        var ids = AdoCommand.ExtractWorkItemIdsFromCommitMessages(commitLog);

        Assert.Equal("53115 53312 54000", string.Join(' ', ids));
    }

    [Fact]
    public void RenderFlatChangelog_markdown_adds_links_on_work_item_numbers()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf", "HOMMAGE AGENCE");
        var items = new[]
        {
            new WorkItemSnapshot("53115", "Bug", "En développement", "Corriger le calcul", null)
        };

        var markdown = ChangelogRenderer.RenderFlatChangelog(items, ChangelogFormat.Markdown, options);

        Assert.Contains("[#53115](https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115)", markdown);
        Assert.Contains("[Bug] En développement - Corriger le calcul", markdown);
    }

    [Fact]
    public void RenderFlatChangelog_markdown_table_renders_columns_and_links()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf", "HOMMAGE AGENCE");
        var items = new[]
        {
            new WorkItemSnapshot("53115", "Bug", "En développement", "Corriger le calcul", null)
        };

        var markdown = ChangelogRenderer.RenderFlatChangelog(items, ChangelogFormat.Markdown, options, markdownTable: true);

        Assert.Contains("| Work Item | Type | Etat | Titre |", markdown);
        Assert.Contains("| [#53115](https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115) | Bug | En développement | Corriger le calcul |", markdown);
    }

    [Fact]
    public void RenderGroupedChangelog_html_adds_links_on_parent_and_children()
    {
        var options = new AzureDevOpsOptions("https://dev.azure.com/digital-factory-ogf", "HOMMAGE AGENCE");
        var groups = new[]
        {
            new WorkItemGroup(
                new WorkItemSnapshot("53115", "User Story", "En réalisation", "Parent", null),
                [new WorkItemSnapshot("53312", "Task", "En développement", "Enfant", null)])
        };

        var html = ChangelogRenderer.RenderGroupedChangelog(groups, ChangelogFormat.Html, options);

        Assert.Contains("<a href=\"https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115\">#53115</a>", html);
        Assert.Contains("<a href=\"https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53312\">#53312</a>", html);
        Assert.Contains("[Task] En d&#233;veloppement - Enfant", html);
    }

    // -- Changelog pipeline integration tests --

    [Fact]
    public async Task GetWorkItemIdsFromPullRequests_returns_ids_from_pr()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupGet(
            "pullRequests/42/workitems",
            HttpStatusCode.OK,
            """
            {"value":[{"id":53115},{"id":53312}]}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var ids = await client.TryGetPullRequestWorkItemIdsAsync("my-repo", 42, DefaultToken);

        Assert.NotNull(ids);
        Assert.Equal(["53115", "53312"], ids!);
        Assert.Single(handler.CapturedRequests);
    }

    [Fact]
    public async Task TryFindActivePullRequest_returns_matching_active_pr_for_source_branch()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupGet(
            "searchCriteria.status=active",
            HttpStatusCode.OK,
            """
            {"value":[{"pullRequestId":42,"sourceRefName":"refs/heads/feat/demo","url":"https://ado/pr/42"}]}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var pullRequest = await client.TryFindActivePullRequestAsync("my-repo", "refs/heads/feat/demo", DefaultToken);

        Assert.NotNull(pullRequest);
        Assert.Equal(42, pullRequest!.PullRequestId);
        Assert.Equal("https://ado/pr/42", pullRequest.Url);
    }

    [Fact]
    public async Task CreateWorkItem_can_assign_to_me_when_requested_in_patch()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupPost(
            "$Task?api-version=",
            HttpStatusCode.OK,
            """
            {"id":55201}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        using var _ = await client.CreateWorkItemAsync("Task",
            [
                new JsonPatchOperation("add", "/fields/System.Title", "[FRONT] Ajouter le formulaire"),
                new JsonPatchOperation("add", "/fields/System.AssignedTo", "@Me")
            ],
            DefaultToken);

        Assert.Contains(handler.CapturedBodies, body => body.Contains("System.AssignedTo", StringComparison.Ordinal) && body.Contains("@Me", StringComparison.Ordinal));
    }

    [Fact]
    public void DescribeRelationTarget_includes_attachment_url_next_to_name()
    {
        var target = AdoCommand.DescribeRelationTarget("AttachedFile", relatedId: null, artifact: null, name: "demande de transport somotha maquette.png", url: "https://dev.azure.com/org/_apis/wit/attachments/123");

        Assert.Equal("demande de transport somotha maquette.png (https://dev.azure.com/org/_apis/wit/attachments/123)", target);
    }

    [Fact]
    public void GetWorkItemIdsFromPullRequests_throws_when_pr_not_found()
    {
        var handler = new TestAzureDevOpsHttpHandler();

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var ex = Assert.Throws<DwException>(() =>
            AdoCommand.GetWorkItemIdsFromPullRequests(
                client, DefaultToken, projectConfig: null, repository: "my-repo", source: "99"));

        Assert.Contains("introuvable", ex.Message);
    }

    [Fact]
    public async Task Changelog_pipeline_from_pr_to_markdown()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupGet(
            "pullRequests/42/workitems",
            HttpStatusCode.OK,
            """
            {"value":[{"id":53115},{"id":53312}]}
            """);
        handler.SetupPost(
            "workitemsbatch",
            HttpStatusCode.OK,
            """
            {"value":[
              {"id":53115,"fields":{"System.WorkItemType":"Bug","System.State":"Active","System.Title":"Fix critical bug"},"url":"https://dev.azure.com/org/Project/_apis/wit/workItems/53115"},
              {"id":53312,"fields":{"System.WorkItemType":"Task","System.State":"In Progress","System.Title":"Implement feature"},"url":"https://dev.azure.com/org/Project/_apis/wit/workItems/53312"}
            ]}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var workItemIds = await client.TryGetPullRequestWorkItemIdsAsync("my-repo", 42, DefaultToken);
        Assert.NotNull(workItemIds);

        var snapshots = await client.GetWorkItemSnapshotsAsync(workItemIds, DefaultToken);
        Assert.Equal(2, snapshots.Count);

        var rendered = ChangelogRenderer.RenderFlatChangelog(snapshots, ChangelogFormat.Markdown, DefaultOptions);

        Assert.Contains("[#53115](https://dev.azure.com/org/Project/_workitems/edit/53115)", rendered);
        Assert.Contains("[Bug] Active - Fix critical bug", rendered);
        Assert.Contains("[#53312](https://dev.azure.com/org/Project/_workitems/edit/53312)", rendered);
        Assert.Contains("[Task] In Progress - Implement feature", rendered);
    }

    [Fact]
    public async Task Changelog_pipeline_markdown_table_format()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupGet(
            "pullRequests/42/workitems",
            HttpStatusCode.OK,
            """
            {"value":[{"id":53115}]}
            """);
        handler.SetupPost(
            "workitemsbatch",
            HttpStatusCode.OK,
            """
            {"value":[
              {"id":53115,"fields":{"System.WorkItemType":"Bug","System.State":"Active","System.Title":"Fix critical bug"},"url":"https://dev.azure.com/org/Project/_apis/wit/workItems/53115"}
            ]}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var workItemIds = await client.TryGetPullRequestWorkItemIdsAsync("my-repo", 42, DefaultToken);
        var snapshots = await client.GetWorkItemSnapshotsAsync(workItemIds!, DefaultToken);
        var rendered = ChangelogRenderer.RenderFlatChangelog(snapshots, ChangelogFormat.Markdown, DefaultOptions, markdownTable: true);

        Assert.Contains("| Work Item | Type | Etat | Titre |", rendered);
        Assert.Contains("| [#53115](https://dev.azure.com/org/Project/_workitems/edit/53115) | Bug | Active | Fix critical bug |", rendered);
    }

    [Fact]
    public async Task Changelog_pipeline_ids_only_from_pr()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupGet(
            "pullRequests/42/workitems",
            HttpStatusCode.OK,
            """
            {"value":[{"id":"53115"},{"id":"53312"}]}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var workItemIds = await client.TryGetPullRequestWorkItemIdsAsync("my-repo", 42, DefaultToken);
        Assert.NotNull(workItemIds);

        var idsOnlyOutput = string.Join(' ', workItemIds!);
        Assert.Equal("53115 53312", idsOnlyOutput);
    }

    [Fact]
    public async Task Changelog_pipeline_group_by_parent()
    {
        var handler = new TestAzureDevOpsHttpHandler();
        handler.SetupGet(
            "pullRequests/42/workitems",
            HttpStatusCode.OK,
            """
            {"value":[{"id":53115},{"id":53312}]}
            """);
        handler.SetupPost(
            "workitemsbatch",
            HttpStatusCode.OK,
            """
            {"value":[
              {"id":53115,"fields":{"System.WorkItemType":"Bug","System.State":"Active","System.Title":"Child bug"},"url":"https://dev.azure.com/org/Project/_apis/wit/workItems/53115"},
              {"id":53312,"fields":{"System.WorkItemType":"Task","System.State":"In Progress","System.Title":"Child task"},"url":"https://dev.azure.com/org/Project/_apis/wit/workItems/53312"}
            ]}
            """);
        handler.SetupGet(
            "$expand=all",
            HttpStatusCode.OK,
            """
            {"id":53115,"fields":{"System.Id":53115,"System.WorkItemType":"Bug","System.State":"Active","System.Title":"Child bug"},"relations":[{"rel":"System.LinkTypes.Hierarchy-Reverse","url":"https://dev.azure.com/org/Project/_apis/wit/workItems/54000"}]}
            """);
        handler.SetupGet(
            "workitems/54000?api-version=",
            HttpStatusCode.OK,
            """
            {"id":54000,"fields":{"System.Id":54000,"System.WorkItemType":"User Story","System.State":"Active","System.Title":"Parent story"},"url":"https://dev.azure.com/org/Project/_apis/wit/workItems/54000"}
            """);

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var workItemIds = await client.TryGetPullRequestWorkItemIdsAsync("my-repo", 42, DefaultToken);
        var snapshots = await client.GetWorkItemSnapshotsAsync(workItemIds!, DefaultToken);
        var groups = AdoCommand.GroupWorkItemsByParent(client, DefaultToken, snapshots);

        Assert.Single(groups);
        Assert.Equal("54000", groups[0].Parent.Id);
        Assert.Equal("Parent story", groups[0].Parent.Title);
        Assert.Equal(2, groups[0].Items.Count);
        Assert.Contains(groups[0].Items, item => item.Id == "53115");
        Assert.Contains(groups[0].Items, item => item.Id == "53312");

        var rendered = ChangelogRenderer.RenderGroupedChangelog(groups, ChangelogFormat.Markdown, DefaultOptions);

        Assert.Contains("## [#54000]", rendered);
        Assert.Contains("- [#53115]", rendered);
        Assert.Contains("- [#53312]", rendered);
    }

    [Fact]
    public async Task Changelog_pipeline_empty_pr_returns_null()
    {
        var handler = new TestAzureDevOpsHttpHandler();

        using var httpClient = new HttpClient(handler);
        var client = new AzureDevOpsClient(httpClient, DefaultOptions);

        var workItemIds = await client.TryGetPullRequestWorkItemIdsAsync("my-repo", 99, DefaultToken);

        Assert.Null(workItemIds);
    }
}
