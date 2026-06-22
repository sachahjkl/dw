namespace Dw.Cli;

internal static class App
{
    public static async Task<int> RunAsync(string[] args)
    {
        var verbose = args.Any(static arg => arg == "-vvv");
        var commandIndex = FirstCommandIndex(args);
        var context = new CommandContext(
            Console.Out,
            Console.Error,
            new SystemClock(),
            new RealFileSystem(),
            new ProcessRunner(),
            verbose);

        if (commandIndex < 0 || IsHelp(args[commandIndex]))
        {
            HelpCommand.WriteRootHelp(context.Out);
            return 0;
        }

        try
        {
            context.Debug($"Arguments: {string.Join(' ', args)}");
            if (CliCatalog.Dispatch.TryGetValue(args[commandIndex], out var command))
            {
                return await command.Handler(context, args.Skip(commandIndex + 1).ToArray());
            }

            return Unknown(context, args[commandIndex]);
        }
        catch (DwException ex)
        {
            context.Error.WriteLine($"Erreur: {ex.Message}");
            return ex.ExitCode;
        }
        catch (Exception ex)
        {
            context.Error.WriteLine("Erreur inattendue.");
            context.Error.WriteLine(ex.Message);
            return 1;
        }
    }

    private static bool IsHelp(string value)
        => value is "-h" or "--help" or "help";

    private static int FirstCommandIndex(string[] args)
    {
        for (var i = 0; i < args.Length; i++)
        {
            if (args[i] == "-vvv")
            {
                continue;
            }

            return i;
        }

        return -1;
    }

    private static int Unknown(CommandContext context, string command)
    {
        context.Error.WriteLine($"Commande inconnue: {command}");
        context.Error.WriteLine();
        HelpCommand.WriteRootHelp(context.Error);
        return 2;
    }
}
