namespace Dw.Cli.Tests;

public sealed class GitRepositoryStatusServiceTests
{
    [Fact]
    public async Task GetStatusesAsync_uses_repository_folder_from_project_config()
    {
        var root = Path.Combine(Path.GetTempPath(), "dw-status-test-" + Guid.NewGuid().ToString("N"));
        var workspace = Path.Combine(root, "workspace");
        var physicalBack = Path.Combine(workspace, "OGF.HOMMAGE.EXPLOITATION");
        try
        {
            var fs = new RealFileSystem();
            fs.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(new WorkspaceManifest(
                1,
                "55206",
                null,
                "he",
                "fix",
                "heures-psfs-incoherentes-affichees",
                "fix/55206-heures-psfs-incoherentes-affichees",
                DateTimeOffset.UtcNow,
                ["back"],
                "created")));
            fs.CreateDirectory(physicalBack);
            var projectConfig = new ProjectConfig("HE", new Dictionary<string, RepositoryConfig>(StringComparer.OrdinalIgnoreCase)
            {
                ["back"] = new("https://example/back.git", "master", Folder: "OGF.HOMMAGE.EXPLOITATION")
            });
            var processRunner = new CapturingProcessRunner();
            var service = new GitRepositoryStatusService(processRunner, fs);

            var statuses = await service.GetStatusesAsync(workspace, projectConfig);

            var status = Assert.Single(statuses);
            Assert.Equal("back", status.Repository);
            Assert.Equal(physicalBack, status.Path);
            Assert.True(status.IsGitRepository);
            Assert.True(status.HasChanges);
            Assert.Equal("M file.cs", status.Detail);
            Assert.Equal(physicalBack, processRunner.WorkingDirectory);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    private sealed class CapturingProcessRunner : IProcessRunner
    {
        public string? WorkingDirectory { get; private set; }

        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
        {
            Assert.Equal("git", fileName);
            Assert.Equal(["status", "--short"], arguments);
            WorkingDirectory = workingDirectory;
            return Task.FromResult(new ProcessResult(0, " M file.cs", string.Empty));
        }

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => RunAsync(fileName, arguments, workingDirectory);

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => throw new NotSupportedException();
    }
}
