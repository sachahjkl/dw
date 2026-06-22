namespace Dw.Cli;

internal static class App
{
    public static async Task<int> RunAsync(string[] args)
    {
        CultureInfo.CurrentUICulture = CultureInfo.GetCultureInfo("fr-FR");
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("fr-FR");
        var verbose = args.Any(static arg => arg == "-vvv");
        var context = new CommandContext(
            Console.Out,
            Console.Error,
            new SystemClock(),
            new RealFileSystem(),
            new ProcessRunner(),
            verbose);

        return await SystemCommandLineApp.RunAsync(args, context);
    }
}
