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
}
