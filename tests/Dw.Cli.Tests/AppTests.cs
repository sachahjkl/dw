namespace Dw.Cli.Tests;

public sealed class AppTests
{
    [Fact]
    public async Task RunAsync_accepts_verbose_flag_before_command()
    {
        var exitCode = await App.RunAsync(["-vvv", "version"]);

        Assert.Equal(0, exitCode);
    }

    [Fact]
    public async Task RunAsync_prints_native_help_for_subcommand()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["task", "start", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("work-item-id", output);
        Assert.Contains("Cree un workspace", output);
    }

    [Fact]
    public async Task RunAsync_exposes_system_commandline_suggestions()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["[suggest]", "task --"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--task", output);
        Assert.Contains("--create-child-tasks", output);
    }

    [Fact]
    public async Task RunAsync_completion_show_guides_dotnet_suggest_installation()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "show"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("dotnet-suggest", output);
        Assert.Contains("[suggest]", output);
    }

    private static async Task<(int ExitCode, string Output, string Error)> CaptureConsole(Func<Task<int>> action)
    {
        var originalOut = Console.Out;
        var originalError = Console.Error;
        using var output = new StringWriter();
        using var error = new StringWriter();
        try
        {
            Console.SetOut(output);
            Console.SetError(error);
            var exitCode = await action();
            return (exitCode, output.ToString(), error.ToString());
        }
        finally
        {
            Console.SetOut(originalOut);
            Console.SetError(originalError);
        }
    }
}
