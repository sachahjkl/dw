namespace Dw.Cli.Commands;

internal static class DbCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        var sub = args.FirstOrDefault()?.ToLowerInvariant();
        return sub switch
        {
            "schema" => Schema(context, args.Skip(1).ToArray()),
            "describe" => Describe(context, args.Skip(1).ToArray()),
            "query" => Query(context, args.Skip(1).ToArray()),
            _ => Help(context)
        };
    }

    private static int Help(CommandContext context)
    {
        context.Out.WriteLine("Usage: dw db <schema|describe|query>");
        context.Out.WriteLine("SQL Server sera read-only par defaut: SELECT/introspection uniquement, maxRows=500, timeout=10min.");
        return 0;
    }

    private static int Query(CommandContext context, string[] args)
    {
        var project = OptionValue(args, "--project") ?? "default";
        var database = OptionValue(args, "--database") ?? OptionValue(args, "--env") ?? "dev";
        var sql = RemainingSql(args);
        var guard = SqlReadOnlyGuard.Validate(sql);
        if (!guard.IsAllowed)
        {
            throw new DwException($"Requete bloquee: {guard.Reason}", 2);
        }

        var (connection, defaults) = ResolveConnection(context, project, database);
        var result = new SqlServerQueryService().QueryAsync(connection, defaults, sql).GetAwaiter().GetResult();
        QueryResultPrinter.Print(context.Out, result);
        return 0;
    }

    private static int Schema(CommandContext context, string[] args)
    {
        var project = OptionValue(args, "--project") ?? "default";
        var database = OptionValue(args, "--database") ?? OptionValue(args, "--env") ?? "dev";
        var sql = """
select TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE
from INFORMATION_SCHEMA.TABLES
order by TABLE_SCHEMA, TABLE_NAME
""";
        var (connection, defaults) = ResolveConnection(context, project, database);
        var result = new SqlServerQueryService().QueryAsync(connection, defaults, sql).GetAwaiter().GetResult();
        QueryResultPrinter.Print(context.Out, result);
        return 0;
    }

    private static int Describe(CommandContext context, string[] args)
    {
        var table = args.FirstOrDefault(arg => !arg.StartsWith("--", StringComparison.Ordinal));
        if (string.IsNullOrWhiteSpace(table))
        {
            throw new DwException("Usage: dw db describe <schema.table> [--project <project>] [--database <name>]", 2);
        }

        var project = OptionValue(args, "--project") ?? "default";
        var database = OptionValue(args, "--database") ?? OptionValue(args, "--env") ?? "dev";
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
        var result = new SqlServerQueryService().QueryAsync(connection, defaults, sql).GetAwaiter().GetResult();
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
        if (!config.Projects.TryGetValue(project, out var projectDatabases) ||
            !projectDatabases.Databases.TryGetValue(database, out var connection))
        {
            throw new DwException($"Base introuvable dans databases.json: {project}/{database}");
        }

        if (connection.Readonly == false || !config.Defaults.Readonly)
        {
            throw new DwException("Execution SQL refusee: readonly doit rester true.");
        }

        return (connection, config.Defaults);
    }

    private static string RemainingSql(string[] args)
    {
        var parts = new List<string>();
        for (var i = 0; i < args.Length; i++)
        {
            if (args[i].StartsWith("--", StringComparison.Ordinal) && i + 1 < args.Length)
            {
                i++;
                continue;
            }

            if (!args[i].StartsWith("--", StringComparison.Ordinal))
            {
                parts.Add(args[i]);
            }
        }

        return string.Join(' ', parts);
    }

    private static string? OptionValue(string[] args, string name)
    {
        for (var i = 0; i < args.Length - 1; i++)
        {
            if (string.Equals(args[i], name, StringComparison.OrdinalIgnoreCase))
            {
                return args[i + 1];
            }
        }

        return null;
    }
}
