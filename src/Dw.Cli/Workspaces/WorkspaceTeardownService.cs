namespace Dw.Cli.Workspaces;

internal sealed record WorkspaceTeardownOptions(
    string? Workspace,
    string? Project,
    string? WorkItemId,
    bool Continue,
    bool Execute,
    bool Yes);

internal sealed record WorkspaceTeardownStep(string Repository, string Action, string Target, string? GitDir = null);

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
            new WorkspaceOpenOptions(options.Workspace, options.Project, options.WorkItemId, options.Continue));
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
            RunGitDir(context, step.Repository, step.GitDir, "worktree", "remove", "--force", step.Target);
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
        context.Out.Flush();
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
            var anchorName = string.IsNullOrWhiteSpace(repository.AnchorName) ? $"{repositoryKey}.git" : repository.AnchorName;
            var gitDir = Path.Combine(projectRoot, "repositories", anchorName);
            yield return new WorkspaceTeardownStep(repositoryKey, "worktree remove", worktreePath, gitDir);

            if (!string.IsNullOrWhiteSpace(repository.Url))
            {
                yield return new WorkspaceTeardownStep(repositoryKey, "worktree prune", gitDir, gitDir);
            }
        }

        yield return new WorkspaceTeardownStep("workspace", "delete directory", workspace);
    }

    private static void RunGitDir(CommandContext context, string repository, string? gitDir, params string[] args)
    {
        if (string.IsNullOrWhiteSpace(gitDir))
        {
            throw new DwException($"Teardown echoue [{repository}]: gitDir manquant.");
        }

        if (!context.FileSystem.DirectoryExists(gitDir))
        {
            var isPrune = args.Length >= 2 && args[0] == "worktree" && args[1] == "prune";
            if (isPrune)
            {
                context.Debug($"git-dir absent, prune ignore [{repository}]: {gitDir}");
                return;
            }

            throw new DwException($"Teardown echoue [{repository}]: gitDir introuvable {gitDir}");
        }

        var gitArgs = new[] { "--git-dir", gitDir }.Concat(args).ToArray();
        context.Debug($"git {string.Join(' ', gitArgs)}");
        var result = context.ProcessRunner.RunAsync("git", gitArgs).GetAwaiter().GetResult();
        if (result.ExitCode != 0)
        {
            throw new DwException($"Teardown prune echoue [{repository}]: {result.StandardError.Trim()}");
        }
    }

}
