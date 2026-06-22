using Dw.Cli.Agents;

namespace Dw.Cli.Workspaces;

internal static class TaskStartService
{
    public static int Start(CommandContext context, TaskStartCommandOptions options)
    {
        var workItemId = options.WorkItemId;
        var project = options.Project ?? "default";
        var taskId = options.TaskId;
        var type = options.Type ?? "feat";

        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        var config = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(config, project);
        var repositories = TaskCommand.ResolveRepositories(projectConfig, options.Only);
        CommandContext.Assert(repositories.Count > 0, "Task start should resolve at least one repository.");
        var adoContext = options.SkipAdo
            ? null
            : TaskCommand.TryCreateAdoContext(context, workflow, projectConfig, required: false);
        WorkItemSnapshot? workItem = null;
        IReadOnlyDictionary<string, string>? childTaskIds = null;

        if (adoContext is not null)
        {
            workItem = adoContext.Client.GetWorkItemSnapshotAsync(workItemId, adoContext.Token).GetAwaiter().GetResult();
            context.Out.WriteLine($"ADO item {workItem.Id}: {workItem.Type} - {workItem.Title}");
            context.Debug($"ADO state courant: {workItem.State ?? "(vide)"}");

            if (workflow.TaskStart?.UpdateWorkItemState ?? true)
            {
                var startState = AdoWorkflowStates.StartState(workItem.Type, workflow.TaskStart);
                if (!string.IsNullOrWhiteSpace(startState) &&
                    !string.Equals(workItem.State, startState, StringComparison.OrdinalIgnoreCase))
                {
                    TaskCommand.UpdateWorkItemState(adoContext.Client, adoContext.Token, workItem.Id, startState, "dw task start");
                    context.Out.WriteLine($"ADO item {workItem.Id}: etat -> {startState}");
                }
            }

            if (options.CreateChildTasks || (workflow.TaskStart?.CreateChildTasks ?? false))
            {
                childTaskIds = TaskCommand.CreateChildTasks(context, adoContext, workItem, repositories);
                if (string.IsNullOrWhiteSpace(taskId) && childTaskIds.Count == 1)
                {
                    taskId = childTaskIds.Values.First();
                }
            }
        }
        else if (!options.SkipAdo && workflow.AzureDevOps is not null)
        {
            context.Out.WriteLine("ADO ignore: aucun token silencieux disponible. Utiliser dw auth login, DW_ADO_TOKEN, ou --skip-ado.");
        }

        var slug = TaskCommand.ResolveSlug(options.Slug, workItemId, workItem);
        context.Debug($"Slug normalise: {slug}");

        var subject = GitBranchNames.BuildSubjectName(type, workItemId, slug);
        var branchName = GitBranchNames.Build(type, workItemId, taskId, slug);
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

            results.Add(result);
        }

        var manifest = new WorkspaceManifest(1, workItemId, taskId, project, type, slug, branchName, context.Clock.Now, repositories, "created", workItem?.Type, workItem?.Title, workItem?.State, childTaskIds);
        context.FileSystem.WriteAllText(Path.Combine(workspace, "task.json"), WorkspaceManifestWriter.Serialize(manifest));
        context.FileSystem.WriteAllText(Path.Combine(workspace, "plan.md"), Templates.PlanMd(workItemId, project));
        WriteWorkspaceAgentConfigs(context.FileSystem, workspace, workItemId, project);

        context.Out.WriteLine($"Workspace cree: {workspace}");
        context.Out.WriteLine($"Branche cible: {branchName}");
        foreach (var result in results)
        {
            context.Out.WriteLine($"Repo {result.Repository}: {result.Status} - {result.Message}");
        }

        context.Out.WriteLine("Prochaine etape: ouvrir ce dossier avec OpenCode/Codex et executer dw agent context.");
        return 0;
    }

    private static void WriteWorkspaceAgentConfigs(IFileSystem fileSystem, string workspace, string workItemId, string project)
    {
        foreach (var file in AgentAdapterRegistry.WorkspaceConfigFiles(new AgentWorkspaceConfigRequest(workspace, workItemId, project)))
        {
            fileSystem.WriteAllText(Path.Combine(workspace, file.RelativePath), file.Content);
        }
    }
}

internal sealed record TaskStartCommandOptions(
    string WorkItemId,
    string? Project,
    string? TaskId,
    string? Type,
    string? Only,
    string? Slug,
    bool SkipAdo,
    bool CreateChildTasks);
