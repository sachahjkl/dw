namespace Dw.Cli.Tests;

public sealed class UpgradeCommandTests
{
    [Fact]
    public void ResolveUpdates_falls_back_to_dw_release_config_when_missing()
    {
        var updates = UpgradeCommand.ResolveUpdates(WorkflowConfig.Empty);

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

        var updates = UpgradeCommand.ResolveUpdates(workflow);

        Assert.Equal("owner", updates.Owner);
        Assert.Equal("repo", updates.Repository);
        Assert.True(updates.IncludePrerelease);
        Assert.Equal("custom.json", updates.AssetName);
    }

    [Fact]
    public void WindowsReplacementScript_waits_for_process_and_restores_backup_on_failure()
    {
        var script = UpgradeCommand.WindowsReplacementScript("new.exe", "dw.exe", "dw.exe.bak", 1234);

        Assert.Contains("tasklist /FI \"PID eq %PID%\"", script);
        Assert.Contains("set \"BACKUP=dw.exe.bak\"", script);
        Assert.Contains("move /Y \"%TARGET%\" \"%BACKUP%\"", script);
        Assert.Contains("copy /Y \"%NEW%\" \"%TARGET%\"", script);
        Assert.Contains("move /Y \"%BACKUP%\" \"%TARGET%\"", script);
        Assert.DoesNotContain("move /Y \"new.exe\" \"dw.exe\"", script);
    }
}
