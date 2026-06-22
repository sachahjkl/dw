namespace Dw.Cli.Tests;

public sealed class DbCommandTests
{
    [Fact]
    public void TryResolveConnection_prefers_project_database_before_global()
    {
        var config = new DatabasesConfig(
            new DatabaseDefaults(true, 500, 600),
            new Dictionary<string, DatabaseConnectionConfig>(StringComparer.OrdinalIgnoreCase)
            {
                ["shared"] = new("sqlserver", "global", null, null, true, null, null)
            },
            new Dictionary<string, ProjectDatabases>(StringComparer.OrdinalIgnoreCase)
            {
                ["ha"] = new(new Dictionary<string, DatabaseConnectionConfig>(StringComparer.OrdinalIgnoreCase)
                {
                    ["shared"] = new("sqlserver", "project", null, null, true, null, null)
                })
            });

        var found = DbCommand.TryResolveConnection(config, "ha", "shared", out var connection);

        Assert.True(found);
        Assert.NotNull(connection);
        if (connection is null)
        {
            throw new InvalidOperationException("Connection should not be null when found is true.");
        }

        Assert.Equal("project", connection.ConnectionString);
    }

    [Fact]
    public void TryResolveConnection_falls_back_to_global_database()
    {
        var config = new DatabasesConfig(
            new DatabaseDefaults(true, 500, 600),
            new Dictionary<string, DatabaseConnectionConfig>(StringComparer.OrdinalIgnoreCase)
            {
                ["shared"] = new("sqlserver", "global", null, null, true, null, null)
            },
            new Dictionary<string, ProjectDatabases>(StringComparer.OrdinalIgnoreCase));

        var found = DbCommand.TryResolveConnection(config, "ha", "shared", out var connection);

        Assert.True(found);
        Assert.NotNull(connection);
        if (connection is null)
        {
            throw new InvalidOperationException("Connection should not be null when found is true.");
        }

        Assert.Equal("global", connection.ConnectionString);
    }
}
