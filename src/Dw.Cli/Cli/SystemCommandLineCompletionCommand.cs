namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static Command Completion(CommandContext context)
    {
        var command = Command("completion", "Configure l'autocompletion native System.CommandLine.");
        AddSubcommands(command,
            Subcommand("show", "Affiche les commandes d'installation de l'autocompletion.", _ => CompletionShow(context)),
            Subcommand("install", "Affiche la commande d'installation pour un shell donne.", parse => CompletionInstall(context, parse.GetRequiredValue<string>("shell")), Argument<string>("shell", "Shell cible: powershell, bash, zsh ou fish.")),
            Subcommand("suggest", "Affiche les completions avec descriptions pour une ligne donnee.", parse => CompletionSuggest(context, parse.GetRequiredValue<string[]>("line")), Remaining("line", "Ligne de commande partielle, par exemple: task --")));
        return command;
    }

    private static int CompletionShow(CommandContext context)
    {
        context.Out.WriteLine("dw utilise la directive native [suggest] de System.CommandLine.");
        context.Out.WriteLine("Installer dotnet-suggest puis charger le script de ton shell:");
        context.Out.WriteLine("  dotnet tool install -g dotnet-suggest");
        context.Out.WriteLine("  dw completion install powershell");
        context.Out.WriteLine("Tester les suggestions:");
        context.Out.WriteLine("  dw [suggest] \"task --\"");
        context.Out.WriteLine("Fallback lisible sans dotnet-suggest:");
        context.Out.WriteLine("  dw completion suggest task --");
        return 0;
    }

    private static int CompletionInstall(CommandContext context, string shell)
    {
        var command = shell.ToLowerInvariant() switch
        {
            "powershell" or "pwsh" => "dotnet-suggest script powershell | Invoke-Expression",
            "bash" => "eval \"$(dotnet-suggest script bash)\"",
            "zsh" => "eval \"$(dotnet-suggest script zsh)\"",
            "fish" => "dotnet-suggest script fish | source",
            _ => throw new DwException($"Shell inconnu: {shell}", 2)
        };

        context.Out.WriteLine(command);
        return 0;
    }

    private static int CompletionSuggest(CommandContext context, IReadOnlyList<string> line)
    {
        var completions = GetCompletionsForTesting(context, string.Join(' ', line));
        if (completions.Count == 0)
        {
            return 0;
        }

        var width = completions.Max(completion => completion.Label.Length) + 2;
        foreach (var completion in completions)
        {
            var description = completion.Documentation ?? completion.Detail ?? string.Empty;
            context.Out.WriteLine($"{completion.Label.PadRight(width)}{description}".TrimEnd());
        }

        return 0;
    }
}
