namespace Dw.Cli;

internal static class VersionCommand
{
    public static int Run(CommandContext context)
    {
        context.Out.WriteLine($"dw {AppVersion.InformationalVersion()}");
        context.Out.WriteLine($".NET {Environment.Version}");
        context.Out.WriteLine($"{Environment.OSVersion.Platform} {Environment.OSVersion.Version}");
        return 0;
    }
}
