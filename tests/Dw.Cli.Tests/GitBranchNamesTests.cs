namespace Dw.Cli.Tests;

public sealed class GitBranchNamesTests
{
    [Fact]
    public void Build_uses_work_item_and_task_when_task_exists()
    {
        var branch = GitBranchNames.Build("feat", "27485", "55201", "descriptif cours");

        Assert.Equal("feat/27485-55201-descriptif-cours", branch);
    }

    [Fact]
    public void Build_omits_task_when_absent()
    {
        var branch = GitBranchNames.Build("bug", "53020", null, "ouverture dossier recherche");

        Assert.Equal("bug/53020-ouverture-dossier-recherche", branch);
    }

    [Fact]
    public void BuildSubjectName_uses_folder_format()
    {
        var subject = GitBranchNames.BuildSubjectName("fix", "53635", "reprendre numéro HE");

        Assert.Equal("fix-53635-reprendre-numero-he", subject);
    }

    [Fact]
    public void Build_uses_all_work_item_ids()
    {
        var branch = GitBranchNames.Build("feat", ["11010", "55206", "55207"], "descriptif cours");

        Assert.Equal("feat/11010-55206-55207-descriptif-cours", branch);
    }
}
