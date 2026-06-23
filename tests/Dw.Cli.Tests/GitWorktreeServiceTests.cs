namespace Dw.Cli.Tests;

public sealed class GitWorktreeServiceTests
{
    [Fact]
    public async Task PrepareAsync_falls_back_to_local_head_ref_when_origin_ref_is_missing()
    {
        var processRunner = new StubProcessRunner();
        var root = Path.Combine(Path.GetTempPath(), "dw-root");
        var anchor = Path.Combine(root, "repositories", "front.git");
        var worktree = Path.Combine(root, "workspaces", "subject", "front");
        var fileSystem = new StubFileSystem(
            existingDirectories:
            [
                anchor
            ]);
        var service = new GitWorktreeService(processRunner, fileSystem);

        var result = await service.PrepareAsync(
            root,
            "front",
            new RepositoryConfig("https://example/repo.git", "develop"),
            "chore/55222-refonte-modale-agences",
            worktree);

        Assert.Equal(GitWorktreeStatus.Prepared, result.Status);
        Assert.Equal("Worktree cree depuis refs/heads/develop.", result.Message);
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", anchor, "rev-parse", "--verify", "origin/develop"]));
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", anchor, "rev-parse", "--verify", "refs/heads/develop"]));
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", anchor, "worktree", "add", "-b", "chore/55222-refonte-modale-agences", worktree, "refs/heads/develop"]));
    }

    [Fact]
    public async Task PrepareAsync_reuses_existing_branch_when_previous_worktree_creation_failed()
    {
        var processRunner = new StubProcessRunner(BranchExists: true);
        var root = Path.Combine(Path.GetTempPath(), "dw-root");
        var anchor = Path.Combine(root, "repositories", "front.git");
        var worktree = Path.Combine(root, "workspaces", "subject", "front");
        var fileSystem = new StubFileSystem(existingDirectories: [anchor]);
        var service = new GitWorktreeService(processRunner, fileSystem);

        var result = await service.PrepareAsync(
            root,
            "front",
            new RepositoryConfig("https://example/repo.git", "develop"),
            "chore/55222-refonte-modale-agences",
            worktree);

        Assert.Equal(GitWorktreeStatus.Prepared, result.Status);
        Assert.Equal("Worktree cree depuis la branche existante chore/55222-refonte-modale-agences.", result.Message);
        Assert.Contains(processRunner.Calls, call =>
            call.FileName == "git" &&
            call.Arguments.SequenceEqual(["--git-dir", anchor, "worktree", "add", worktree, "chore/55222-refonte-modale-agences"]));
        Assert.DoesNotContain(processRunner.Calls, call => call.Arguments.Contains("-b"));
    }

    private sealed class StubProcessRunner(bool BranchExists = false) : IProcessRunner
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

            if (arguments is ["--git-dir", _, "fetch", "--prune", "origin"])
            {
                return Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));
            }

            if (arguments is ["--git-dir", _, "rev-parse", "--verify", "origin/develop"])
            {
                return Task.FromResult(new ProcessResult(1, string.Empty, "fatal: Needed a single revision"));
            }

            if (arguments is ["--git-dir", _, "rev-parse", "--verify", "refs/heads/develop"])
            {
                return Task.FromResult(new ProcessResult(0, "abc123", string.Empty));
            }

            if (arguments is ["--git-dir", _, "rev-parse", "--verify", "refs/heads/chore/55222-refonte-modale-agences"])
            {
                return Task.FromResult(BranchExists
                    ? new ProcessResult(0, "abc123", string.Empty)
                    : new ProcessResult(1, string.Empty, "fatal: Needed a single revision"));
            }

            if (arguments is ["--git-dir", _, "worktree", "add", "-b", "chore/55222-refonte-modale-agences", _, "refs/heads/develop"])
            {
                return Task.FromResult(new ProcessResult(0, string.Empty, string.Empty));
            }

            if (arguments is ["--git-dir", _, "worktree", "add", _, "chore/55222-refonte-modale-agences"])
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
