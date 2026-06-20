namespace Dw.Cli.Commands;

internal static class AgentCommand
{
    public static int Run(CommandContext context, string[] args)
    {
        if (args.Length == 0 || args[0] is "-h" or "--help")
        {
            context.Out.WriteLine("Usage: dw agent context");
            return 0;
        }

        return args[0].ToLowerInvariant() switch
        {
            "context" => WriteContext(context),
            _ => throw new DwException($"Sous-commande agent inconnue: {args[0]}", 2)
        };
    }

    private static int WriteContext(CommandContext context)
    {
        var settings = UserSettingsStore.Load(context.FileSystem);
        var root = settings.Root ?? AppPaths.DefaultRoot;
        context.Out.WriteLine(Templates.AgentContext(root));
        return 0;
    }
}
