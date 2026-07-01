namespace Dw.Cli.Commands;

internal static partial class TaskCommand
{
    internal static int Commit(CommandContext context, TaskCommitRequest request)
    {
        var workspace = ResolveWorkspacePath(context, request.Workspace, request.Continue);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projectConfig = ResolveProjectConfig(context, manifest.Project);
        var statuses = new GitRepositoryStatusService(context.ProcessRunner, context.FileSystem)
            .GetStatusesAsync(workspace, projectConfig)
            .GetAwaiter()
            .GetResult();

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Branche: {manifest.BranchName}");
        PrintStatuses(context, statuses);

        var changed = statuses.Where(status => status.IsGitRepository && status.HasChanges).ToList();
        if (changed.Count == 0)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Rien a committer.");
            return 0;
        }

        var commitMessage = CommitMessage.Build(manifest, request.Message);
        context.Out.WriteLine();
        context.Out.WriteLine($"Message: {commitMessage}");

        if (!request.Execute)
        {
            context.Out.WriteLine("Dry-run uniquement. Relancer avec --execute pour committer.");
            return 0;
        }

        foreach (var status in changed)
        {
            RunGitOrThrow(context, status.Path, "add", ".");
            RunGitOrThrow(context, status.Path, "commit", "-m", commitMessage);
        }

        context.Out.WriteLine("Commits termines. Aucun push ni PR creee.");
        return 0;
    }
}
