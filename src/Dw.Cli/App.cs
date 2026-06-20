namespace Dw.Cli;

internal static class App
{
    public static async Task<int> RunAsync(string[] args)
    {
        var context = new CommandContext(
            Console.Out,
            Console.Error,
            new SystemClock(),
            new RealFileSystem(),
            new ProcessRunner());

        if (args.Length == 0 || IsHelp(args[0]))
        {
            HelpCommand.WriteRootHelp(context.Out);
            return 0;
        }

        try
        {
            if (CliCatalog.Dispatch.TryGetValue(args[0], out var command))
            {
                return await command.Handler(context, args.Skip(1).ToArray());
            }

            return Unknown(context, args[0]);
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

    private static int Unknown(CommandContext context, string command)
    {
        context.Error.WriteLine($"Commande inconnue: {command}");
        context.Error.WriteLine();
        HelpCommand.WriteRootHelp(context.Error);
        return 2;
    }
}
