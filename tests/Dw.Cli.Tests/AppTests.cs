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
    public async Task RunAsync_db_query_help_exposes_max_rows_option()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["db", "query", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--max-rows", output);
        Assert.Contains("Nombre maximum de lignes", output);
    }

    [Fact]
    public async Task RunAsync_db_query_rejects_non_positive_max_rows()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["db", "query", "--max-rows", "0", "select", "1"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--max-rows doit etre superieur a 0", error);
    }

    [Fact]
    public async Task RunAsync_task_commit_help_exposes_intermediate_commit_command()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["task", "commit", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("Commit intermediaire", output);
        Assert.Contains("--execute", output);
        Assert.Contains("--continue", output);
    }

    [Fact]
    public async Task RunAsync_task_finish_help_exposes_continue_option()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["task", "finish", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--continue", output);
        Assert.Contains("--create-pr", output);
        Assert.DoesNotContain("--project", output);
        Assert.DoesNotContain("--task", output);
        Assert.DoesNotContain("feat-123-456-demo", output);
    }

    [Fact]
    public async Task RunAsync_ado_changelog_help_exposes_source_modes_and_formats()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["ado", "changelog", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--from-pr", output);
        Assert.Contains("--from-git", output);
        Assert.Contains("--format", output);
        Assert.Contains("--table", output);
        Assert.Contains("--ids-only", output);
        Assert.Contains("--git-to", output);
        Assert.Contains("ids", output);
    }

    [Fact]
    public async Task RunAsync_task_open_help_exposes_positional_work_item_id()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["task", "open", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("work-item-id", output);
        Assert.Contains("--work-item", output);
    }

    [Fact]
    public async Task RunAsync_task_create_child_task_help_exposes_repo_and_title()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["task", "create-child-task", "--help"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--repo", output);
        Assert.Contains("--title", output);
        Assert.Contains("sous-tache", output);
    }

    [Fact]
    public async Task RunAsync_exposes_system_commandline_suggestions()
    {
        using var output = new StringWriter();
        using var error = new StringWriter();
        var context = new CommandContext(output, error, new FixedClock(), new RealFileSystem(), new NoopProcessRunner());

        var completions = SystemCommandLineApp.GetCompletionsForTesting(context, "task finish --");
        var labels = completions.Select(c => c.Label).ToArray();

        Assert.Contains("--workspace", labels);
        Assert.Contains("--create-pr", labels);
    }

    [Fact]
    public async Task RunAsync_completion_show_guides_shell_bridge_installation()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "show"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("dw completion install powershell", output);
        Assert.Contains("dw completion suggest task --", output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_prints_descriptions()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "task", "finish", "--"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--workspace", output);
        Assert.Contains("Chemin explicite du workspace", output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_can_emit_json()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--format", "json", "task", "finish", "--"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("\"label\":\"--workspace\"", output);
        Assert.Contains("\"description\":\"Chemin explicite du workspace.\"", output);
    }

    [Fact]
    public async Task RunAsync_completion_install_powershell_emits_dw_bridge()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "install", "powershell"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("Register-ArgumentCompleter", output);
        Assert.Contains("dw completion suggest --format json", output);
        Assert.Contains("IsNullOrEmpty($wordToComplete)", output);
        Assert.Contains("--empty-token", output);
    }

    [Theory]
    [InlineData("bash", "--empty-token")]
    [InlineData("zsh", "--empty-token")]
    [InlineData("fish", "--empty-token")]
    public async Task RunAsync_completion_install_preserves_empty_token_for_shells(string shell, string expected)
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "install", shell]));

        Assert.Equal(0, exitCode);
        Assert.Contains("dw completion suggest --format json", output);
        Assert.Contains(expected, output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_empty_token_lists_subcommands()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--empty-token", "task"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("start", output);
        Assert.Contains("Cree un workspace et des worktrees", output);
        Assert.True(output.IndexOf("add-repo", StringComparison.Ordinal) < output.IndexOf("--help", StringComparison.Ordinal));
    }

    [Fact]
    public async Task RunAsync_completion_suggest_dash_token_lists_only_options()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "task", "finish", "--"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--create-pr", output);
        Assert.DoesNotContain("add-repo", output);
    }

    [Fact]
    public void Completion_ado_changelog_suggests_local_options()
    {
        using var output = new StringWriter();
        using var error = new StringWriter();
        var context = new CommandContext(output, error, new FixedClock(), new RealFileSystem(), new NoopProcessRunner());

        var completions = SystemCommandLineApp.GetCompletionsForTesting(context, "ado changelog --");

        var labels = completions.Select(c => c.Label).ToArray();
        Assert.Contains("--from-pr", labels);
        Assert.Contains("--from-git", labels);
        Assert.Contains("--repo", labels);
        Assert.Contains("--format", labels);
        Assert.Contains("--table", labels);
        Assert.Contains("--ids-only", labels);
        Assert.Contains("--group-by-parent", labels);
        Assert.Contains("--git-to", labels);
        Assert.DoesNotContain("--comments", labels);
        Assert.DoesNotContain("--summary", labels);
        Assert.DoesNotContain("--top", labels);
        Assert.DoesNotContain("--all", labels);
        Assert.DoesNotContain("--json", labels);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_hides_mutually_exclusive_changelog_switches()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--empty-token", "ado", "changelog", "--from-git"]));

        Assert.Equal(0, exitCode);
        Assert.Contains("--git-to", output);
        Assert.DoesNotContain("--from-pr", output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_hides_finish_flags_blocked_by_selected_options()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--empty-token", "task", "finish", "--skip-ado"]));

        Assert.Equal(0, exitCode);
        Assert.DoesNotContain("--create-pr", output);
        Assert.DoesNotContain("--ready", output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_hides_workspace_when_continue_is_already_selected()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--empty-token", "task", "finish", "--continue"]));

        Assert.Equal(0, exitCode);
        Assert.DoesNotContain("--workspace", output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_hides_table_without_markdown_mode()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--empty-token", "ado", "changelog", "--format", "html"]));

        Assert.Equal(0, exitCode);
        Assert.DoesNotContain("--table", output);
    }

    [Fact]
    public async Task RunAsync_completion_suggest_hides_env_when_database_is_selected()
    {
        var (exitCode, output, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--empty-token", "db", "query", "--database", "dev"]));

        Assert.Equal(0, exitCode);
        Assert.DoesNotContain("--env", output);
    }

    [Fact]
    public async Task RunAsync_ado_changelog_rejects_git_to_without_from_git()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["ado", "changelog", "123", "--git-to", "main"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--git-to requiert --from-git", error);
    }

    [Fact]
    public async Task RunAsync_task_finish_rejects_ready_without_create_pr()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["task", "finish", "--ready"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--ready requiert --create-pr", error);
    }

    [Fact]
    public async Task RunAsync_db_query_rejects_database_and_env_together()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["db", "query", "--database", "dev", "--env", "rec", "select", "1"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--database ne peut pas etre combine avec --env", error);
    }

    [Fact]
    public async Task RunAsync_secret_set_rejects_value_and_from_env_together()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["secret", "set", "demo", "--value", "x", "--from-env", "DW_DEMO"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--value ne peut pas etre combine avec --from-env", error);
    }

    [Fact]
    public async Task RunAsync_task_open_rejects_workspace_with_filters()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["task", "open", "123", "--workspace", "C:\\tmp\\ws", "--project", "ha"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--workspace ne peut pas etre combine avec --project", error);
    }

    [Fact]
    public async Task RunAsync_upgrade_rejects_check_with_rid()
    {
        var (exitCode, _, error) = await CaptureConsole(() => App.RunAsync(["upgrade", "--check", "--rid", "win-x64"]));

        Assert.Equal(2, exitCode);
        Assert.Contains("--check ne peut pas etre combine avec --rid", error);
    }

    [Fact]
    public async Task Completion_sources_use_live_project_and_repository_values()
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
            Directory.CreateDirectory(Path.Combine(root, "projects", "ha", "workspaces", "feat-123-456-demo"));
            File.WriteAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-123-456-demo", "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(
                1,
                "123",
                null,
                "ha",
                "feat",
                "demo",
                "feat/123-456-demo",
                DateTimeOffset.UtcNow,
                ["front"],
                "created",
                WorkItems: [new WorkspaceWorkItem("123"), new WorkspaceWorkItem("456")])));

            var (_, project, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--format", "json", "--empty-token", "task", "start", "123", "--project"]));
            var (_, repo, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--format", "json", "--empty-token", "task", "open", "--repo"]));
            var (_, workItem, _) = await CaptureConsole(() => App.RunAsync(["completion", "suggest", "--format", "json", "task", "start", "123,"]));

            Assert.Contains("\"label\":\"ha\"", project);
            Assert.Contains("\"label\":\"front\"", repo);
            Assert.Contains("\"label\":\"123,456\"", workItem);
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
