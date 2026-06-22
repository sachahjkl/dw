using System.Text.Json;

namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static Command Completion(CommandContext context)
    {
        var command = Command("completion", "Configure l'autocompletion native System.CommandLine.");
        AddSubcommands(command,
            Subcommand("show", "Affiche les commandes d'installation de l'autocompletion.", _ => CompletionShow(context)),
            Subcommand("install", "Affiche la commande d'installation pour un shell donne.", parse => CompletionInstall(context, parse.GetRequiredValue<string>("shell")), Argument<string>("shell", "Shell cible: powershell, bash, zsh ou fish.")));
        command.Add(Suggest(context));
        return command;
    }

    private static Command Suggest(CommandContext context)
    {
        var command = Command("suggest", "Affiche les completions avec descriptions pour une ligne donnee.");
        var format = Value("--format", "Format de sortie: text ou json.", ["text", "json"]);
        command.Add(format);
        command.Add(Remaining("line", "Ligne de commande partielle, par exemple: task --"));
        command.SetAction(parse => CompletionSuggest(context, parse.GetRequiredValue<string[]>("line"), parse.GetValue<string>("--format") ?? "text"));
        return command;
    }

    private static int CompletionShow(CommandContext context)
    {
        context.Out.WriteLine("Installer l'integration shell dw:");
        context.Out.WriteLine("  dw completion install powershell | Invoke-Expression");
        context.Out.WriteLine("Fallback lisible:");
        context.Out.WriteLine("  dw completion suggest task --");
        context.Out.WriteLine("Directive native System.CommandLine:");
        context.Out.WriteLine("  dw [suggest] \"task --\"");
        return 0;
    }

    private static int CompletionInstall(CommandContext context, string shell)
    {
        var script = shell.ToLowerInvariant() switch
        {
            "powershell" or "pwsh" => PowerShellCompletionScript(),
            "bash" => BashCompletionScript(),
            "zsh" => ZshCompletionScript(),
            "fish" => FishCompletionScript(),
            _ => throw new DwException($"Shell inconnu: {shell}", 2)
        };

        context.Out.WriteLine(script);
        return 0;
    }

    private static int CompletionSuggest(CommandContext context, IReadOnlyList<string> line, string format)
    {
        var completions = GetCompletionsForTesting(context, string.Join(' ', line));
        if (completions.Count == 0)
        {
            return 0;
        }

        if (format.Equals("json", StringComparison.OrdinalIgnoreCase))
        {
            var payload = completions.Select(completion => new CompletionSuggestion(
                completion.Label,
                string.IsNullOrWhiteSpace(completion.InsertText) ? completion.Label : completion.InsertText,
                completion.Documentation ?? completion.Detail ?? string.Empty)).ToArray();
            context.Out.WriteLine(JsonSerializer.Serialize(payload, AppJsonContext.Default.CompletionSuggestionArray));
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

    private static string PowerShellCompletionScript()
        => """Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock { param($wordToComplete, $commandAst, $cursorPosition) $line = $commandAst.ToString(); if ($line.StartsWith("dw ")) { $line = $line.Substring(3) }; $json = dw completion suggest --format json $line 2>$null; if ([string]::IsNullOrWhiteSpace($json)) { return }; $items = $json | ConvertFrom-Json; foreach ($item in $items) { if ($item.label -like "$wordToComplete*") { [System.Management.Automation.CompletionResult]::new($item.insertText, $item.label, 'ParameterValue', $item.description) } } }""";

    private static string BashCompletionScript()
        => """
_dw_completion() {
    local cur line json labels
    cur="${COMP_WORDS[COMP_CWORD]}"
    line="${COMP_WORDS[*]:1}"
    json=$(dw completion suggest --format json $line 2>/dev/null) || return 0
    labels=$(printf '%s' "$json" | sed -n 's/.*"label":"\([^"]*\)".*/\1/p')
    COMPREPLY=( $(compgen -W "$labels" -- "$cur") )
}
complete -F _dw_completion dw
""";

    private static string ZshCompletionScript()
        => """
#compdef dw
_dw() {
  local -a labels
  local line="${words[@]:1}"
  labels=(${(f)$(dw completion suggest --format json $line 2>/dev/null | sed -n 's/.*"label":"\([^"]*\)".*/\1/p')})
  compadd -- $labels
}
_dw "$@"
""";

    private static string FishCompletionScript()
        => """
function __dw_complete
    set -l line (commandline -opc)[2..-1]
    dw completion suggest --format json $line 2>/dev/null | string replace -ra '.*"label":"([^"]*)".*"description":"([^"]*)".*' '$1	$2'
end
complete -c dw -f -a '(__dw_complete)'
""";
}
