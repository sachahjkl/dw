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

            var exitCode = InitCommand.Run(context, ["--root", root, "--profile", "ogf", "--no-save"]);

            Assert.Equal(0, exitCode);
            Assert.True(File.Exists(Path.Combine(root, "config", "projects.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "projects.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "workflow.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "databases.schema.json")));
            Assert.True(File.Exists(Path.Combine(root, "schemas", "release.schema.json")));
            Assert.Contains("\"$schema\": \"../schemas/projects.schema.json\"", File.ReadAllText(Path.Combine(root, "config", "projects.json")));
            Assert.Contains("\"$schema\": \"../schemas/workflow.schema.json\"", File.ReadAllText(Path.Combine(root, "config", "workflow.json")));
            Assert.Contains("\"$schema\": \"../schemas/databases.schema.json\"", File.ReadAllText(Path.Combine(root, "config", "databases.json")));
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
