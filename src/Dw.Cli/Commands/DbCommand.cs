using System.Diagnostics.CodeAnalysis;

namespace Dw.Cli.Commands;

internal static class DbCommand
{
    internal static int Query(CommandContext context, string? project, string? database, string? env, int? maxRows, IReadOnlyList<string> sqlTokens)
    {
        project ??= "default";
        database ??= env ?? "dev";
        var sql = string.Join(' ', sqlTokens);
        var guard = SqlReadOnlyGuard.Validate(sql);
        if (!guard.IsAllowed)
        {
            throw new DwException($"Requete bloquee: {guard.Reason}", 2);
        }

        var (connection, defaults) = ResolveConnection(context, project, database);
        var result = new SqlServerQueryService().QueryAsync(connection, defaults, sql, maxRows).GetAwaiter().GetResult();
        QueryResultPrinter.Print(context.Out, result);
        return 0;
    }

    internal static int Schema(CommandContext context, string? project, string? database, string? env)
    {
        project ??= "default";
        database ??= env ?? "dev";
        var sql = """
select TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE
from INFORMATION_SCHEMA.TABLES
order by TABLE_SCHEMA, TABLE_NAME
""";
        var (connection, defaults) = ResolveConnection(context, project, database);
        var result = new SqlServerQueryService().QueryAsync(connection, defaults, sql, maxRowsOverride: 0).GetAwaiter().GetResult();
        QueryResultPrinter.Print(context.Out, result);
        return 0;
    }

    internal static int Describe(CommandContext context, string? project, string? database, string? env, string table)
    {
        project ??= "default";
        database ??= env ?? "dev";
        var parts = table.Split('.', 2);
        var schema = parts.Length == 2 ? parts[0] : "dbo";
        var name = parts.Length == 2 ? parts[1] : parts[0];
        var sql = $"""
select COLUMN_NAME, DATA_TYPE, IS_NULLABLE, CHARACTER_MAXIMUM_LENGTH
from INFORMATION_SCHEMA.COLUMNS
where TABLE_SCHEMA = '{schema.Replace("'", "''")}'
  and TABLE_NAME = '{name.Replace("'", "''")}'
order by ORDINAL_POSITION
""";

        var (connection, defaults) = ResolveConnection(context, project, database);
        var result = new SqlServerQueryService().QueryAsync(connection, defaults, sql, maxRowsOverride: 0).GetAwaiter().GetResult();
        QueryResultPrinter.Print(context.Out, result);
        return 0;
    }

    private static (DatabaseConnectionConfig Connection, DatabaseDefaults Defaults) ResolveConnection(
        CommandContext context,
        string project,
        string database)
    {
        var root = UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
        var config = DatabasesConfigLoader.Load(context.FileSystem, root);
        var resolvedConnection = ResolveConnectionOrThrow(config, project, database);
        if (resolvedConnection is { Readonly: false } || !config.Defaults.Readonly)
        {
            throw new DwException("Execution SQL refusee: readonly doit rester true.");
        }

        return (resolvedConnection, config.Defaults);
    }

    internal static bool TryResolveConnection(
        DatabasesConfig config,
        string project,
        string database,
        [NotNullWhen(true)] out DatabaseConnectionConfig? connection)
    {
        if (config.Projects.TryGetValue(project, out var projectDatabases) &&
            projectDatabases.Databases.TryGetValue(database, out var projectConnection))
        {
            connection = projectConnection;
            return true;
        }

        if (config.Globals.TryGetValue(database, out var globalConnection))
        {
            connection = globalConnection;
            return true;
        }

        connection = null;
        return false;
    }

    private static DatabaseConnectionConfig ResolveConnectionOrThrow(DatabasesConfig config, string project, string database)
    {
        if (TryResolveConnection(config, project, database, out var connection) && connection is not null)
        {
            return connection;
        }

        throw new DwException($"Base introuvable dans databases.json: {project}/{database}");
    }

}
