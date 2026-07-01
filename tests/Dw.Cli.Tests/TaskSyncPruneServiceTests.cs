namespace Dw.Cli.Tests;

public sealed class TaskSyncPruneServiceTests
{
    [Fact]
    public void DisplayWorkItem_includes_id_title_and_state_when_requested()
    {
        var text = TaskSyncPruneService.DisplayWorkItem(new WorkspaceWorkItem("55206", "Bug", "Heures PSFs incoherentes affichees", "Valide"), includeState: true);

        Assert.Equal("#55206 Heures PSFs incoherentes affichees [Valide]", text);
    }

    [Fact]
    public void DisplayWorkItems_joins_multiple_items_with_titles()
    {
        var text = TaskSyncPruneService.DisplayWorkItems(
            [
                new WorkspaceWorkItem("26999", "User Story", "Edition de la demande de transport", "En realisation"),
                new WorkspaceWorkItem("55264", "Task", "Transmission automatique du dossier", "En realisation")
            ],
            includeState: true);

        Assert.Equal("#26999 Edition de la demande de transport [En realisation], #55264 Transmission automatique du dossier [En realisation]", text);
    }
}
