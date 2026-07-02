namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private sealed record CompletionFilters(string? Project, WorkItemSet? WorkItems, string? Token);

    private static CompletionFilters Filters(ParseResult? parseResult, string? token = null, bool includeWorkItems = true)
        => new(
            parseResult?.GetValue<string>(OptionNames.Project),
            includeWorkItems ? WorkItemSet.ParseOptional(parseResult?.GetValue<string>(OptionNames.WorkItem)) : null,
            token);

    private static IEnumerable<CompletionItem> CompleteProjects(CommandContext context)
        => DevWorkflowConfigLoader.Load(context.FileSystem, Root(context))
            .Projects
            .OrderBy(pair => pair.Key, StringComparer.OrdinalIgnoreCase)
            .Select(pair => Item(pair.Key, pair.Value.DisplayName));

    private static IEnumerable<CompletionItem> CompleteWorkspaces(CommandContext context, CompletionFilters filters)
        => WorkspaceDiscoveryService.Filter(
                WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, Root(context)),
                filters.Project,
                filters.WorkItems)
            .OrderByDescending(workspace => workspace.Manifest.CreatedAt)
            .Select(workspace => Item(workspace.Path, $"{workspace.Manifest.Project} #{workspace.Manifest.DisplayWorkItemIds} {workspace.Manifest.Slug}"));

    private static IEnumerable<CompletionItem> CompleteRepositories(CommandContext context, CompletionFilters filters)
        => DevWorkflowConfigLoader.Load(context.FileSystem, Root(context))
            .Projects
            .Where(project => string.IsNullOrWhiteSpace(filters.Project)
                || string.Equals(project.Key, filters.Project, StringComparison.OrdinalIgnoreCase))
            .SelectMany(project => project.Value.Repositories.Select(repository => new { repository.Key, Project = project.Key, repository.Value.Folder }))
            .GroupBy(repository => repository.Key, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .Select(group => Item(group.Key, string.Join(", ", group.Select(repository => $"{repository.Project}/{repository.Folder ?? repository.Key}"))));

    private static IEnumerable<CompletionItem> CompleteWorkItems(CommandContext context, CompletionFilters filters)
        => PrefixForMultiValue(WorkspaceWorkItemCompletions(context, filters)
            .Concat(AssignedWorkItemCompletions(context, filters))
            .GroupBy(item => item.Label, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First()), filters.Token);

    private static IEnumerable<CompletionItem> CompleteDatabases(CommandContext context, CompletionFilters filters)
    {
        var config = DatabasesConfigLoader.Load(context.FileSystem, Root(context));
        var projectDatabases = !string.IsNullOrWhiteSpace(filters.Project)
            && config.Projects.TryGetValue(filters.Project, out var projectConfig)
            ? projectConfig.Databases.Keys
            : config.Projects.SelectMany(candidate => candidate.Value.Databases.Keys);
        return config.Globals.Keys
            .Concat(projectDatabases)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .OrderBy(name => name, StringComparer.OrdinalIgnoreCase)
            .Select(name => Item(name, "Base configuree dans databases.json"));
    }

    private static IEnumerable<CompletionItem> WorkspaceWorkItemCompletions(CommandContext context, CompletionFilters filters)
        => WorkspaceDiscoveryService.Filter(
                WorkspaceDiscoveryService.FindWorkspaces(context.FileSystem, Root(context)),
                filters.Project,
                workItems: null)
            .OrderByDescending(workspace => workspace.Manifest.CreatedAt)
            .SelectMany(workspace => workspace.Manifest.ParentWorkItems.Select(item => new { item.Id, workspace.Manifest.Project, item.Title }))
            .DistinctBy(workItem => workItem.Id)
            .Select(workItem => Item(workItem.Id, $"{workItem.Project} {workItem.Title}"));

    private static IEnumerable<CompletionItem> AssignedWorkItemCompletions(CommandContext context, CompletionFilters filters)
    {
        try
        {
            var (_, azureDevOps, token) = AdoClientFactory.CreateInputs(context, null, filters.Project);
            using var http = new HttpClient();
            var client = new AzureDevOpsClient(http, azureDevOps);
            return AdoCommand.FilterAssignedItems(client.GetAssignedWorkItemsAsync(100, token).GetAwaiter().GetResult(), includeFinalStates: false)
                .Select(item => Item(item.Id, $"{filters.Project ?? "ado"} {item.Title}"));
        }
        catch
        {
            return [];
        }
    }
}
