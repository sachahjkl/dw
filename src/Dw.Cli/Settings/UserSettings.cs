using System.Text.Json;

namespace Dw.Cli.Settings;

internal sealed record UserSettings(string? Root, string? Color = null)
{
    public static UserSettings Empty { get; } = new((string?)null, null);
}

internal static class UserSettingsStore
{
    public static UserSettings Load(IFileSystem fileSystem)
    {
        if (!fileSystem.FileExists(AppPaths.UserSettingsPath))
        {
            return UserSettings.Empty;
        }

        var json = fileSystem.ReadAllText(AppPaths.UserSettingsPath);
        return JsonSerializer.Deserialize(json, AppJsonContext.Default.UserSettings) ?? UserSettings.Empty;
    }

    public static void Save(IFileSystem fileSystem, UserSettings settings)
    {
        fileSystem.WriteAllText(AppPaths.UserSettingsPath, JsonSerializer.Serialize(settings, AppJsonContext.Default.UserSettings));
    }
}
