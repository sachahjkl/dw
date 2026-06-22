namespace Dw.Cli.Tests;

public sealed class ConfigCommandTests
{
    [Fact]
    public void Doctor_returns_success_when_config_files_are_valid_json()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-config-doctor-test-" + Guid.NewGuid().ToString("N"));
        try
        {
            var fs = new RealFileSystem();
            fs.WriteAllText(Path.Combine(root, "config", "projects.json"), "{\"schema\":1,\"projects\":{}}");
            fs.WriteAllText(Path.Combine(root, "config", "workflow.json"), "{\"schema\":1,\"branchPrefixes\":{},\"azureDevOps\":{},\"auth\":{},\"updates\":{}}");
            fs.WriteAllText(Path.Combine(root, "config", "databases.json"), "{\"schema\":1,\"defaults\":{},\"globals\":{},\"projects\":{}}");
            fs.WriteAllText(Path.Combine(root, "config", "opencode", "opencode.jsonc"), "{\"instructions\":[]}");
            fs.WriteAllText(Path.Combine(root, "schemas", "projects.schema.json"), "{}");
            fs.WriteAllText(Path.Combine(root, "schemas", "workflow.schema.json"), "{}");
            fs.WriteAllText(Path.Combine(root, "schemas", "databases.schema.json"), "{}");
            using var output = new StringWriter();
            using var error = new StringWriter();
            var context = new CommandContext(output, error, new FixedClock(), fs, new NoopProcessRunner());

            var exitCode = ConfigCommand.Doctor(context, root);

            Assert.Equal(0, exitCode);
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
