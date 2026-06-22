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

    [Fact]
    public async Task RunAsync_completion_suggest_prints_descriptions()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "task", "--"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--task", output);
        Assert.Contains("ID de tache ADO concrete", output);
    }

    [Fact]
    public void Completion_sources_use_live_project_and_repository_values()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-live-completion-test-" + Guid.NewGuid().ToString("N"));
        var previousSettings = File.Exists(AppPaths.UserSettingsPath)
            ? File.ReadAllText(AppPaths.UserSettingsPath)
            : null;
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(AppPaths.UserSettingsPath)!);
            File.WriteAllText(AppPaths.UserSettingsPath, $$"""
{
  "root": "{{root.Replace("\\", "\\\\", StringComparison.Ordinal)}}"
}
""");
            Directory.CreateDirectory(Path.Combine(root, "config"));
            File.WriteAllText(Path.Combine(root, "config", "projects.json"), """
{
  "schema": 1,
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": { "url": "", "defaultBranch": "develop" }
      }
    }
  }
}
""");

            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), new RealFileSystem(), new NoopProcessRunner());
            var project = SystemCommandLineApp.GetCompletionsForTesting(context, "task start 123 --project ");
            var repo = SystemCommandLineApp.GetCompletionsForTesting(context, "task open --repo ");

            Assert.Contains(project, item => item.Label == "ha");
            Assert.Contains(repo, item => item.Label == "front");
        }
        finally
        {
            if (previousSettings is null)
            {
                File.Delete(AppPaths.UserSettingsPath);
            }
            else
            {
                File.WriteAllText(AppPaths.UserSettingsPath, previousSettings);
            }

            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
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

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now => new(2026, 6, 22, 12, 0, 0, TimeSpan.Zero);
    }

    private sealed class NoopProcessRunner : IProcessRunner
    {
        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => Task.FromResult(0);
    }
}
