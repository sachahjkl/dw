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

    public static int List(CommandContext context, string[] args)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var project = CommandOptions.OptionValue(args, "--project");
        var workItemId = CommandOptions.OptionValue(args, "--work-item");
        var json = CommandOptions.HasFlag(args, "--json");
        var workspaces = WorkspaceDiscoveryService.Filter(
            WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, root),
            project,
            workItemId);

        if (workspaces.Count == 0)
        {
            context.Out.WriteLine("Aucun workspace task trouve.");
            return 0;
        }

        if (json)
        {
            var payload = workspaces.Select(workspace => new
            {
                path = workspace.Path,
                workspace.Manifest.Project,
                workspace.Manifest.WorkItemId,
                workspace.Manifest.TaskId,
                workspace.Manifest.Type,
                workspace.Manifest.Slug,
                workspace.Manifest.BranchName,
                workspace.Manifest.CreatedAt,
                workspace.Manifest.WorkItemType,
                workspace.Manifest.WorkItemTitle,
                workspace.Manifest.WorkItemState,
                workspace.Manifest.Repositories
            });
            context.Out.WriteLine(JsonSerializer.Serialize(payload, new JsonSerializerOptions { WriteIndented = true }));
            return 0;
        }

        context.Out.WriteLine("Project  WorkItem  Created     Branch");
        foreach (var workspace in workspaces)
        {
            context.Out.WriteLine($"{workspace.Manifest.Project,-8} {workspace.Manifest.WorkItemId,-8} {workspace.Manifest.CreatedAt:yyyy-MM-dd}  {workspace.Manifest.BranchName}");
            context.Out.WriteLine($"  {workspace.Path}");
        }

        return 0;
    }

    public static int Current(CommandContext context)
    {
        var workspace = WorkspaceCurrentService.FindWorkspacePath(context.FileSystem, Environment.CurrentDirectory)
            ?? throw new DwException("Aucun workspace task trouve depuis le dossier courant.", 2);
        var manifest = WorkspaceManifestReader.Read(context.FileSystem, Path.Combine(workspace, "task.json"));
        context.Out.WriteLine($"Workspace: {workspace}");
        context.Out.WriteLine($"Project: {manifest.Project}");
        context.Out.WriteLine($"Work item: {manifest.WorkItemId}");
        context.Out.WriteLine($"Branch: {manifest.BranchName}");
        context.Out.WriteLine($"Repos: {string.Join(", ", manifest.Repositories)}");
        return 0;
    }
}
