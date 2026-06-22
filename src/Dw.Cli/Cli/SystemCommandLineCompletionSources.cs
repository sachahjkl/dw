namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static IEnumerable<CompletionItem> ProjectCompletions(CommandContext context)
        => SafeCompletions(() => DevWorkflowConfigLoader.Load(context.FileSystem, Root(context))
            .Projects
            .OrderBy(pair => pair.Key, StringComparer.OrdinalIgnoreCase)
            .Select(pair => Item(pair.Key, pair.Value.DisplayName)));

    private static IEnumerable<CompletionItem> WorkspaceCompletions(CommandContext context)
        => SafeCompletions(() => WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, Root(context))
            .OrderByDescending(workspace => workspace.Manifest.CreatedAt)
            .Select(workspace => Item(workspace.Path, $"{workspace.Manifest.Project} #{workspace.Manifest.WorkItemId} {workspace.Manifest.Slug}")));

    private static IEnumerable<CompletionItem> WorkItemCompletions(CommandContext context)
        => SafeCompletions(() => WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, Root(context))
            .OrderByDescending(workspace => workspace.Manifest.CreatedAt)
            .Select(workspace => new { workspace.Manifest.WorkItemId, workspace.Manifest.Project, workspace.Manifest.WorkItemTitle })
            .DistinctBy(workItem => workItem.WorkItemId)
            .Select(workItem => Item(workItem.WorkItemId, $"{workItem.Project} {workItem.WorkItemTitle}")));

    private static IEnumerable<CompletionItem> RepositoryCompletions(CommandContext context)
        => SafeCompletions(() => DevWorkflowConfigLoader.Load(context.FileSystem, Root(context))
            .Projects
            .SelectMany(project => project.Value.Repositories.Select(repository => new { repository.Key, Project = project.Key, repository.Value.Folder }))
            .GroupBy(repository => repository.Key, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .Select(group => Item(group.Key, string.Join(", ", group.Select(repository => $"{repository.Project}/{repository.Folder ?? repository.Key}")))));

    private static IEnumerable<CompletionItem> DatabaseCompletions(CommandContext context)
        => SafeCompletions(() =>
        {
            var config = DatabasesConfigLoader.Load(context.FileSystem, Root(context));
            return config.Globals.Keys
                .Concat(config.Projects.SelectMany(project => project.Value.Databases.Keys))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(name => name, StringComparer.OrdinalIgnoreCase)
                .Select(name => Item(name, "Base configuree dans databases.json"));
        });

    private static string Root(CommandContext context)
        => UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;

    private static CompletionItem Item(string label, string? detail = null)
        => new(label: label, kind: string.Empty, sortText: string.Empty, insertText: string.Empty, documentation: detail ?? string.Empty, detail: detail ?? string.Empty);

    private static IEnumerable<CompletionItem> SafeCompletions(Func<IEnumerable<CompletionItem>> completions)
    {
        try
        {
            return completions();
        }
        catch
        {
            return [];
        }
    }
}
