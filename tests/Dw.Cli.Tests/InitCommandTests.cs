namespace Dw.Cli.Tests;

public sealed class InitCommandTests
{
    [Fact]
    public void Init_writes_config_and_embedded_schemas_with_relative_schema_links()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-init-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), new RealFileSystem(), new NoopProcessRunner());

            var exitCode = InitCommand.Run(context, new InitRequest(root, "ogf", NoSave: true, DryRun: false));

            Assert.Equal(0, exitCode);
            Assert.True(File.Exists(Path.Combine(root, "config", "projects.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "projects.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "workflow.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "databases.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "release.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "config", "opencode", "AGENTS.md")));
            Assert.True(File.Exists(Path.Combine(root, "config", "opencode", "opencode.jsonc")));
            Assert.True(File.Exists(Path.Combine(root, "config", "claude", "CLAUDE.md")));
            Assert.True(File.Exists(Path.Combine(root, "config", "cursor", "devworkflow.mdc")));
            Assert.True(File.Exists(Path.Combine(root, "config", "codex", "AGENTS.md")));
            Assert.True(File.Exists(Path.Combine(root, "config", "codex", "config.toml")));
            Assert.True(File.Exists(Path.Combine(root, "config", "copilot", "copilot-instructions.md")));
            Assert.Contains("\"$schema\": \"../schemas/projects.schema.json\"", File.ReadAllText(Path.Combine(root, "config", "projects.json")));
            Assert.Contains("\"$schema\": \"../schemas/workflow.schema.json\"", File.ReadAllText(Path.Combine(root, "config", "workflow.json")));
            Assert.Contains("\"$schema\": \"../schemas/databases.schema.json\"", File.ReadAllText(Path.Combine(root, "config", "databases.json")));

            var opencodeConfig = File.ReadAllText(Path.Combine(root, "config", "opencode", "opencode.jsonc"));
            var opencodeInstructions = File.ReadAllText(Path.Combine(root, "config", "opencode", "AGENTS.md"));
            Assert.DoesNotContain("@azure-devops/mcp", opencodeConfig);
            Assert.DoesNotContain("\"mcp\"", opencodeConfig);
            Assert.Contains("\"lsp\": true", opencodeConfig);
            Assert.Contains("\"bash\": \"allow\"", opencodeConfig);
            Assert.Contains("\"edit\": \"allow\"", opencodeConfig);
            Assert.Contains("dw ado", opencodeInstructions);
            Assert.Contains("dw ado work-item", opencodeInstructions);
            Assert.Contains("dw db schema", opencodeInstructions);
            Assert.Contains("dw task current", opencodeInstructions);
            Assert.Contains("dw task sync --continue", opencodeInstructions);
            Assert.Contains("plan.md", opencodeInstructions);
            Assert.Contains("dw task commit", opencodeInstructions);
            Assert.Contains("do not use Azure DevOps MCP tools", opencodeInstructions);
            Assert.Contains("Use `dw` commands for ADO lifecycle", opencodeInstructions);
            Assert.DoesNotContain("skills", opencodeInstructions, StringComparison.OrdinalIgnoreCase);
            Assert.DoesNotContain("Keep front and back as separate Git repositories", opencodeInstructions);
            Assert.DoesNotContain("Group worktrees for the same subject", opencodeInstructions);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    private sealed class FixedClock : IClock
    {
        public DateTimeOffset Now => new(2026, 6, 20, 12, 0, 0, TimeSpan.Zero);
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
