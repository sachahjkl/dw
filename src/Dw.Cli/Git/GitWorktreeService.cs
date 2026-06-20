namespace Dw.Cli.Git;

internal sealed class GitWorktreeService(IProcessRunner processRunner, IFileSystem fileSystem)
{
    public async Task<GitWorktreeResult> PrepareAsync(
        string projectRoot,
        string repositoryKey,
        RepositoryConfig repository,
        string branchName,
        string worktreePath)
    {
        if (string.IsNullOrWhiteSpace(repository.Url))
        {
            fileSystem.CreateDirectory(worktreePath);
            return GitWorktreeResult.Placeholder(repositoryKey, "URL distante absente dans projects.json.");
        }

        var anchorName = string.IsNullOrWhiteSpace(repository.AnchorName)
            ? $"{repositoryKey}.git"
            : repository.AnchorName;

        var repositoriesRoot = Path.Combine(projectRoot, "repositories");
        var anchorPath = Path.Combine(repositoriesRoot, anchorName);
        fileSystem.CreateDirectory(repositoriesRoot);

        if (!fileSystem.DirectoryExists(anchorPath))
        {
            var clone = await processRunner.RunAsync("git", ["clone", "--bare", repository.Url, anchorPath], projectRoot);
            if (clone.ExitCode != 0)
            {
                return GitWorktreeResult.Failed(repositoryKey, clone.StandardError.Trim());
            }
        }
        else
        {
            var fetch = await processRunner.RunAsync("git", ["--git-dir", anchorPath, "fetch", "--prune", "origin"], projectRoot);
            if (fetch.ExitCode != 0)
            {
                return GitWorktreeResult.Failed(repositoryKey, fetch.StandardError.Trim());
            }
        }

        if (fileSystem.DirectoryExists(worktreePath))
        {
            return GitWorktreeResult.Prepared(repositoryKey, "Worktree deja present.");
        }

        var baseRef = $"origin/{repository.DefaultBranch}";
        var add = await processRunner.RunAsync(
            "git",
            ["--git-dir", anchorPath, "worktree", "add", "-b", branchName, worktreePath, baseRef],
            projectRoot);

        if (add.ExitCode != 0)
        {
            return GitWorktreeResult.Failed(repositoryKey, add.StandardError.Trim());
        }

        return GitWorktreeResult.Prepared(repositoryKey, $"Worktree cree depuis {baseRef}.");
    }
}

internal sealed record GitWorktreeResult(string Repository, GitWorktreeStatus Status, string Message)
{
    public static GitWorktreeResult Prepared(string repository, string message)
        => new(repository, GitWorktreeStatus.Prepared, message);

    public static GitWorktreeResult Placeholder(string repository, string message)
        => new(repository, GitWorktreeStatus.Placeholder, message);

    public static GitWorktreeResult Failed(string repository, string message)
        => new(repository, GitWorktreeStatus.Failed, message);
}

internal enum GitWorktreeStatus
{
    Prepared,
    Placeholder,
    Failed
}
