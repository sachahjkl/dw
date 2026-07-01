using System.Text.Json;

namespace Dw.Cli.Workspaces;

internal static class TaskListService
{
    public static int Status(CommandContext context)
    {
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        context.Out.WriteLine($"Root: {root}");
        context.Out.WriteLine("Workspaces detectes:");

        var files = context.FileSystem.EnumerateFiles(Path.Combine(root, "projects"), "task.json", SearchOption.AllDirectories).ToList();
        if (files.Count == 0)
        {
            context.Out.WriteLine("  Aucun workspace task trouve.");
            return 0;
        }

        foreach (var file in files)
        {
            context.Out.WriteLine($"  {Path.GetDirectoryName(file)}");
        }

        return 0;
    }

    public static int List(CommandContext context, TaskListOptions options)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var workspaces = WorkspaceDiscoveryService.Filter(
            WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root),
            options.Project,
            WorkItemSet.ParseOptional(options.WorkItemId));

        if (workspaces.Count == 0)
        {
            context.Out.WriteLine("Aucun workspace task trouve.");
            return 0;
        }

        if (options.Json)
        {
            var payload = workspaces.Select(workspace => new TaskListItem(
                workspace.Path,
                workspace.Manifest.Project,
                workspace.Manifest.DisplayWorkItemIds,
                workspace.Manifest.TaskId,
                workspace.Manifest.Type,
                workspace.Manifest.Slug,
                workspace.Manifest.BranchName,
                workspace.Manifest.CreatedAt,
                workspace.Manifest.WorkItemType,
                workspace.Manifest.WorkItemTitle,
                workspace.Manifest.WorkItemState,
                workspace.Manifest.Repositories)).ToArray();
            context.Out.WriteLine(JsonSerializer.Serialize(payload, AppJsonContext.Default.TaskListItemArray));
            return 0;
        }

        context.Out.WriteLine("Project  WorkItem  Created     Branch");
        foreach (var workspace in workspaces)
        {
            context.Out.WriteLine($"{workspace.Manifest.Project,-8} {workspace.Manifest.DisplayWorkItemIds,-8} {workspace.Manifest.CreatedAt:yyyy-MM-dd}  {workspace.Manifest.BranchName}");
            context.Out.WriteLine($"  {workspace.Path}");
        }

        return 0;
    }

    public static int Current(CommandContext context, bool json)
    {
        var workspace = WorkspaceCurrentService.FindWorkspacePath(context.FileSystem, Environment.CurrentDirectory)
            ?? throw new DwException("Aucun workspace task trouve depuis le dossier courant.", 2);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        if (json)
        {
            context.Out.WriteLine(JsonSerializer.Serialize(new TaskCurrentItem(
                workspace,
                manifest.Project,
                manifest.PrimaryWorkItemId,
                manifest.ParentWorkItems,
                manifest.TaskId,
                manifest.LegacyChildTaskIds,
                manifest.NormalizedChildTasks,
                manifest.BranchName,
                manifest.Repositories)));
            return 0;
        }

        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Project: {manifest.Project}");
        context.Out.WriteLine($"Work items: {manifest.DisplayWorkItemIds}");
        context.Out.WriteLine($"Branch: {manifest.BranchName}");
        context.Out.WriteLine($"Repos: {string.Join(", ", manifest.Repositories)}");
        return 0;
    }
}

internal sealed record TaskListOptions(string? Project, string? WorkItemId, bool Json);

internal sealed record TaskListItem(
    string Path,
    string Project,
    string WorkItemId,
    string? TaskId,
    string Type,
    string Slug,
    string BranchName,
    DateTimeOffset CreatedAt,
    string? WorkItemType,
    string? WorkItemTitle,
    string? WorkItemState,
    IReadOnlyList<string> Repositories);

internal sealed record TaskCurrentItem(
    string Workspace,
    string Project,
    string PrimaryWorkItemId,
    IReadOnlyList<WorkspaceWorkItem> WorkItems,
    string? TaskId,
    IReadOnlyDictionary<string, string>? ChildTaskIds,
    IReadOnlyList<WorkspaceChildTask> ChildTasks,
    string Branch,
    IReadOnlyList<string> Repositories);
