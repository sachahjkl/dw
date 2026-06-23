using Microsoft.Data.SqlClient;
using System.Data;

namespace Dw.Cli.Database;

internal sealed class SqlServerQueryService
{
    public async Task<QueryResult> QueryAsync(
        DatabaseConnectionConfig connection,
        DatabaseDefaults defaults,
        string sql,
        int? maxRowsOverride = null,
        CancellationToken cancellationToken = default)
    {
        var guard = SqlReadOnlyGuard.Validate(sql);
        if (!guard.IsAllowed)
        {
            throw new DwException($"Requete bloquee: {guard.Reason}", 2);
        }

        if (!string.Equals(connection.Provider, "sqlserver", StringComparison.OrdinalIgnoreCase))
        {
            throw new DwException($"Provider DB non supporte: {connection.Provider}");
        }

        var connectionString = ResolveConnectionString(connection);
        var builder = new SqlConnectionStringBuilder(connectionString)
        {
            ApplicationIntent = ApplicationIntent.ReadOnly
        };

        var maxRows = maxRowsOverride ?? connection.MaxRows ?? defaults.MaxRows;
        var timeout = connection.TimeoutSeconds ?? defaults.TimeoutSeconds;
        await using var sqlConnection = new SqlConnection(builder.ConnectionString);
        await sqlConnection.OpenAsync(cancellationToken);
        await using var command = sqlConnection.CreateCommand();
        command.CommandText = sql;
        command.CommandTimeout = timeout;
        command.CommandType = CommandType.Text;

        await using var reader = await command.ExecuteReaderAsync(cancellationToken);
        var columns = Enumerable.Range(0, reader.FieldCount)
            .Select(reader.GetName)
            .ToArray();

        var rows = new List<string?[]>();
        while ((maxRows <= 0 || rows.Count < maxRows) && await reader.ReadAsync(cancellationToken))
        {
            var values = new string?[reader.FieldCount];
            for (var i = 0; i < reader.FieldCount; i++)
            {
                values[i] = await reader.IsDBNullAsync(i, cancellationToken)
                    ? null
                    : Convert.ToString(reader.GetValue(i), System.Globalization.CultureInfo.InvariantCulture);
            }

            rows.Add(values);
        }

        return new QueryResult(columns, rows, maxRows > 0 && rows.Count == maxRows);
    }

    private static string ResolveConnectionString(DatabaseConnectionConfig connection)
    {
        if (!string.IsNullOrWhiteSpace(connection.ConnectionString))
        {
            return connection.ConnectionString;
        }

        if (!string.IsNullOrWhiteSpace(connection.ConnectionStringEnvironmentVariable))
        {
            var value = Environment.GetEnvironmentVariable(connection.ConnectionStringEnvironmentVariable);
            if (!string.IsNullOrWhiteSpace(value))
            {
                return value;
            }
        }

        if (!string.IsNullOrWhiteSpace(connection.CredentialKey))
        {
            var value = SecretStoreFactory.Create().Get(connection.CredentialKey);
            if (!string.IsNullOrWhiteSpace(value))
            {
                return value;
            }
        }

        throw new DwException("Connection string SQL introuvable. Renseigner connectionString, connectionStringEnvironmentVariable ou credentialKey.");
    }
}

internal sealed record QueryResult(IReadOnlyList<string> Columns, IReadOnlyList<string?[]> Rows, bool Truncated);
