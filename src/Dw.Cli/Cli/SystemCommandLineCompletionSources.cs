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

    private static IEnumerable<CompletionItem> TableCompletions(CommandContext context, CompletionContext completion)
        => SafeCompletions(() =>
        {
            var parse = completion.ParseResult;
            var project = parse.GetValue<string>("--project") ?? "default";
            var database = parse.GetValue<string>("--database") ?? parse.GetValue<string>("--env") ?? "dev";
            var root = Root(context);
            var config = DatabasesConfigLoader.Load(context.FileSystem, root);
            if (!DbCommand.TryResolveConnection(config, project, database, out var connection) || connection is null)
            {
                return [];
            }

            if (connection.Readonly is false || !config.Defaults.Readonly)
            {
                return [];
            }

            var tables = new SqlServerQueryService().ListTablesAsync(connection, config.Defaults).GetAwaiter().GetResult();
            return tables.Select(table => Item(table, "Table SQL"));
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

        var project = completion.ParseResult.GetValue<string>("--project");
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

    private static IEnumerable<CompletionItem> PrefixForMultiValue(IEnumerable<CompletionItem> completions, CompletionContext? completion)
    {
        var token = completion?.WordToComplete;
        if (string.IsNullOrWhiteSpace(token) || !token.Contains(',', StringComparison.Ordinal))
        {
            return completions;
        }

        var lastSeparator = token.LastIndexOf(',');
        if (lastSeparator < 0)
        {
            return completions;
        }

        var prefix = token[..(lastSeparator + 1)];
        return completions.Select(item => new CompletionItem(
            label: prefix + item.Label,
            kind: item.Kind ?? string.Empty,
            sortText: item.SortText,
            insertText: prefix + (string.IsNullOrWhiteSpace(item.InsertText) ? item.Label : item.InsertText),
            documentation: item.Documentation,
            detail: item.Detail));
    }

    private static IEnumerable<CompletionItem> SqlQueryCompletions(CommandContext context, CompletionContext completion)
    {
        var word = completion.WordToComplete ?? string.Empty;

        var keywords = new[] { "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "IN", "BETWEEN", "LIKE", "JOIN", "INNER", "LEFT", "RIGHT", "OUTER", "ON", "AS", "ORDER", "BY", "ASC", "DESC", "GROUP", "HAVING", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "TOP", "DISTINCT", "CASE", "WHEN", "THEN", "ELSE", "END", "IS", "NULL", "COUNT", "SUM", "AVG", "MIN", "MAX", "UNION", "ALL", "EXISTS", "WITH", "OFFSET", "FETCH", "NEXT", "ROWS", "CAST", "COALESCE", "CONVERT", "GETDATE", "DATEADD", "DATEDIFF", "YEAR", "MONTH", "DAY" }
            .Where(keyword => keyword.StartsWith(word, StringComparison.OrdinalIgnoreCase))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Select(keyword => Item(keyword, "Mot-cle SQL"));

        return SafeCompletions(() =>
        {
            var parse = completion.ParseResult;
            var project = parse.GetValue<string>("--project") ?? "default";
            var database = parse.GetValue<string>("--database") ?? parse.GetValue<string>("--env") ?? "dev";
            var root = Root(context);
            var config = DatabasesConfigLoader.Load(context.FileSystem, root);
            if (!DbCommand.TryResolveConnection(config, project, database, out var connection) || connection is null)
            {
                return keywords;
            }

            if (connection.Readonly is false || !config.Defaults.Readonly)
            {
                return keywords;
            }

            var tables = new SqlServerQueryService().ListTablesAsync(connection, config.Defaults).GetAwaiter().GetResult();
            return keywords.Concat(tables
                .Where(table => table.StartsWith(word, StringComparison.OrdinalIgnoreCase))
                .Select(table => Item(table, "Table SQL")));
        });
    }
}
