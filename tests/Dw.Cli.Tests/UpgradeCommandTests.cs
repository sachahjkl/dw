namespace Dw.Cli.Tests;

using System.IO.Compression;

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

    [Fact]
    public void PrepareReplacementExecutable_extracts_dw_exe_from_zip()
    {
        var zipPath = CreateZip(("dw.exe", [0x4D, 0x5A, 0x01, 0x02]));

        var replacement = UpgradeCommand.PrepareReplacementExecutable("dw-win-x64.zip", zipPath, "win-x64");

        try
        {
            Assert.False(File.Exists(zipPath));
            Assert.True(File.Exists(replacement));
            Assert.Equal([0x4D, 0x5A, 0x01, 0x02], File.ReadAllBytes(replacement));
        }
        finally
        {
            if (File.Exists(replacement))
            {
                File.Delete(replacement);
            }
        }
    }

    [Fact]
    public void PrepareReplacementExecutable_rejects_zip_without_dw_exe()
    {
        var zipPath = CreateZip(("readme.txt", [0x41]));

        var ex = Assert.Throws<DwException>(() => UpgradeCommand.PrepareReplacementExecutable("dw-win-x64.zip", zipPath, "win-x64"));

        Assert.Contains("dw.exe introuvable", ex.Message);
        Assert.False(File.Exists(zipPath));
    }

    [Fact]
    public void PrepareReplacementExecutable_rejects_zip_when_dw_exe_is_not_windows_executable()
    {
        var zipPath = CreateZip(("dw.exe", [0x50, 0x4B, 0x03, 0x04]));

        var ex = Assert.Throws<DwException>(() => UpgradeCommand.PrepareReplacementExecutable("dw-win-x64.zip", zipPath, "win-x64"));

        Assert.Contains("n'est pas un executable Windows", ex.Message);
        Assert.False(File.Exists(zipPath));
    }

    [Fact]
    public void PrepareReplacementExecutable_rejects_tar_gz_asset()
    {
        var assetPath = Path.Combine(Path.GetTempPath(), "dw-upgrade-test-" + Guid.NewGuid().ToString("N") + ".tar.gz");
        File.WriteAllBytes(assetPath, [0x1F, 0x8B]);

        var ex = Assert.Throws<DwException>(() => UpgradeCommand.PrepareReplacementExecutable("dw-linux-x64.tar.gz", assetPath, "linux-x64"));

        Assert.Contains("archive non supporte", ex.Message);
        Assert.False(File.Exists(assetPath));
    }

    private static string CreateZip(params (string Name, byte[] Content)[] entries)
    {
        var zipPath = Path.Combine(Path.GetTempPath(), "dw-upgrade-test-" + Guid.NewGuid().ToString("N") + ".zip");
        using (var archive = ZipFile.Open(zipPath, ZipArchiveMode.Create))
        {
            foreach (var entry in entries)
            {
                var zipEntry = archive.CreateEntry(entry.Name);
                using var stream = zipEntry.Open();
                stream.Write(entry.Content);
            }
        }

        return zipPath;
    }
}
