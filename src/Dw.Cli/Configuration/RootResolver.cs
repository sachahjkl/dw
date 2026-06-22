namespace Dw.Cli.Configuration;

internal static class RootResolver
{
    public static string Resolve(CommandContext context, string? configuredRoot)
    {
        if (!string.IsNullOrWhiteSpace(configuredRoot))
        {
            return Path.GetFullPath(Environment.ExpandEnvironmentVariables(configuredRoot));
        }

        return UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
    }
}
