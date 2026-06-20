namespace Dw.Cli.Commands;

internal static class CommandOptions
{
    public static string ResolveRoot(CommandContext context, string[] args)
    {
        var configured = OptionValue(args, "--root");
        if (!string.IsNullOrWhiteSpace(configured))
        {
            return Path.GetFullPath(Environment.ExpandEnvironmentVariables(configured));
        }

        return UserSettingsStore.Load(context.FileSystem).Root ?? AppPaths.DefaultRoot;
    }

    public static string? OptionValue(string[] args, string name)
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
