namespace Dw.Cli.Tests;

public sealed class UpdateCommandTests
{
    [Fact]
    public void ResolveUpdates_falls_back_to_dw_release_config_when_missing()
    {
        var updates = UpdateCommand.ResolveUpdates(WorkflowConfig.Empty);

        Assert.Equal("sachahjkl", updates.Owner);
        Assert.Equal("dw", updates.Repository);
        Assert.False(updates.IncludePrerelease);
        Assert.Equal("release.json", updates.AssetName);
    }

    [Fact]
    public void ResolveUpdates_prefers_workflow_override_when_present()
    {
        var workflow = WorkflowConfig.Empty with
        {
            Updates = new UpdateOptions("owner", "repo", true, "custom.json")
        };

        var updates = UpdateCommand.ResolveUpdates(workflow);

        Assert.Equal("owner", updates.Owner);
        Assert.Equal("repo", updates.Repository);
        Assert.True(updates.IncludePrerelease);
        Assert.Equal("custom.json", updates.AssetName);
    }
}
