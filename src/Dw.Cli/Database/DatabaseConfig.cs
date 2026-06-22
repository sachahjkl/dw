using System.Text.Json;

namespace Dw.Cli.Database;

internal sealed record DatabasesConfig(
    DatabaseDefaults Defaults,
    IReadOnlyDictionary<string, DatabaseConnectionConfig> Globals,
    IReadOnlyDictionary<string, ProjectDatabases> Projects)
{
    public static DatabasesConfig Empty { get; } = new(new DatabaseDefaults(true, 500, 600), new Dictionary<string, DatabaseConnectionConfig>(StringComparer.OrdinalIgnoreCase), new Dictionary<string, ProjectDatabases>(StringComparer.OrdinalIgnoreCase));
}

internal sealed record DatabaseDefaults(bool Readonly, int MaxRows, int TimeoutSeconds);

internal sealed record ProjectDatabases(IReadOnlyDictionary<string, DatabaseConnectionConfig> Databases);

internal sealed record DatabaseConnectionConfig(
    string Provider,
    string? ConnectionString,
    string? ConnectionStringEnvironmentVariable,
    string? CredentialKey,
    bool? Readonly,
    int? MaxRows,
    int? TimeoutSeconds);

internal static class DatabasesConfigLoader
{
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        ReadCommentHandling = JsonCommentHandling.Skip,
        AllowTrailingCommas = true
    };

    public static DatabasesConfig Load(IFileSystem fileSystem, string root)
    {
        var path = Path.Combine(root, "config", "databases.json");
        if (!fileSystem.FileExists(path))
        {
            return DatabasesConfig.Empty;
        }

        return JsonSerializer.Deserialize<DatabasesConfig>(fileSystem.ReadAllText(path), Options)
               ?? DatabasesConfig.Empty;
    }
}
