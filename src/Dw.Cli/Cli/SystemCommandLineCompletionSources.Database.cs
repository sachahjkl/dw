namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
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
            var project = parse.GetValue<string>(OptionNames.Project) ?? "default";
            var database = parse.GetValue<string>(OptionNames.Database) ?? parse.GetValue<string>(OptionNames.Env) ?? "dev";
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
            var project = parse.GetValue<string>(OptionNames.Project) ?? "default";
            var database = parse.GetValue<string>(OptionNames.Database) ?? parse.GetValue<string>(OptionNames.Env) ?? "dev";
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
