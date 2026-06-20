using System.Text.Json;

namespace Dw.Cli.Settings;

internal sealed record UserSettings(string? Root)
{
    public static UserSettings Empty { get; } = new((string?)null);
}

internal static class UserSettingsStore
{
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true
    };

    public static UserSettings Load(IFileSystem fileSystem)
    {
        if (!fileSystem.FileExists(AppPaths.UserSettingsPath))
        {
            return UserSettings.Empty;
        }

        var json = fileSystem.ReadAllText(AppPaths.UserSettingsPath);
        return JsonSerializer.Deserialize<UserSettings>(json, Options) ?? UserSettings.Empty;
    }

    public static void Save(IFileSystem fileSystem, UserSettings settings)
    {
        fileSystem.WriteAllText(AppPaths.UserSettingsPath, JsonSerializer.Serialize(settings, Options));
    }
}
