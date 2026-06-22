namespace Dw.Cli.Commands;

internal static class CommandOptions
{
    public static readonly string[] NoOptionsWithValue = [];

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

    public static bool HasFlag(string[] args, string name)
        => args.Any(arg => string.Equals(arg, name, StringComparison.OrdinalIgnoreCase));

    public static int IntValue(string[] args, string name, int defaultValue, int minValue = int.MinValue)
        => int.TryParse(OptionValue(args, name), out var value)
            ? Math.Max(minValue, value)
            : defaultValue;

    public static string? FirstPositional(string[] args)
        => FirstPositional(args, NoOptionsWithValue);

    public static string? FirstPositional(string[] args, IReadOnlyCollection<string> optionsWithValue)
    {
        for (var i = 0; i < args.Length; i++)
        {
            if (optionsWithValue.Contains(args[i], StringComparer.OrdinalIgnoreCase))
            {
                i++;
                continue;
            }

            if (!args[i].StartsWith("-", StringComparison.Ordinal))
            {
                return args[i];
            }
        }

        return null;
    }

    public static string[] SubcommandArgs(string[] args)
        => args.Skip(1).ToArray();
}
