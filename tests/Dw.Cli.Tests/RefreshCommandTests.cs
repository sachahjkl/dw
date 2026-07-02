namespace Dw.Cli.Tests;

public sealed class RefreshCommandTests
{
    [Fact]
    public void Refresh_regenerates_generated_files_and_preserves_user_files()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-refresh-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            fs.CreateDirectory(Path.Combine(root, "config", "opencode"));
            fs.CreateDirectory(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo"));
            fs.WriteAllText(Path.Combine(root, "config", "projects.json"), Templates.BusinessProjectsJson);
            fs.WriteAllText(Path.Combine(root, "config", "workflow.json"), "custom workflow");
            fs.WriteAllText(Path.Combine(root, "config", "databases.json"), "custom databases");
            fs.WriteAllText(Path.Combine(root, "config", "opencode", "AGENTS.md"), "stale agents");
            fs.WriteAllText(Path.Combine(root, "schemas", "projects.schema.json"), "stale schema");
            fs.WriteAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(
                1,
                "11010",
                null,
                "ha",
                "feat",
                "demo",
                "feat/11010-demo",
                DateTimeOffset.UtcNow,
                ["front"],
                "created",
                WorkItems: [new WorkspaceWorkItem("11010"), new WorkspaceWorkItem("55206")])));
            fs.WriteAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "AGENTS.md"), "stale workspace agents");
            fs.WriteAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "plan.md"), "my plan");

            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), fs, new NoopProcessRunner());

            var exitCode = RefreshCommand.Run(context, root, null);

            Assert.Equal(0, exitCode);
            Assert.Contains("dw ado", fs.ReadAllText(Path.Combine(root, "config", "opencode", "AGENTS.md")));
            Assert.NotEqual("stale schema", fs.ReadAllText(Path.Combine(root, "schemas", "projects.schema.json")));
            Assert.Contains("#11010", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "AGENTS.md")));
            Assert.Contains("dw task create-child-task", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "AGENTS.md")));
            Assert.Contains("dw task preflight --continue", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "AGENTS.md")));
            Assert.Contains("dw task handoff-validate --continue", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "AGENTS.md")));
            Assert.Contains("Use sub-agents for independent tracks whenever possible", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "AGENTS.md")));
            Assert.True(fs.FileExists(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "handoff-front.md")));
            Assert.Contains("Synthèse structurée attendue", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "handoff-front.md")));
            Assert.Equal("my plan", fs.ReadAllText(Path.Combine(root, "projects", "ha", "workspaces", "feat-11010-demo", "plan.md")));
            Assert.Equal("custom workflow", fs.ReadAllText(Path.Combine(root, "config", "workflow.json")));
            Assert.Equal("custom databases", fs.ReadAllText(Path.Combine(root, "config", "databases.json")));
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
        public DateTimeOffset Now => new(2026, 6, 26, 12, 0, 0, TimeSpan.Zero);
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
