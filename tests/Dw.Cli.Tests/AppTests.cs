namespace Dw.Cli.Tests;

public sealed class AppTests
{
    [Fact]
    public async Task RunAsync_accepts_verbose_flag_before_command()
    {
        var exitCode = await App.RunAsync(["-vvv", "version"]);

        Assert.Equal(0, exitCode);
    }
}
