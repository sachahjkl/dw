namespace Dw.Cli.Workspaces;

internal sealed record TaskRepoLatestOptions(string? Workspace, bool Continue, string? Only);

internal static class TaskRepoLatestService
{
    public static int Run(CommandContext context, TaskRepoLatestOptions options)
    {
        var workspace = TaskCommand.ResolveWorkspacePathForWorkspaceCommand(context, options.Workspace, options.Continue);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projectConfig = TaskCommand.ResolveProjectConfig(context, manifest.Project);
        var repositories = ResolveRepositories(manifest, options.Only);

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Branche: {manifest.BranchName}");

        foreach (var repositoryKey in repositories)
        {
            var repositoryConfig = projectConfig?.Repositories.GetValueOrDefault(repositoryKey)
                ?? new RepositoryConfig("", "main", Folder: repositoryKey);
            var folder = string.IsNullOrWhiteSpace(repositoryConfig.Folder) ? repositoryKey : repositoryConfig.Folder;
            var repositoryPath = Path.Combine(workspace, folder);
            context.Out.WriteLine($"Repo {repositoryKey}: sync latest...");
            UpdateRepository(context, repositoryKey, repositoryPath, repositoryConfig.DefaultBranch);
        }

        context.Out.WriteLine("Repos synchronises avec la remote.");
        return 0;
    }

    internal static string ResolveRemoteSourceBranch(string defaultBranch)
        => $"origin/{defaultBranch}";

    private static IReadOnlyList<string> ResolveRepositories(WorkspaceManifest manifest, string? only)
    {
        if (string.IsNullOrWhiteSpace(only))
        {
            return manifest.Repositories;
        }

        var selected = only.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        var unknown = selected.Where(repository => !manifest.Repositories.Contains(repository, StringComparer.OrdinalIgnoreCase)).ToArray();
        if (unknown.Length > 0)
        {
            throw new DwException($"Repo absent du workspace: {string.Join(", ", unknown)}", 2);
        }

        return selected;
    }

    private static void UpdateRepository(CommandContext context, string repositoryKey, string repositoryPath, string defaultBranch)
    {
        var status = context.ProcessRunner.RunAsync("git", ["status", "--short"], repositoryPath).GetAwaiter().GetResult();
        if (status.ExitCode != 0)
        {
            throw new DwException($"Repo latest echoue [{repositoryKey}]: {status.StandardError.Trim()}");
        }

        var hasChanges = !string.IsNullOrWhiteSpace(status.StandardOutput.Trim());
        var stashed = false;
        if (hasChanges)
        {
            TaskCommand.RunGitOrThrow(context, repositoryPath, "stash", "push", "--include-untracked", "-m", "dw task repo-latest autostash");
            stashed = true;
        }

        TaskCommand.RunGitOrThrow(context, repositoryPath, "fetch", "--prune", "origin");
        var sourceBranch = ResolveRemoteSourceBranch(defaultBranch);
        var rebase = context.ProcessRunner.RunAsync("git", ["rebase", sourceBranch], repositoryPath).GetAwaiter().GetResult();
        if (rebase.ExitCode != 0)
        {
            context.ProcessRunner.RunAsync("git", ["rebase", "--abort"], repositoryPath).GetAwaiter().GetResult();
            throw new DwException($"Conflit de rebase sur {repositoryKey}. Relancer manuellement avec: git -C \"{repositoryPath}\" fetch --prune origin puis git -C \"{repositoryPath}\" rebase {sourceBranch}", 2);
        }

        if (stashed)
        {
            var pop = context.ProcessRunner.RunAsync("git", ["stash", "pop"], repositoryPath).GetAwaiter().GetResult();
            if (pop.ExitCode != 0)
            {
                throw new DwException($"Reapplication du stash echouee sur {repositoryKey}: {pop.StandardError.Trim()}");
            }
        }
    }
}
