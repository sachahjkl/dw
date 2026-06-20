namespace Dw.Cli;

internal static class HelpCommand
{
    public static void WriteRootHelp(TextWriter writer)
    {
        writer.WriteLine($"dw - Dev Workflow {AppVersion.InformationalVersion()}");
        writer.WriteLine();
        writer.WriteLine("Usage:");
        writer.WriteLine("  dw <commande> [options]");
        writer.WriteLine();
        writer.WriteLine("Commandes:");
        var width = CliCatalog.Commands.Max(command => command.Usage.Length) + 2;
        foreach (var command in CliCatalog.Commands)
        {
            writer.WriteLine($"  {command.Usage.PadRight(width)}{command.Description}");
        }
    }
}
