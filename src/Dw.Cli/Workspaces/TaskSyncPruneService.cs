namespace Dw.Cli.Workspaces;

internal static class TaskSyncPruneService
{
    public static int Sync(CommandContext context, string[] args)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, new WorkspaceOpenOptions(CommandOptions.OptionValue(args, "--workspace"), CommandOptions.OptionValue(args, "--project"), CommandOptions.OptionValue(args, "--work-item"), CommandOptions.HasFlag(args, "--continue")));
        var manifestPath = Path.Combine(workspace, "task.json");
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, manifestPath);
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, manifest.Project);
        var adoContext = TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: true);
        if (adoContext is null)
        {
            throw new DwException("Contexte Azure DevOps indisponible.");
        }

        var workItem = adoContext.Client.GetWorkItemSnapshotAsync(manifest.WorkItemId, adoContext.Token).GetAwaiter().GetResult();
        var updated = manifest with
        {
            WorkItemType = workItem.Type,
            WorkItemTitle = workItem.Title,
            WorkItemState = workItem.State
        };
        context.FileSystem.WriteAllText(manifestPath, WorkspaceManifestWriter.Serialize(updated));
        context.Out.WriteLine($"Workspace synchronise: {workspace}");
        context.Out.WriteLine($"ADO item {workItem.Id}: {workItem.Type} / {workItem.State} / {workItem.Title}");
        return 0;
    }

    public static int Prune(CommandContext context, string[] args)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var execute = CommandOptions.HasFlag(args, "--execute");
        var yes = CommandOptions.HasFlag(args, "--yes");
        var sync = !CommandOptions.HasFlag(args, "--no-sync");
        var project = CommandOptions.OptionValue(args, "--project");
        var workItem = CommandOptions.OptionValue(args, "--work-item");
        var workspaces = WorkspaceDiscoveryService.Filter(WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root), project, workItem);
        if (sync)
        {
            SyncWorkspaces(context, root, workspaces);
            workspaces = WorkspaceDiscoveryService.Filter(WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root), project, workItem);
        }

        var candidates = workspaces
            .Where(workspace => TaskCommand.IsFinalState(workspace.Manifest.WorkItemType, workspace.Manifest.WorkItemState))
            .ToArray();

        if (candidates.Length == 0)
        {
            context.Out.WriteLine("Aucun workspace eligible au prune.");
            return 0;
        }

        foreach (var candidate in candidates)
        {
            context.Out.WriteLine($"{candidate.Manifest.Project} / {candidate.Manifest.WorkItemId} / {candidate.Manifest.WorkItemState}: {candidate.Path}");
            if (execute)
            {
                WorkspaceTeardownService.Teardown(context, new WorkspaceTeardownOptions(candidate.Path, null, null, Continue: false, Execute: true, Yes: yes), root);
            }
        }

        if (!execute)
        {
            context.Out.WriteLine();
            context.Out.WriteLine("Dry-run uniquement. Relancer avec --execute --yes pour supprimer les workspaces eligibles.");
        }

        return 0;
    }

    private static void SyncWorkspaces(CommandContext context, string root, IReadOnlyList<WorkspaceSummary> workspaces)
    {
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        foreach (var workspace in workspaces)
        {
            try
            {
                var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, workspace.Manifest.Project);
                var adoContext = TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: false);
                if (adoContext is null)
                {
                    context.Out.WriteLine($"Sync ignoree (ADO indisponible): {workspace.Path}");
                    continue;
                }

                var workItem = adoContext.Client.GetWorkItemSnapshotAsync(workspace.Manifest.WorkItemId, adoContext.Token).GetAwaiter().GetResult();
                var updated = workspace.Manifest with
                {
                    WorkItemType = workItem.Type,
                    WorkItemTitle = workItem.Title,
                    WorkItemState = workItem.State
                };
                context.FileSystem.WriteAllText(Path.Combine(workspace.Path, "task.json"), WorkspaceManifestWriter.Serialize(updated));
                context.Out.WriteLine($"Sync: {workspace.Manifest.WorkItemId} -> {workItem.State}");
            }
            catch (DwException ex)
            {
                context.Out.WriteLine($"Sync ignoree [{workspace.Manifest.WorkItemId}]: {ex.Message}");
            }
        }
    }
}
