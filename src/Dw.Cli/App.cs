namespace Dw.Cli;

using System.Text;

internal static class App
{
    public static async Task<int> RunAsync(string[] args)
    {
        CultureInfo.CurrentUICulture = CultureInfo.GetCultureInfo("fr-FR");
        CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("fr-FR");
        Console.InputEncoding = Encoding.UTF8;
        Console.OutputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);
        var verbose = args.Any(static arg => arg == "-vvv");
        var fileSystem = new RealFileSystem();
        var settings = UserSettingsStore.Load(fileSystem);
        var output = TerminalOutput.CreateStyledWriter(Console.Out, isError: false, settings.Color);
        var error = TerminalOutput.CreateStyledWriter(Console.Error, isError: true, settings.Color);
        var context = new CommandContext(
            output,
            error,
            new SystemClock(),
            fileSystem,
            new ProcessRunner(),
            verbose);

        return await SystemCommandLineApp.RunAsync(args, context);
    }
}
