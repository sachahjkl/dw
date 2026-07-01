using Dw.Cli.Agents;

namespace Dw.Cli.Workspaces;

internal sealed record WorkspaceSummary(string Path, WorkspaceManifest Manifest);

internal static class WorkspaceDiscoveryService
{
    public static IReadOnlyList<WorkspaceSummary> FindWorkspaces(IFileSystem fileSystem, string root)
        => fileSystem.EnumerateFiles(Path.Combine(root, "projects"), "task.json", SearchOption.AllDirectories)
            .Select(path => new WorkspaceSummary(Path.GetDirectoryName(path) ?? root, WorkspaceManifestReader.Read(fileSystem, path)))
            .OrderByDescending(workspace => workspace.Manifest.CreatedAt)
            .ToArray();

    public static IReadOnlyList<WorkspaceSummary> Filter(
        IReadOnlyList<WorkspaceSummary> workspaces,
        string? project,
        WorkItemSet? workItems)
        => workspaces
            .Where(workspace => string.IsNullOrWhiteSpace(project) || string.Equals(workspace.Manifest.Project, project, StringComparison.OrdinalIgnoreCase))
            .Where(workspace => workItems is null || workItems.Ids.All(workspace.Manifest.MatchesWorkItem))
            .ToArray();
}

internal sealed record WorkspaceOpenOptions(
    string? Workspace,
    string? Project,
    string? WorkItemId,
    bool Continue,
    string? PositionalWorkItemId = null,
    string? Agent = null,
    string? Repository = null);

internal static class WorkspaceOpenService
{
    public static int Open(CommandContext context, WorkspaceOpenOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        return Open(context, options, root);
    }

    internal static int Open(CommandContext context, WorkspaceOpenOptions options, string root)
    {
        var workflow = WorkflowConfigStore.Load(context.FileSystem, root);
        var projects = DevWorkflowConfigLoader.Load(context.FileSystem, root);
        var workspace = ResolveWorkspace(context, root, options);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        var projectConfig = DevWorkflowConfigLoader.ResolveProject(projects, manifest.Project);
        var target = ResolveOpenTarget(workspace, manifest, projectConfig, options.Repository);
        var adapter = AgentAdapterRegistry.Resolve(options.Agent ?? projectConfig?.Agent?.Default ?? workflow.Agent?.Default);
        var launch = adapter.BuildOpenLaunch(new AgentOpenRequest(root, target, options.Continue));

        foreach (var environmentVariable in launch.Environment)
        {
            context.Debug($"{environmentVariable.Key}={environmentVariable.Value}");
        }

        context.Debug($"{launch.FileName} {string.Join(' ', launch.Arguments)}");
        return context.ProcessRunner.RunInteractiveAsync(launch.FileName, launch.Arguments, launch.WorkingDirectory, launch.Environment).GetAwaiter().GetResult();
    }

    internal static string ResolveOpenTarget(string workspace, WorkspaceManifest manifest, ProjectConfig? projectConfig, string? repositoryKey)
    {
        if (string.IsNullOrWhiteSpace(repositoryKey))
        {
            return workspace;
        }

        if (!manifest.Repositories.Contains(repositoryKey, StringComparer.OrdinalIgnoreCase))
        {
            throw new DwException($"Repo absent du workspace: {repositoryKey}", 2);
        }

        var repository = projectConfig?.Repositories.GetValueOrDefault(repositoryKey)
            ?? new RepositoryConfig("", "main", Folder: repositoryKey);
        var folder = string.IsNullOrWhiteSpace(repository.Folder) ? repositoryKey : repository.Folder;
        return Path.Combine(workspace, folder);
    }

    internal static string ResolveWorkspace(CommandContext context, string root, WorkspaceOpenOptions options)
    {
        var workItems = ResolveWorkItemIds(options);

        if (!string.IsNullOrWhiteSpace(options.Workspace))
        {
            return Path.GetFullPath(Environment.ExpandEnvironmentVariables(options.Workspace));
        }

        var workspaces = WorkspaceDiscoveryService.Filter(
            WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root),
            options.Project,
            workItems);

        if (workspaces.Count == 0)
        {
            throw new DwException("Aucun workspace task trouve.", 2);
        }

        if (options.Continue || workspaces.Count == 1)
        {
            return workspaces[0].Path;
        }

        return AskWorkspace(context, workspaces);
    }

    private static WorkItemSet? ResolveWorkItemIds(WorkspaceOpenOptions options)
    {
        if (string.IsNullOrWhiteSpace(options.PositionalWorkItemId))
        {
            return WorkItemSet.ParseOptional(options.WorkItemId);
        }

        var positional = WorkItemSet.Parse(options.PositionalWorkItemId);
        var optional = WorkItemSet.ParseOptional(options.WorkItemId);
        if (optional is not null && !WorkItemSet.SetEquals(positional, optional))
        {
            throw new DwException("work-item-id et --work-item doivent pointer vers le meme work item.", 2);
        }

        return positional;
    }

    private static string AskWorkspace(CommandContext context, IReadOnlyList<WorkspaceSummary> workspaces)
    {
        var visible = workspaces;
        while (true)
        {
            PrintWorkspaceChoices(context, visible);
            context.Out.Write("Filtre ou numero: ");
            var input = Console.ReadLine()?.Trim();
            if (string.IsNullOrWhiteSpace(input))
            {
                return visible[0].Path;
            }

            if (int.TryParse(input, out var selected) && selected >= 1 && selected <= visible.Count)
            {
                return visible[selected - 1].Path;
            }

            var filtered = workspaces
                .Where(workspace => SearchText(workspace).Contains(input, StringComparison.OrdinalIgnoreCase))
                .ToArray();
            if (filtered.Length == 0)
            {
                context.Out.WriteLine("Aucun workspace ne correspond au filtre.");
                continue;
            }

            visible = filtered;
        }
    }

    private static void PrintWorkspaceChoices(CommandContext context, IReadOnlyList<WorkspaceSummary> workspaces)
    {
        context.Out.WriteLine("Workspaces disponibles:");
        context.Out.WriteLine();
        for (var i = 0; i < workspaces.Count; i++)
        {
            var workspace = workspaces[i];
            context.Out.WriteLine($"{i + 1}. {workspace.Manifest.Project} / {workspace.Manifest.DisplayWorkItems} / {workspace.Manifest.Type}-{workspace.Manifest.PrimaryWorkItemId}-{workspace.Manifest.Slug} / {workspace.Manifest.CreatedAt:yyyy-MM-dd}");
        }

        context.Out.WriteLine();
    }

    private static string SearchText(WorkspaceSummary workspace)
        => string.Join(' ',
            workspace.Path,
            workspace.Manifest.Project,
            workspace.Manifest.DisplayWorkItemIds,
            workspace.Manifest.TaskId,
            workspace.Manifest.Type,
            workspace.Manifest.Slug,
            workspace.Manifest.BranchName,
            workspace.Manifest.WorkItemTitle);
}
