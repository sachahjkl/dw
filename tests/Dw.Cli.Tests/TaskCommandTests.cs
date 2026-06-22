namespace Dw.Cli.Tests;

public sealed class TaskCommandTests
{
    [Fact]
    public void ResolveSlug_normalizes_user_prose()
    {
        var slug = TaskCommand.ResolveSlug("ceci est un Test hehe", "55222", null);

        Assert.Equal("ceci-est-un-test-hehe", slug);
    }

    [Fact]
    public void ResolveSlug_uses_work_item_title_when_slug_is_missing()
    {
        var workItem = new WorkItemSnapshot("55222", "Activité", null, "[TECH] Refaire la modale de changement d'agence", null);

        var slug = TaskCommand.ResolveSlug(null, "55222", workItem);

        Assert.Equal("refaire-la-modale-de-changement-d-agence", slug);
    }

    [Theory]
    [InlineData("User Story", "Validé", true)]
    [InlineData("Anomalie", "Clôturé", true)]
    [InlineData("Bug", "Clôturé", true)]
    [InlineData("Activité", "Abandonné", true)]
    [InlineData("Bug", "En développement", false)]
    public void IsFinalState_detects_terminal_states(string type, string state, bool expected)
    {
        Assert.Equal(expected, TaskCommand.IsFinalState(type, state));
    }
}
