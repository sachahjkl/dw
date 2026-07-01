namespace Dw.Cli.Workspaces;

internal static class TaskRenameService
{
    public static int Rename(CommandContext context, TaskRenameOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, options.OpenOptions);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, manifest.Project);
        var slug = Slug.FromPhraseOrFallback(options.Slug, manifest.Slug);
        var newBranch = GitBranchNames.Build(manifest.Type, manifest.BranchWorkItemIds, slug);
        var newWorkspace = Path.Combine(Path.GetDirectoryName(workspace) ?? workspace, GitBranchNames.BuildSubjectName(manifest.Type, manifest.ParentWorkItems.Select(item => item.Id).ToArray(), slug));

        context.Out.WriteLine("Rename dry-run:");
        context.Out.WriteLine($"- slug: {manifest.Slug} -> {slug}");
        context.Out.WriteLine($"- branch: {manifest.BranchName} -> {newBranch}");
        context.Out.WriteLine($"- workspace: {workspace} -> {newWorkspace}");
        if (!options.Execute)
        {
            context.Out.WriteLine("Relancer avec --execute pour appliquer.");
            return 0;
        }

        var updated = manifest with { Slug = slug, BranchName = newBranch };
        context.FileSystem.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(updated));
        foreach (var repositoryKey in manifest.Repositories)
        {
            var repository = projectConfig?.Repositories.GetValueOrDefault(repositoryKey) ?? new RepositoryConfig("", "main", Folder: repositoryKey);
            var folder = string.IsNullOrWhiteSpace(repository.Folder) ? repositoryKey : repository.Folder;
            var repositoryPath = Path.Combine(workspace, folder);
            if (context.FileSystem.DirectoryExists(repositoryPath))
            {
                RenameLocalBranchIfPresent(context, repositoryPath, manifest.BranchName, newBranch);
            }
        }

        if (!string.Equals(workspace, newWorkspace, StringComparison.OrdinalIgnoreCase))
        {
            Directory.Move(workspace, newWorkspace);
        }

        context.Out.WriteLine($"Workspace renomme: {newWorkspace}");
        return 0;
    }

    internal static void RenameLocalBranchIfPresent(CommandContext context, string repositoryPath, string oldBranch, string newBranch)
    {
        var current = context.ProcessRunner.RunAsync("git", ["branch", "--show-current"], repositoryPath).GetAwaiter().GetResult();
        if (current.ExitCode != 0)
        {
            context.Debug($"rename branch ignore: {repositoryPath} n'est pas un repo git utilisable");
            return;
        }

        if (!string.Equals(current.StandardOutput.Trim(), oldBranch, StringComparison.OrdinalIgnoreCase))
        {
            context.Debug($"rename branch ignore: branche courante {current.StandardOutput.Trim()} != {oldBranch}");
            return;
        }

        TaskCommand.RunGitOrThrow(context, repositoryPath, "branch", "-m", newBranch);
    }
}

internal sealed record TaskRenameOptions(string Slug, WorkspaceOpenOptions OpenOptions, bool Execute);
