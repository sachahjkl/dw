namespace Dw.Cli.Workspaces;

internal sealed record WorkspaceTeardownOptions(
    string? Workspace,
    string? Project,
    string? WorkItemId,
    bool Continue,
    bool Execute,
    bool Yes);

internal sealed record WorkspaceTeardownStep(string Repository, string Action, string Target);

internal static class WorkspaceTeardownService
{
    public static int Teardown(CommandContext context, WorkspaceTeardownOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        return Teardown(context, options, root);
    }

    internal static int Teardown(CommandContext context, WorkspaceTeardownOptions options, string root)
    {
        var workspace = WorkspaceOpenService.ResolveWorkspace(
            context,
            root,
            new WorkspaceOpenOptions(options.Workspace, options.Project, options.WorkItemId, options.Continue, ResumeSession: false));
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var config = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(config, manifest.Project);
        var steps = Plan(root, workspace, manifest, projectConfig).ToArray();

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine(options.Execute ? "Teardown execute:" : "Teardown dry-run:");
        foreach (var step in steps)
        {
            context.Out.WriteLine($"- [{step.Repository}] {step.Action}: {step.Target}");
        }

        if (!options.Execute)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Dry-run uniquement. Relancer avec --execute pour supprimer les worktrees et le workspace.");
            return 0;
        }

        if (!options.Yes && !Confirm(context, workspace))
        {
            context.Out.WriteLine("Teardown annule.");
            return 1;
        }

        foreach (var step in steps.Where(step => step.Action == "worktree remove"))
        {
            RunGit(context, step.Repository, "worktree", "remove", "--force", step.Target);
        }

        foreach (var step in steps.Where(step => step.Action == "worktree prune"))
        {
            RunGitDir(context, step.Repository, step.Target, "worktree", "prune");
        }

        if (context.FileSystem.DirectoryExists(workspace))
        {
            context.FileSystem.DeleteDirectory(workspace, recursive: true);
            context.Out.WriteLine($"Workspace supprime: {workspace}");
        }

        return 0;
    }

    private static bool Confirm(CommandContext context, string workspace)
    {
        context.Out.Write($"Confirmer suppression de {workspace} ? [y/N] ");
        var input = Console.ReadLine()?.Trim();
        return string.Equals(input, "y", StringComparison.OrdinalIgnoreCase)
               || string.Equals(input, "yes", StringComparison.OrdinalIgnoreCase)
               || string.Equals(input, "oui", StringComparison.OrdinalIgnoreCase);
    }

    internal static IEnumerable<WorkspaceTeardownStep> Plan(string root, string workspace, WorkspaceManifest manifest, ProjectConfig? projectConfig)
    {
        var projectRoot = Path.Combine(root, "projects", manifest.Project);
        foreach (var repositoryKey in manifest.Repositories)
        {
            var repository = projectConfig?.Repositories.GetValueOrDefault(repositoryKey)
                ?? new RepositoryConfig("", "main", Folder: repositoryKey);
            var folder = string.IsNullOrWhiteSpace(repository.Folder) ? repositoryKey : repository.Folder;
            var worktreePath = Path.Combine(workspace, folder);
            yield return new WorkspaceTeardownStep(repositoryKey, "worktree remove", worktreePath);

            if (!string.IsNullOrWhiteSpace(repository.Url))
            {
                var anchorName = string.IsNullOrWhiteSpace(repository.AnchorName) ? $"{repositoryKey}.git" : repository.AnchorName;
                yield return new WorkspaceTeardownStep(repositoryKey, "worktree prune", Path.Combine(projectRoot, "repositories", anchorName));
            }
        }

        yield return new WorkspaceTeardownStep("workspace", "delete directory", workspace);
    }

    private static void RunGit(CommandContext context, string repository, params string[] args)
    {
        context.Debug($"git {string.Join(' ', args)}");
        var result = context.ProcessRunner.RunAsync("git", args).GetAwaiter().GetResult();
        if (result.ExitCode != 0)
        {
            throw new DwException($"Teardown echoue [{repository}]: {result.StandardError.Trim()}");
        }
    }

    private static void RunGitDir(CommandContext context, string repository, string gitDir, params string[] args)
    {
        if (!context.FileSystem.DirectoryExists(gitDir))
        {
            context.Debug($"git-dir absent, prune ignore [{repository}]: {gitDir}");
            return;
        }

        context.Debug($"git --git-dir {gitDir} {string.Join(' ', args)}");
        var result = context.ProcessRunner.RunAsync("git", ["--git-dir", gitDir, .. args]).GetAwaiter().GetResult();
        if (result.ExitCode != 0)
        {
            throw new DwException($"Teardown prune echoue [{repository}]: {result.StandardError.Trim()}");
        }
    }
}
