using Dw.Cli.Agents;

namespace Dw.Cli.Workspaces;

internal static class TaskStartService
{
    public static int Start(CommandContext context, TaskStartRequest request)
    {
        var workItemsSelection = WorkItemSet.Parse(request.WorkItemId);
        var workItemId = workItemsSelection.PrimaryId;
        var project = request.Project ?? "default";
        var taskId = request.TaskId;
        var type = request.Type ?? "feat";

        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        var config = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(config, project);
        var repositories = TaskCommand.ResolveRepositories(projectConfig, request.Only);
        CommandContext.Assert(repositories.Count > 0, "Task start should resolve at least one repository.");
        var adoContext = request.SkipAdo
            ? null
            : TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: false);
        WorkItemSnapshot? workItem = null;
        IReadOnlyList<WorkspaceWorkItem>? workItems = null;
        IReadOnlyList<WorkspaceChildTask>? childTasks = null;

        if (adoContext is not null)
        {
            var snapshots = ExpandSelection(adoContext, workItemsSelection, request.WithActiveChildren);
            RejectFinalStates(snapshots);
            RejectWorkspaceConflicts(context, root, project, snapshots.Select(snapshot => snapshot.Id));
            workItem = snapshots[0];
            workItems = snapshots
                .Select(snapshot => new WorkspaceWorkItem(snapshot.Id, snapshot.Type, snapshot.Title, snapshot.State))
                .ToArray();
            context.Out.WriteLine($"ADO item {workItem.Id}: {workItem.Type} - {workItem.Title}");
            foreach (var additional in snapshots.Skip(1))
            {
                context.Out.WriteLine($"ADO item {additional.Id}: {additional.Type} - {additional.Title}");
            }
            context.Debug($"ADO state courant: {workItem.State ?? "(vide)"}");

            if (workflow.TaskStart?.UpdateWorkItemState ?? true)
            {
                foreach (var snapshot in snapshots)
                {
                    var startState = AdoWorkflowStates.StartState(snapshot.Type, workflow.TaskStart);
                    if (!string.IsNullOrWhiteSpace(startState) &&
                        !string.Equals(snapshot.State, startState, StringComparison.OrdinalIgnoreCase))
                    {
                        TaskCommand.UpdateWorkItemState(adoContext.Client, adoContext.Token, snapshot.Id, startState, "dw task start");
                        context.Out.WriteLine($"ADO item {WorkspaceManifest.FormatWorkItem(new WorkspaceWorkItem(snapshot.Id, snapshot.Type, snapshot.Title, snapshot.State))}: etat -> {startState}");
                    }
                }
            }

            if (request.CreateChildTasks || (workflow.TaskStart?.CreateChildTasks ?? false))
            {
                childTasks = TaskCommand.CreateChildTasks(context, adoContext, workItem, repositories);
                if (string.IsNullOrWhiteSpace(taskId) && childTasks.Count == 1)
                {
                    taskId = childTasks[0].Id;
                }
            }
        }
        else if (!request.SkipAdo && workflow.AzureDevOps is not null)
        {
            throw new DwException("ADO requis pour verifier les work items. Utiliser dw auth login, DW_ADO_TOKEN, ou --skip-ado pour bypass explicite.", 2);
        }
        else
        {
            RejectWorkspaceConflicts(context, root, project, workItemsSelection.Ids);
        }

        var slug = TaskCommand.ResolveSlug(request.Slug, workItemId, workItem);
        context.Debug($"Slug normalise: {slug}");

        workItems ??= workItemsSelection.Ids.Select(id => new WorkspaceWorkItem(id)).ToArray();

        var branchWorkItemIds = workItems.Select(item => item.Id)
            .Concat(string.IsNullOrWhiteSpace(taskId) ? [] : [taskId])
            .Concat((childTasks ?? []).Select(task => task.Id).Where(id => !string.IsNullOrWhiteSpace(id)))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

        var subject = GitBranchNames.BuildSubjectName(type, workItems.Select(item => item.Id).ToArray(), slug);
        var branchName = GitBranchNames.Build(type, branchWorkItemIds, slug);
        var projectRoot = Path.Combine(root, "projects", project);
        var workspace = Path.Combine(projectRoot, "workspaces", subject);
        context.Debug($"Workspace cible: {workspace}");
        context.Debug($"Branche cible: {branchName}");

        context.FileSystem.CreateDirectory(workspace);
        var git = new GitWorktreeService(context.ProcessRunner, context.FileSystem);
        var results = new List<GitWorktreeResult>();

        foreach (var repositoryKey in repositories)
        {
            var repository = projectConfig?.Repositories.GetValueOrDefault(repositoryKey)
                ?? new RepositoryConfig("", "main", Folder: repositoryKey);
            var folder = string.IsNullOrWhiteSpace(repository.Folder) ? repositoryKey : repository.Folder;
            var result = git.PrepareAsync(projectRoot, repositoryKey, repository, branchName, Path.Combine(workspace, folder))
                .GetAwaiter()
                .GetResult();
            context.Debug($"Repo {repositoryKey}: {result.Status} - {result.Message}");

            if (result.Status == GitWorktreeStatus.Failed)
            {
                throw new DwException($"Creation worktree impossible pour {repositoryKey}: {result.Message}");
            }

            if (result.Status == GitWorktreeStatus.Prepared)
            {
                context.Out.WriteLine($"  fetch refspec configure pour {repositoryKey}");
            }

            results.Add(result);
        }

        var manifest = new WorkspaceManifest(1, workItemId, taskId, project, type, slug, branchName, context.Clock.Now, repositories, "created", workItem?.Type, workItem?.Title, workItem?.State, ChildTasks: childTasks, WorkItems: workItems);
        context.FileSystem.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(manifest));
        context.FileSystem.WriteAllText(Path.Combine(workspace, "plan.md"), Templates.PlanMd(manifest.ParentWorkItems, project));
        WriteWorkspaceAgentConfigs(context.FileSystem, workspace, manifest.ParentWorkItems, project);

        context.Out.WriteLine($"Workspace cree: {workspace}");
        context.Out.WriteLine($"Branche cible: {branchName}");
        foreach (var result in results)
        {
            context.Out.WriteLine($"Repo {result.Repository}: {result.Status} - {result.Message}");
        }

        context.Out.WriteLine("Prochaine etape:");
        context.Out.WriteLine($"  dw task open {workItemId} --project {project}");
        context.Out.WriteLine("Puis, pour un commit intermediaire:");
        context.Out.WriteLine("  dw task commit --continue --execute");
        context.Out.WriteLine("Et pour terminer avec push + PR:");
        context.Out.WriteLine("  dw task finish --continue --execute --create-pr");
        return 0;
    }

    private static IReadOnlyList<WorkItemSnapshot> ExpandSelection(AdoContext adoContext, WorkItemSet selection, bool withActiveChildren)
    {
        var snapshots = selection.Ids
            .Select(id => adoContext.Client.GetWorkItemSnapshotAsync(id, adoContext.Token).GetAwaiter().GetResult())
            .ToList();

        if (!withActiveChildren)
        {
            return snapshots;
        }

        var childIds = snapshots
            .SelectMany(snapshot => adoContext.Client
                .GetRelatedWorkItemIdsAsync(snapshot.Id, "System.LinkTypes.Hierarchy-Forward", adoContext.Token)
                .GetAwaiter()
                .GetResult())
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Where(id => snapshots.All(snapshot => !string.Equals(snapshot.Id, id, StringComparison.OrdinalIgnoreCase)))
            .ToArray();

        var childSnapshots = childIds
            .Select(id => adoContext.Client.GetWorkItemSnapshotAsync(id, adoContext.Token).GetAwaiter().GetResult())
            .Where(snapshot => !TaskCommand.IsFinalState(snapshot.Type, snapshot.State))
            .ToArray();

        snapshots.AddRange(childSnapshots);
        return snapshots;
    }

    private static void RejectFinalStates(IEnumerable<WorkItemSnapshot> snapshots)
    {
        var finalItems = snapshots
            .Where(snapshot => TaskCommand.IsFinalState(snapshot.Type, snapshot.State))
            .Select(snapshot => $"#{snapshot.Id} ({snapshot.State})")
            .ToArray();

        if (finalItems.Length > 0)
        {
            throw new DwException($"Impossible de demarrer un workspace avec des work items en etat final: {string.Join(", ", finalItems)}", 2);
        }
    }

    private static void RejectWorkspaceConflicts(CommandContext context, string root, string project, IEnumerable<string> workItemIds)
    {
        var selection = workItemIds.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        var conflicts = WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root)
            .Where(workspace => string.Equals(workspace.Manifest.Project, project, StringComparison.OrdinalIgnoreCase))
            .Where(workspace => selection.Any(workspace.Manifest.MatchesWorkItem))
            .Select(workspace => new
            {
                workspace.Path,
                MatchingIds = selection.Where(workspace.Manifest.MatchesWorkItem).ToArray()
            })
            .ToArray();

        if (conflicts.Length == 0)
        {
            return;
        }

        var details = string.Join("; ", conflicts.Select(conflict => $"{string.Join(", ", conflict.MatchingIds)} deja present(s) dans {conflict.Path}"));
        throw new DwException($"Workspace deja existant pour un des work items demandes: {details}", 2);
    }

    private static void WriteWorkspaceAgentConfigs(IFileSystem fileSystem, string workspace, IReadOnlyList<WorkspaceWorkItem> workItems, string project)
    {
        foreach (var file in AgentAdapterRegistry.WorkspaceConfigFiles(new AgentWorkspaceConfigRequest(workspace, workItems, project)))
        {
            fileSystem.WriteAllText(Path.Combine(workspace, file.RelativePath), file.Content);
        }
    }
}

internal sealed record TaskStartRequest(
    string WorkItemId,
    string? Project,
    string? TaskId,
    string? Type,
    string? Only,
    string? Slug,
    bool SkipAdo,
    bool CreateChildTasks,
    bool WithActiveChildren);
