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
}
