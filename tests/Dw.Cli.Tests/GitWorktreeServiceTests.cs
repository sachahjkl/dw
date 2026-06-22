namespace Dw.Cli.Tests;

public sealed class GitWorktreeServiceTests
{
    [Fact]
    public async Task PrepareAsync_falls_back_to_local_head_ref_when_origin_ref_is_missing()
    {
        var processRunner = new StubProcessRunner();
        var fileSystem = new StubFileSystem(
            existingDirectories:
            [
                @"S:\root\repositories\front.git"
            ]);
        var service = new GitWorktreeService(processRunner, fileSystem);

        var result = await service.PrepareAsync(
            @"S:\root",
            "front",
            new RepositoryConfig("https://example/repo.git", "develop"),
            "chore/55222-refonte-modale-agences",
            @"S:\root\workspaces\subject\front");

        Assert.Equal(GitWorktreeStatus.Prepared, result.Status);
        Assert.Equal("Worktree cree depuis refs/heads/develop.", result.Message);
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "rev-parse", "--verify", "origin/develop"]));
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "rev-parse", "--verify", "refs/heads/develop"]));
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "worktree", "add", "-b", "chore/55222-refonte-modale-agences", @"S:\root\workspaces\subject\front", "refs/heads/develop"]));
    }

    private sealed class StubProcessRunner : IProcessRunner
    {
        public List<ProcessCall> Calls { get; } = [];

        public Task<ProcessResult> RunAsync(string fileName, string arguments, string? workingDirectory = null)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(ProcessRequest request)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => RunAsync(fileName, arguments, workingDirectory);

        public Task<int> RunInteractiveAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory, IReadOnlyDictionary<string, string>? environment)
            => throw new NotSupportedException();

        public Task<ProcessResult> RunAsync(string fileName, IReadOnlyList<string> arguments, string? workingDirectory = null)
        {
            Calls.Add(new ProcessCall(fileName, arguments.ToArray(), workingDirectory));

            if (arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "fetch", "--prune", "origin"]))
            {
                return Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));
            }

            if (arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "rev-parse", "--verify", "origin/develop"]))
            {
                return Task.FromResult(new ProcessResult(1, string.Empty, "fatal: Needed a single revision"));
            }

            if (arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "rev-parse", "--verify", "refs/heads/develop"]))
            {
                return Task.FromResult(new ProcessResult(0, "abc123", string.Empty));
            }

            if (arguments.SequenceEqual(["--git-dir", @"S:\root\repositories\front.git", "worktree", "add", "-b", "chore/55222-refonte-modale-agences", @"S:\root\workspaces\subject\front", "refs/heads/develop"]))
            {
                return Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));
            }

            throw new InvalidOperationException($"Unexpected command: {fileName} {string.Join(' ', arguments)}");
        }
    }

    private sealed class StubFileSystem(IReadOnlyCollection<string> existingDirectories) : IFileSystem
    {
        public bool DirectoryExists(string path) => existingDirectories.Contains(path, StringComparer.OrdinalIgnoreCase);

        public bool FileExists(string path) => false;

        public void CreateDirectory(string path)
        {
        }

        public string ReadAllText(string path) => throw new NotSupportedException();

        public void WriteAllText(string path, string content) => throw new NotSupportedException();

        public IEnumerable<string> EnumerateFiles(string path, string searchPattern, SearchOption searchOption)
            => throw new NotSupportedException();

        public void DeleteDirectory(string path, bool recursive) => throw new NotSupportedException();
    }

    private sealed record ProcessCall(string FileName, string[] Arguments, string? WorkingDirectory);
}
