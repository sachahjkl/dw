namespace Dw.Cli.Tests;

public sealed class AdoCommandTests
{
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

        var markdown = AdoCommand.RenderFlatChangelog(items, ChangelogFormat.Markdown, options);

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

        var markdown = AdoCommand.RenderFlatChangelog(items, ChangelogFormat.Markdown, options, markdownTable: true);

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

        var html = AdoCommand.RenderGroupedChangelog(groups, ChangelogFormat.Html, options);

        Assert.Contains("<a href=\"https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53115\">#53115</a>", html);
        Assert.Contains("<a href=\"https://dev.azure.com/digital-factory-ogf/HOMMAGE%20AGENCE/_workitems/edit/53312\">#53312</a>", html);
        Assert.Contains("[Task] En d&#233;veloppement - Enfant", html);
    }
}
