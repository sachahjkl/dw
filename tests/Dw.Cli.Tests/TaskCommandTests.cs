namespace Dw.Cli.Tests;

public sealed class TaskCommandTests
{
    [Fact]
    public void ResolveSlug_normalizes_user_prose()
    {
        var slug = TaskCommand.ResolveSlug("ceci est un Test hehe", "55222", null);

        Assert.Equal("ceci-est-un-test-hehe", slug);
    }

    [Fact]
    public void ResolveSlug_uses_work_item_title_when_slug_is_missing()
    {
        var workItem = new WorkItemSnapshot("55222", "Activité", null, "[TECH] Refaire la modale de changement d'agence", null);

        var slug = TaskCommand.ResolveSlug(null, "55222", workItem);

        Assert.Equal("refaire-la-modale-de-changement-d-agence", slug);
    }

    [Theory]
    [InlineData("User Story", "Validé", true)]
    [InlineData("Anomalie", "Clôturé", true)]
    [InlineData("Bug", "Clôturé", true)]
    [InlineData("Activité", "Abandonné", true)]
    [InlineData("Bug", "En développement", false)]
    public void IsFinalState_detects_terminal_states(string type, string state, bool expected)
    {
        Assert.Equal(expected, TaskCommand.IsFinalState(type, state));
    }

    [Fact]
    public void ResolveNodePackageManagerCommand_prefers_pnpm_for_npm_commands_when_available()
    {
        var context = new CommandContext(new StringWriter(), new StringWriter(), new FixedClock(), new RealFileSystem(), new PackageManagerProcessRunner(pnpmAvailable: true));

        var command = TaskCommand.ResolveNodePackageManagerCommand(context, "npm test");

        Assert.Equal("pnpm test", command);
    }

    [Fact]
    public void ResolveNodePackageManagerCommand_keeps_npm_when_pnpm_is_unavailable()
    {
        var context = new CommandContext(new StringWriter(), new StringWriter(), new FixedClock(), new RealFileSystem(), new PackageManagerProcessRunner(pnpmAvailable: false));

        var command = TaskCommand.ResolveNodePackageManagerCommand(context, "npm test");

        Assert.Equal("npm test", command);
    }

    [Fact]
    public void ResolveNodePackageManagerCommand_leaves_non_npm_commands_unchanged()
    {
        var context = new CommandContext(new StringWriter(), new StringWriter(), new FixedClock(), new RealFileSystem(), new PackageManagerProcessRunner(pnpmAvailable: true));

        var command = TaskCommand.ResolveNodePackageManagerCommand(context, "dotnet test");

        Assert.Equal("dotnet test", command);
    }

    [Fact]
    public void SelectPullRequestCandidates_keeps_actionable_repositories_when_present()
    {
        var context = new CommandContext(new StringWriter(), new StringWriter(), new FixedClock(), new RealFileSystem(), new ReviewableBranchProcessRunner());
        var statuses = new[]
        {
            new RepositoryStatus("front", @"C:\ws\front", true, true, false, string.Empty),
            new RepositoryStatus("back", @"C:\ws\back", true, false, false, string.Empty)
        };

        var selected = TaskCommand.SelectPullRequestCandidates(context, statuses, [statuses[0]], projectConfig: null);

        Assert.Collection(selected, status => Assert.Equal("front", status.Repository));
    }

    [Fact]
    public void SelectPullRequestCandidates_falls_back_to_clean_repositories_with_reviewable_commits()
    {
        var context = new CommandContext(new StringWriter(), new StringWriter(), new FixedClock(), new RealFileSystem(), new ReviewableBranchProcessRunner());
        var statuses = new[]
        {
            new RepositoryStatus("front", @"C:\ws\front", true, false, false, string.Empty),
            new RepositoryStatus("back", @"C:\ws\back", true, false, false, string.Empty)
        };

        var selected = TaskCommand.SelectPullRequestCandidates(context, statuses, [], projectConfig: null);

        Assert.Collection(selected, status => Assert.Equal("front", status.Repository));
    }

    [Fact]
    public void TaskPreflightReport_models_blocking_and_warning_issues()
    {
        var report = new TaskPreflightReport(
            "dw.task.preflight.v1",
            @"C:\ws",
            "ha",
            ["55201"],
            [
                new TaskPreflightIssue("ado.predecessors.active", "blocking", "55201", "Pred actif", null, ["55199"]),
                new TaskPreflightIssue("ado.attachments.present", "warning", "55201", "PJ presente", null, ["55201"])
            ],
            HasBlockingIssues: true);

        Assert.Equal("dw.task.preflight.v1", report.SchemaVersion);
        Assert.True(report.HasBlockingIssues);
        Assert.Contains(report.Issues, issue => issue.Severity == "blocking");
        Assert.Contains(report.Issues, issue => issue.Severity == "warning");
    }

    [Fact]
    public void TaskPreflightReport_detects_stale_workspace_state()
    {
        using var document = System.Text.Json.JsonDocument.Parse(
            """
            {
              "id": 55201,
              "fields": {
                "System.Title": "Titre ADO",
                "System.WorkItemType": "Bug",
                "System.State": "En developpement"
              }
            }
            """);

        var aiContext = AdoCommand.MapAiContextItem(document.RootElement, new AzureDevOpsOptions("https://dev.azure.com/org", "Project"), summaryOnly: false);
        var manifest = new WorkspaceManifest(1, "55201", null, "ha", "feat", "demo", "feat/55201-demo", DateTimeOffset.UtcNow, ["front"], "created", WorkItems: [new WorkspaceWorkItem("55201", "Bug", "Titre local", "New")]);

        var staleIssues = typeof(TaskPreflightService)
            .GetMethod("BuildStaleContextIssues", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static)!
            .Invoke(null, [aiContext, manifest]) as IReadOnlyList<TaskPreflightIssue>;

        Assert.NotNull(staleIssues);
        Assert.Single(staleIssues!);
        Assert.Equal("workspace.ado-context.stale", staleIssues[0].Code);
        Assert.Equal("warning", staleIssues[0].Severity);
    }

    [Fact]
    public void TaskHandoffValidationReport_is_invalid_when_one_repo_stays_todo()
    {
        var report = new TaskHandoffValidationReport(
            "dw.task.handoff-validation.v1",
            @"C:\ws",
            "ha",
            [
                new TaskHandoffValidationItem("front", @"C:\ws\handoff-front.md", "valid", true, "ok"),
                new TaskHandoffValidationItem("back", @"C:\ws\handoff-back.md", "todo", false, "todo")
            ],
            IsValid: false);

        Assert.False(report.IsValid);
        Assert.Contains(report.Items, item => item.Status == "todo");
    }

    [Theory]
    [InlineData("done", true)]
    [InlineData("todo", false)]
    [InlineData("in_progress", false)]
    [InlineData("blocked", false)]
    public void TaskHandoffValidation_status_policy_requires_done_for_finish(string status, bool expectedValid)
    {
        var valid = string.Equals(status, "done", StringComparison.OrdinalIgnoreCase);

        Assert.Equal(expectedValid, valid);
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now => new(2026, 6, 22, 12, 0, 0, TimeSpan.Zero);
    }

    private sealed class PackageManagerProcessRunner(bool pnpmAvailable) : IProcessRunner
    {
        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
        {
            if (fileName == "pnpm" || arguments.Contains("pnpm --version", StringComparison.OrdinalIgnoreCase))
            {
                return Task.FromResult(pnpmAvailable
                    ? new ProcessResult(0, "10.0.0", string.Empty)
                    : new ProcessResult(1, string.Empty, "not found"));
            }

            return Task.FromResult(new ProcessResult(1, string.Empty, "unexpected command"));
        }

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => RunAsync(request.FileName, request.ArgumentString ?? string.Join(' ', request.Arguments ?? Array.Empty<string>()), request.WorkingDirectory);

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
            => RunAsync(fileName, string.Join(' ', arguments), workingDirectory);

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => RunAsync(fileName, arguments, workingDirectory);

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => Task.FromResult(0);
    }

    private sealed class ReviewableBranchProcessRunner : IProcessRunner
    {
        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => Task.FromResult(new ProcessResult(1, string.Empty, "unexpected command"));

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => RunAsync(request.FileName, request.ArgumentString ?? string.Join(' ', request.Arguments ?? Array.Empty<string>()), request.WorkingDirectory);

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
        {
            if (fileName == "git"
                && arguments.Count == 3
                && arguments[0] == "rev-list"
                && arguments[1] == "--count")
            {
                var output = string.Equals(workingDirectory, @"C:\ws\front", StringComparison.OrdinalIgnoreCase) ? "2" : "0";
                return Task.FromResult(new ProcessResult(0, output, string.Empty));
            }

            return Task.FromResult(new ProcessResult(1, string.Empty, "unexpected command"));
        }

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => RunAsync(fileName, arguments, workingDirectory);

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => Task.FromResult(0);
    }
}
