namespace Dw.Cli.Workspaces;

internal static class TaskWorkItemService
{
    public static int Add(CommandContext context, TaskWorkItemUpdateOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, options.OpenOptions);
        var manifestPath = Path.Combine(workspace, "task.json");
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, manifestPath);
        var selection = WorkItemSet.Parse(options.WorkItemIds);
        var missingIds = selection.Ids.Where(id => !manifest.MatchesWorkItem(id)).ToArray();
        if (missingIds.Length == 0)
        {
            context.Out.WriteLine("Tous les work items demandes sont deja presents dans le workspace.");
            return 0;
        }

        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, manifest.Project);
        var adoContext = TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: true)
            ?? throw new DwException("Contexte Azure DevOps indisponible.");
        var snapshots = missingIds
            .Select(id => adoContext.Client.GetWorkItemSnapshotAsync(id, adoContext.Token).GetAwaiter().GetResult())
            .ToArray();
        var finalItems = snapshots.Where(snapshot => TaskCommand.IsFinalState(snapshot.Type, snapshot.State)).ToArray();
        if (finalItems.Length > 0)
        {
            throw new DwException($"Impossible d'ajouter des work items en etat final: {string.Join(", ", finalItems.Select(item => $"#{item.Id} ({item.State})"))}", 2);
        }

        RejectConflicts(context, root, manifest.Project, workspace, missingIds);
        var updatedWorkItems = manifest.ParentWorkItems
            .Concat(snapshots.Select(snapshot => new WorkspaceWorkItem(snapshot.Id, snapshot.Type, snapshot.Title, snapshot.State)))
            .ToArray();
        ApplyUpdate(context, root, workspace, manifestPath, manifest, updatedWorkItems);
        context.Out.WriteLine($"Work items ajoutes: {string.Join(", ", snapshots.Select(snapshot => WorkspaceManifest.FormatWorkItem(new WorkspaceWorkItem(snapshot.Id, snapshot.Type, snapshot.Title, snapshot.State))))}");
        return 0;
    }

    public static int Remove(CommandContext context, TaskWorkItemUpdateOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspace = WorkspaceOpenService.ResolveWorkspace(context, root, options.OpenOptions);
        var manifestPath = Path.Combine(workspace, "task.json");
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, manifestPath);
        var selection = WorkItemSet.Parse(options.WorkItemIds);
        var remaining = manifest.ParentWorkItems
            .Where(item => !selection.Contains(item.Id))
            .ToArray();
        if (remaining.Length == manifest.ParentWorkItems.Count)
        {
            context.Out.WriteLine("Aucun des work items demandes n'est present dans le workspace.");
            return 0;
        }

        if (remaining.Length == 0)
        {
            throw new DwException("Impossible de retirer tous les work items du workspace. Supprimer le workspace ou laisser au moins un work item principal.", 2);
        }

        ApplyUpdate(context, root, workspace, manifestPath, manifest, remaining);
        context.Out.WriteLine($"Work items retires: {string.Join(", ", manifest.ParentWorkItems.Where(item => selection.Contains(item.Id)).Select(WorkspaceManifest.FormatWorkItem))}");
        return 0;
    }

    private static void ApplyUpdate(
        CommandContext context,
        string root,
        string workspace,
        string manifestPath,
        WorkspaceManifest manifest,
        IReadOnlyList<WorkspaceWorkItem> updatedWorkItems)
    {
        var updated = manifest with
        {
            WorkItemId = updatedWorkItems[0].Id,
            WorkItemType = updatedWorkItems[0].Type,
            WorkItemTitle = updatedWorkItems[0].Title,
            WorkItemState = updatedWorkItems[0].State,
            WorkItems = updatedWorkItems,
            BranchName = GitBranchNames.Build(manifest.Type,
                updatedWorkItems.Select(item => item.Id)
                    .Concat(string.IsNullOrWhiteSpace(manifest.TaskId) ? [] : [manifest.TaskId])
                    .Concat(manifest.NormalizedChildTasks.Select(task => task.Id).Where(id => !string.IsNullOrWhiteSpace(id)))
                    .Distinct(StringComparer.OrdinalIgnoreCase)
                    .ToArray(),
                manifest.Slug)
        };

        context.FileSystem.WriteAllText(manifestPath, WorkspaceManifestWriter.Serialize(updated));
        RewriteAgentConfigs(context.FileSystem, workspace, updated);
        WorkspaceHandoffService.WriteFiles(context.FileSystem, workspace, updated);

        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, updated.Project);
        foreach (var repositoryKey in updated.Repositories)
        {
            var repositoryPath = WorkspaceOpenService.ResolveOpenTarget(workspace, updated, projectConfig, repositoryKey);
            if (context.FileSystem.DirectoryExists(repositoryPath))
            {
                TaskRenameService.RenameLocalBranchIfPresent(context, repositoryPath, manifest.BranchName, updated.BranchName);
            }
        }

        var newWorkspace = Path.Combine(Path.GetDirectoryName(workspace) ?? workspace, GitBranchNames.BuildSubjectName(updated.Type, updated.ParentWorkItems.Select(item => item.Id).ToArray(), updated.Slug));
        if (!string.Equals(workspace, newWorkspace, StringComparison.OrdinalIgnoreCase))
        {
            if (context.FileSystem.DirectoryExists(newWorkspace))
            {
                throw new DwException($"Impossible de renommer le workspace: dossier cible deja existant: {newWorkspace}", 2);
            }

            Directory.Move(workspace, newWorkspace);
            context.Out.WriteLine($"Workspace renomme: {newWorkspace}");
        }
    }

    private static void RewriteAgentConfigs(IFileSystem fileSystem, string workspace, WorkspaceManifest manifest)
    {
        foreach (var file in AgentAdapterRegistry.WorkspaceConfigFiles(new AgentWorkspaceConfigRequest(workspace, manifest.ParentWorkItems, manifest.Project)))
        {
            fileSystem.WriteAllText(Path.Combine(workspace, file.RelativePath), file.Content);
        }
    }

    private static void RejectConflicts(CommandContext context, string root, string project, string currentWorkspace, IReadOnlyList<string> ids)
    {
        var conflicts = WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root)
            .Where(workspace => !string.Equals(workspace.Path, currentWorkspace, StringComparison.OrdinalIgnoreCase))
            .Where(workspace => string.Equals(workspace.Manifest.Project, project, StringComparison.OrdinalIgnoreCase))
            .Where(workspace => ids.Any(workspace.Manifest.MatchesWorkItem))
            .ToArray();

        if (conflicts.Length == 0)
        {
            return;
        }

        throw new DwException($"Un des work items est deja present dans un autre workspace: {string.Join("; ", conflicts.Select(conflict => conflict.Path))}", 2);
    }
}

internal sealed record TaskWorkItemUpdateOptions(string WorkItemIds, WorkspaceOpenOptions OpenOptions);
