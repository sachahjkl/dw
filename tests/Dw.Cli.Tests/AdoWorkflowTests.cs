namespace Dw.Cli.Tests;

public sealed class AdoWorkflowTests
{
    [Fact]
    public void StartState_uses_expected_ogf_defaults()
    {
        Assert.Equal("En réalisation", AdoWorkflowStates.StartState("User Story", null));
        Assert.Equal("En réalisation", AdoWorkflowStates.StartState("Anomalie", null));
        Assert.Equal("En développement", AdoWorkflowStates.StartState("Bug", null));
        Assert.Equal("En développement", AdoWorkflowStates.StartState("Task", null));
    }

    [Fact]
    public void FinishState_never_moves_user_story_to_pr_en_attente()
    {
        Assert.Null(AdoWorkflowStates.FinishState("User Story", null));
        Assert.Null(AdoWorkflowStates.FinishState("Anomalie", null));
        Assert.Equal("PR en attente", AdoWorkflowStates.FinishState("Bug", null));
        Assert.Equal("PR en attente", AdoWorkflowStates.FinishState("Task", null));
    }

    [Fact]
    public void ChildTaskTitle_uses_skill_prefixes()
    {
        Assert.Equal("[FRONT] Ajouter le formulaire", AdoTaskNaming.ChildTaskTitle("front", "Ajouter le formulaire"));
        Assert.Equal("[BACK] Ajouter endpoint", AdoTaskNaming.ChildTaskTitle("back", "Ajouter endpoint"));
        Assert.Equal("[SQL] Ajouter vue", AdoTaskNaming.ChildTaskTitle("sql", "Ajouter vue"));
    }

    [Fact]
    public void CommitMessage_adds_work_item_reference_when_missing()
    {
        var manifest = new WorkspaceManifest(1, "27485", "55201", "ha", "feat", "descriptif", "feat/27485-55201-descriptif", DateTimeOffset.UtcNow, ["front"], "created");

        var message = CommitMessage.EnsureWorkItemReference("feat: descriptif", manifest);

        Assert.Equal("feat: descriptif #55201", message);
    }
}
