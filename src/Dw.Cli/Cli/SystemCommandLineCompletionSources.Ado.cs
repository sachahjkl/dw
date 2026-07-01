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
            .Select(workspace => Item(workspace.Path, $"{workspace.Manifest.Project} #{workspace.Manifest.DisplayWorkItemIds} {workspace.Manifest.Slug}")));

    private static IEnumerable<CompletionItem> WorkItemCompletions(CommandContext context, CompletionContext? completion = null)
        => SafeCompletions(() => PrefixForMultiValue(WorkspaceWorkItemCompletions(context)
            .Concat(AssignedWorkItemCompletions(context, completion))
            .GroupBy(item => item.Label, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First()), completion));

    private static IEnumerable<CompletionItem> RepositoryCompletions(CommandContext context)
        => SafeCompletions(() => DevWorkflowConfigLoader.Load(context.FileSystem, Root(context))
            .Projects
            .SelectMany(project => project.Value.Repositories.Select(repository => new { repository.Key, Project = project.Key, repository.Value.Folder }))
            .GroupBy(repository => repository.Key, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .Select(group => Item(group.Key, string.Join(", ", group.Select(repository => $"{repository.Project}/{repository.Folder ?? repository.Key}")))));

    private static IEnumerable<CompletionItem> WorkspaceWorkItemCompletions(CommandContext context)
        => WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, Root(context))
            .OrderByDescending(workspace => workspace.Manifest.CreatedAt)
            .SelectMany(workspace => workspace.Manifest.ParentWorkItems.Select(item => new { item.Id, workspace.Manifest.Project, item.Title }))
            .DistinctBy(workItem => workItem.Id)
            .Select(workItem => Item(workItem.Id, $"{workItem.Project} {workItem.Title}"));

    private static IEnumerable<CompletionItem> AssignedWorkItemCompletions(CommandContext context, CompletionContext? completion)
    {
        if (completion?.ParseResult is null)
        {
            return [];
        }

        var project = completion.ParseResult.GetValue<string>(OptionNames.Project);
        try
        {
            var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, null, project);
            using var http = new HttpClient();
            var client = new AzureDevOpsClient(http, azureDevOps);
            return AdoCommand.FilterAssignedItems(client.GetAssignedWorkItemsAsync(100, token).GetAwaiter().GetResult(), includeFinalStates: false)
                .Select(item => Item(item.Id, $"{project ?? "ado"} {item.Title}"));
        }
        catch
        {
            return [];
        }
    }
}
