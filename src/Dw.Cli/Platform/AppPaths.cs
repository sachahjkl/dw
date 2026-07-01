namespace Dw.Cli.Platform;

internal static class AppPaths
{
    public const string AppDirectoryName = "DevWorkflow";

    public static string DefaultRoot
    {
        get
        {
            var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            return Path.Combine(profile, "dev", "dw");
        }
    }

    public static string UserConfigDirectory
    {
        get
        {
            var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            return Path.Combine(local, AppDirectoryName);
        }
    }

    public static string UserSettingsPath => Path.Combine(UserConfigDirectory, "settings.json");
}
