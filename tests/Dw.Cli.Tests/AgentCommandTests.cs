namespace Dw.Cli.Tests;

public sealed class AgentCommandTests
{
    [Fact]
    public void Config_set_default_updates_workflow_agent_default()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-agent-config-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            fs.WriteAllText(Path.Combine(root, "config", "workflow.json"), """
{
  "schema": 1,
  "branchPrefixes": {}
}
""");
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), fs, new NoopProcessRunner());

            var exitCode = AgentCommand.SetDefaultAgent(context, root, "claude");

            Assert.Equal(0, exitCode);
            var workflow = WorkflowConfigStore.Load(fs, root);
            Assert.Equal("claude", workflow.Agent?.Default);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    [Fact]
    public void Config_show_prints_configured_default_agent()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-agent-config-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            fs.WriteAllText(Path.Combine(root, "config", "workflow.json"), """
{
  "schema": 1,
  "branchPrefixes": {},
  "agent": {
    "default": "codex"
  }
}
""");
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), fs, new NoopProcessRunner());

            var exitCode = AgentCommand.ShowConfig(context, root);

            Assert.Equal(0, exitCode);
            Assert.Contains("codex", output.ToString());
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    [Fact]
    public void Context_prints_dw_ado_commands_and_forbids_ado_mcp()
    {
        var context = Templates.AgentContext("S:\\ai-agent-workdir\\dw");

        Assert.Contains("dw ado context", context);
        Assert.Contains("dw task commit", context);
        Assert.Contains("Do not use Azure DevOps MCP tools", context);
        Assert.Contains("do not create them manually", context);
        Assert.Contains("Use `dw` for every ADO, Git naming, PR and worktree operation", context);
        Assert.DoesNotContain("Branches, commits and PR titles must follow the loaded skills", context);
        Assert.DoesNotContain("skills", context, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("Git repositories remain separate", context);
        Assert.DoesNotContain("subject workspace groups", context);
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
