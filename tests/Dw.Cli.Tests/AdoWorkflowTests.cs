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

    [Fact]
    public void CommitMessage_builds_expected_ogf_message_with_parent_and_task_ids()
    {
        var manifest = new WorkspaceManifest(1, "53115", "53312", "he", "fix", "corriger-le-calcul-des-creneaux", "fix/53115-53312-corriger-le-calcul-des-creneaux", DateTimeOffset.UtcNow, ["back"], "created");

        var message = CommitMessage.Build(manifest);

        Assert.Equal("fix(#53115 #53312): corriger-le-calcul-des-creneaux", message);
    }

    [Fact]
    public void CommitMessage_builds_expected_ogf_message_without_task_id()
    {
        var manifest = new WorkspaceManifest(1, "53020", null, "he", "bug", "corriger-ouverture-dossier", "bug/53020-corriger-ouverture-dossier", DateTimeOffset.UtcNow, ["back"], "created");

        var message = CommitMessage.Build(manifest);

        Assert.Equal("bug(#53020): corriger-ouverture-dossier", message);
    }

    [Theory]
    [InlineData("refactor(#51553 #51786): simplifie l'intégration racine de la palette debug")]
    [InlineData("fix(#51553 #51786): supprime les imports inutilisés signalés par le lint")]
    [InlineData("feat(#53882): ajout des agent skills")]
    [InlineData("chore(#53687): normaliser les fins de ligne Git")]
    [InlineData("bug(#53098): fiabiliser le retour depuis la fiche personne")]
    [InlineData("test(#52162 #53099) : corriger les mocks de route et d'activité")]
    [InlineData("refactor(#52162 #53099) : simplifier le contrat des permissions connectées")]
    public void CommitMessage_accepts_observed_ogf_formats(string message)
    {
        Assert.True(CommitMessage.IsWellFormed(message));
    }

    [Theory]
    [InlineData("refactor: adjust tests (ActivatedRoute)")]
    [InlineData("fix(#53635 #53890) restaurer l url du hub HE")]
    [InlineData("feat #53882: ajout des agent skills")]
    [InlineData("feat(#53882):")]
    public void CommitMessage_rejects_non_ogf_formats(string message)
    {
        Assert.False(CommitMessage.IsWellFormed(message));
    }
}
