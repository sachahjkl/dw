namespace Dw.Cli.Tests;

public sealed class ProcessRunnerTests
{
    [Fact]
    public void ResolveRequestsForTesting_on_windows_adds_cmd_and_powershell_ps1_fallbacks_for_bare_command()
    {
        if (!OperatingSystem.IsWindows())
        {
            return;
        }

        var requests = ProcessRunner.ResolveRequestsForTesting(new ProcessRequest("opencode", Arguments: ["--version"], Interactive: true));

        Assert.Equal(3, requests.Count);
        Assert.Equal("opencode", requests[0].FileName);
        Assert.Equal("opencode.cmd", requests[1].FileName);
        Assert.Equal("powershell", requests[2].FileName);
        Assert.Equal(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "opencode.ps1", "--version"], requests[2].Arguments);
    }
}
