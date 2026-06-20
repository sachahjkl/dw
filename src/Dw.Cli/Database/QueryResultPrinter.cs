namespace Dw.Cli.Database;

internal static class QueryResultPrinter
{
    public static void Print(TextWriter writer, QueryResult result)
    {
        writer.WriteLine(string.Join('\t', result.Columns));
        foreach (var row in result.Rows)
        {
            writer.WriteLine(string.Join('\t', row.Select(value => value ?? "NULL")));
        }

        writer.WriteLine(result.Truncated
            ? $"-- {result.Rows.Count} rows (truncated)"
            : $"-- {result.Rows.Count} rows");
    }
}
