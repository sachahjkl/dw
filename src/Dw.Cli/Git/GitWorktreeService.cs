using System.Diagnostics;

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
        Debug.Assert(!string.IsNullOrWhiteSpace(anchorName), "Anchor name should always be resolved.");

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

            var fixFetch = await processRunner.RunAsync("git", ["--git-dir", anchorPath, "config", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"], projectRoot);
            if (fixFetch.ExitCode != 0)
            {
                return GitWorktreeResult.Failed(repositoryKey, $"Config fetch echouee: {fixFetch.StandardError.Trim()}");
            }
        }
        else
        {
            var ensureFetch = await processRunner.RunAsync("git", ["--git-dir", anchorPath, "config", "remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"], projectRoot);
            if (ensureFetch.ExitCode != 0)
            {
                return GitWorktreeResult.Failed(repositoryKey, $"Config fetch echouee: {ensureFetch.StandardError.Trim()}");
            }

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

        var baseRef = await ResolveBaseRefAsync(projectRoot, anchorPath, repository.DefaultBranch);
        if (baseRef is null)
        {
            return GitWorktreeResult.Failed(
                repositoryKey,
                $"Branche de base introuvable: {repository.DefaultBranch}. References testees: origin/{repository.DefaultBranch}, refs/heads/{repository.DefaultBranch}.");
        }

        var branchExists = await BranchExistsAsync(projectRoot, anchorPath, branchName);
        var addArguments = branchExists
            ? new[] { "--git-dir", anchorPath, "worktree", "add", worktreePath, branchName }
            : ["--git-dir", anchorPath, "worktree", "add", "-b", branchName, worktreePath, baseRef];

        var add = await processRunner.RunAsync(
            "git",
            addArguments,
            projectRoot);

        if (add.ExitCode != 0)
        {
            return GitWorktreeResult.Failed(repositoryKey, add.StandardError.Trim());
        }

        return GitWorktreeResult.Prepared(repositoryKey, branchExists ? $"Worktree cree depuis la branche existante {branchName}." : $"Worktree cree depuis {baseRef}.");
    }

    private async Task<bool> BranchExistsAsync(string projectRoot, string anchorPath, string branchName)
    {
        var result = await processRunner.RunAsync(
            "git",
            ["--git-dir", anchorPath, "rev-parse", "--verify", $"refs/heads/{branchName}"],
            projectRoot);

        return result.ExitCode == 0;
    }

    private async Task<string?> ResolveBaseRefAsync(string projectRoot, string anchorPath, string defaultBranch)
    {
        foreach (var candidate in new[] { $"origin/{defaultBranch}", $"refs/heads/{defaultBranch}" })
        {
            var result = await processRunner.RunAsync(
                "git",
                ["--git-dir", anchorPath, "rev-parse", "--verify", candidate],
                projectRoot);

            if (result.ExitCode == 0)
            {
                return candidate;
            }
        }

        return null;
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
