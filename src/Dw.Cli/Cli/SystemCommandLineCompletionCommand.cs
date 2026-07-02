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
        var format = Value(OptionNames.Format, "Format de sortie: text ou json.", ["text", "json"]);
        var emptyToken = Flag(OptionNames.EmptyToken, "Indique que le curseur est sur un nouveau token vide.");
        command.Add(format);
        command.Add(emptyToken);
        command.Add(Remaining("line", "Ligne de commande partielle, par exemple: task --"));
        command.SetAction(parse => CompletionSuggest(context, parse.GetRequiredValue<string[]>("line"), parse.GetValue<string>(OptionNames.Format) ?? "text", parse.GetValue<bool>(OptionNames.EmptyToken)));
        return command;
    }

    private static int CompletionShow(CommandContext context)
    {
        context.Out.WriteLine("Installer l'integration shell dw:");
        context.Out.WriteLine("PowerShell - session courante:");
        context.Out.WriteLine("  dw completion install powershell | Invoke-Expression");
        context.Out.WriteLine("PowerShell - installation persistante:");
        context.Out.WriteLine("  dw completion install powershell >> $PROFILE");
        context.Out.WriteLine("  . $PROFILE");
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

    private static int CompletionSuggest(CommandContext context, IReadOnlyList<string> line, string format, bool emptyToken)
    {
        var commandLine = string.Join(' ', line);
        if (emptyToken && !commandLine.EndsWith(' '))
        {
            commandLine += " ";
        }

        var parseLine = ParseableCommandLine(commandLine, line, emptyToken);
        var completions = SortCompletions(FilterCompletionOptions(AddDynamicCompletions(context, line, parseLine, emptyToken, GetCompletionsForTesting(context, parseLine)), line, emptyToken), CurrentToken(line, emptyToken));
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

    private static IReadOnlyList<CompletionItem> SortCompletions(IReadOnlyList<CompletionItem> completions, string currentToken)
    {
        var filtered = currentToken.StartsWith("-", StringComparison.Ordinal)
            ? completions.Where(IsOption)
            : completions;

        return filtered
            .OrderBy(completion => IsOption(completion) ? 1 : 0)
            .ThenBy(completion => completion.Label, StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }

    private static IReadOnlyList<CompletionItem> FilterCompletionOptions(IReadOnlyList<CompletionItem> completions, IReadOnlyList<string> line, bool emptyToken)
    {
        var currentToken = CurrentToken(line, emptyToken);
        var specifiedOptions = line
            .Where(token => token.StartsWith("-", StringComparison.Ordinal))
            .Where(token => !string.Equals(token, currentToken, StringComparison.Ordinal))
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        var excludedOptions = MutuallyExcludedOptions(line, specifiedOptions);

        return completions
            .Where(completion => !IsOption(completion)
                || (!specifiedOptions.Contains(completion.Label) && !excludedOptions.Contains(completion.Label)))
            .ToArray();
    }

    private static IReadOnlyList<CompletionItem> AddDynamicCompletions(CommandContext context, IReadOnlyList<string> line, string commandLine, bool emptyToken, IReadOnlyList<CompletionItem> completions)
    {
        var root = BuildRoot(context);
        var parse = root.Parse(commandLine, new ParserConfiguration { EnablePosixBundling = false });
        var dynamicCompletions = ResolveDynamicCompletions(context, line, parse, CurrentToken(line, emptyToken), emptyToken);
        return completions
            .Concat(dynamicCompletions)
            .GroupBy(completion => completion.Label, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.First())
            .ToArray();
    }

    private static string ParseableCommandLine(string commandLine, IReadOnlyList<string> line, bool emptyToken)
    {
        if (!emptyToken || line.Count == 0)
        {
            return commandLine;
        }

        var previous = line[^1];
        return previous.StartsWith("--", StringComparison.Ordinal) && OptionExpectsValue(previous)
            ? commandLine + "__dw_completion__"
            : commandLine;
    }

    private static bool OptionExpectsValue(string option)
        => option is OptionNames.Project
            or OptionNames.Workspace
            or OptionNames.Repo
            or OptionNames.Database
            or OptionNames.WorkItem
            or OptionNames.Root
            or OptionNames.Profile
            or OptionNames.Task
            or OptionNames.Type
            or OptionNames.Only
            or OptionNames.Slug
            or OptionNames.Message
            or OptionNames.Env
            or OptionNames.Comments
            or OptionNames.GitTo
            or OptionNames.Value
            or OptionNames.FromEnv
            or OptionNames.Rid
            or OptionNames.Title
            or OptionNames.Format;

    private static IEnumerable<CompletionItem> ResolveDynamicCompletions(CommandContext context, IReadOnlyList<string> line, ParseResult parse, string currentToken, bool emptyToken)
    {
        var valueOption = ExpectedValueOption(line, currentToken, emptyToken);
        var filters = Filters(parse, currentToken);
        if (string.Equals(valueOption, OptionNames.Project, StringComparison.OrdinalIgnoreCase))
        {
            return ProjectCompletions(context);
        }

        if (string.Equals(valueOption, OptionNames.Workspace, StringComparison.OrdinalIgnoreCase))
        {
            return WorkspaceCompletions(context, parse);
        }

        if (string.Equals(valueOption, OptionNames.Repo, StringComparison.OrdinalIgnoreCase))
        {
            return RepositoryCompletions(context, parse);
        }

        if (string.Equals(valueOption, OptionNames.Only, StringComparison.OrdinalIgnoreCase))
        {
            return HasCommandPath(line, "task", "repo-latest")
                ? CompleteRepositorySelection(context, filters, workspaceScoped: true)
                : CompleteRepositorySelection(context, filters, workspaceScoped: false);
        }

        if (string.Equals(valueOption, OptionNames.Database, StringComparison.OrdinalIgnoreCase))
        {
            return DatabaseCompletions(context, parse);
        }

        if (string.Equals(valueOption, OptionNames.WorkItem, StringComparison.OrdinalIgnoreCase))
        {
            return CompleteWorkItems(context, filters);
        }

        if (HasCommandPath(line, "task", "start") || HasCommandPath(line, "task", "open") || HasCommandPath(line, "task", "add-work-item") || HasCommandPath(line, "task", "remove-work-item") || HasCommandPath(line, "ado", "work-item") || HasCommandPath(line, "ado", "context"))
        {
            return CompleteWorkItems(context, filters);
        }

        if (HasCommandPath(line, "db", "describe"))
        {
            return TableCompletions(context, parse);
        }

        if (HasCommandPath(line, "db", "query"))
        {
            return SqlQueryCompletions(context, parse, currentToken);
        }

        return [];
    }

    private static string? ExpectedValueOption(IReadOnlyList<string> line, string currentToken, bool emptyToken)
    {
        if (line.Count == 0)
        {
            return null;
        }

        if (emptyToken)
        {
            var previous = line[^1];
            return previous.StartsWith("--", StringComparison.Ordinal) ? previous : null;
        }

        if (currentToken.StartsWith("-", StringComparison.Ordinal) || line.Count < 2)
        {
            return null;
        }

        var candidate = line[^2];
        return candidate.StartsWith("--", StringComparison.Ordinal) ? candidate : null;
    }

    private static HashSet<string> MutuallyExcludedOptions(IReadOnlyList<string> line, HashSet<string> specifiedOptions)
    {
        var excluded = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        AddExclusions(excluded, specifiedOptions, OptionNames.FromPr, OptionNames.FromGit, line, "ado", "changelog");
        AddExclusions(excluded, specifiedOptions, OptionNames.Table, OptionNames.IdsOnly, line, "ado", "changelog");
        AddExclusions(excluded, specifiedOptions, OptionNames.CreatePr, OptionNames.SkipAdo, line, "task", "finish");
        AddExclusions(excluded, specifiedOptions, OptionNames.Database, OptionNames.Env, line, "db", "schema");
        AddExclusions(excluded, specifiedOptions, OptionNames.Database, OptionNames.Env, line, "db", "describe");
        AddExclusions(excluded, specifiedOptions, OptionNames.Database, OptionNames.Env, line, "db", "query");
        AddExclusions(excluded, specifiedOptions, OptionNames.Value, OptionNames.FromEnv, line, "secret", "set");
        AddExclusions(excluded, specifiedOptions, OptionNames.Check, OptionNames.Rid, line, "upgrade");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "open");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "open");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "open");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "sync");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "sync");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "sync");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "preflight");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "preflight");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "preflight");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "handoff-validate");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "handoff-validate");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "handoff-validate");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "rename");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "rename");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "rename");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "teardown");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "teardown");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "teardown");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "commit");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "finish");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "add-work-item");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "add-work-item");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "add-work-item");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "create-child-task");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "create-child-task");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "create-child-task");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "repo-latest");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "task", "remove-work-item");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "task", "remove-work-item");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "task", "remove-work-item");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Project, line, "agent", "open");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.WorkItem, line, "agent", "open");
        AddExclusions(excluded, specifiedOptions, OptionNames.Workspace, OptionNames.Continue, line, "agent", "open");
        AddConditionalExclusions(excluded, specifiedOptions, line);
        return excluded;
    }

    private static void AddConditionalExclusions(HashSet<string> excluded, HashSet<string> specifiedOptions, IReadOnlyList<string> line)
    {
        if (HasCommandPath(line, "ado", "changelog"))
        {
            if (!specifiedOptions.Contains(OptionNames.FromGit))
            {
                excluded.Add(OptionNames.GitTo);
            }

            var format = OptionValue(line, OptionNames.Format);
            if (!string.IsNullOrWhiteSpace(format) && !string.Equals(format, "markdown", StringComparison.OrdinalIgnoreCase))
            {
                excluded.Add(OptionNames.Table);
            }
        }

        if (HasCommandPath(line, "task", "finish") && !specifiedOptions.Contains(OptionNames.CreatePr))
        {
            excluded.Add(OptionNames.Ready);
        }
    }

    private static string? OptionValue(IReadOnlyList<string> line, string optionName)
    {
        for (var i = 0; i < line.Count - 1; i++)
        {
            if (string.Equals(line[i], optionName, StringComparison.OrdinalIgnoreCase))
            {
                var value = line[i + 1];
                if (!value.StartsWith("-", StringComparison.Ordinal))
                {
                    return value;
                }
            }
        }

        return null;
    }

    private static void AddExclusions(HashSet<string> excluded, HashSet<string> specifiedOptions, string left, string right, IReadOnlyList<string> line, params string[] commandPath)
    {
        if (!HasCommandPath(line, commandPath))
        {
            return;
        }

        if (specifiedOptions.Contains(left))
        {
            excluded.Add(right);
        }

        if (specifiedOptions.Contains(right))
        {
            excluded.Add(left);
        }
    }

    private static bool HasCommandPath(IReadOnlyList<string> line, params string[] commandPath)
    {
        if (line.Count < commandPath.Length)
        {
            return false;
        }

        for (var i = 0; i < commandPath.Length; i++)
        {
            if (!string.Equals(line[i], commandPath[i], StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }
        }

        return true;
    }

    private static bool IsOption(CompletionItem completion)
        => completion.Label.StartsWith("-", StringComparison.Ordinal)
           || completion.Label.StartsWith("/", StringComparison.Ordinal);

    private static string CurrentToken(IReadOnlyList<string> line, bool emptyToken)
        => emptyToken || line.Count == 0 ? string.Empty : line[^1];

    private static string PowerShellCompletionScript()
        => """Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock { param($wordToComplete, $commandAst, $cursorPosition) $line = $commandAst.ToString(); if ($line.StartsWith("dw ")) { $line = $line.Substring(3) }; $empty = if ([string]::IsNullOrEmpty($wordToComplete)) { "--empty-token" } else { "" }; $json = dw completion suggest --format json $empty $line 2>$null; if ([string]::IsNullOrWhiteSpace($json)) { return }; $items = $json | ConvertFrom-Json; foreach ($item in $items) { if ($item.label -like "$wordToComplete*") { [System.Management.Automation.CompletionResult]::new($item.insertText, $item.label, 'ParameterValue', $item.description) } } }""";

    private static string BashCompletionScript()
        => """
_dw_completion() {
    local cur line json labels
    cur="${COMP_WORDS[COMP_CWORD]}"
    line="${COMP_WORDS[*]:1}"
    local empty=""
    if [[ -z "$cur" ]]; then empty="--empty-token"; fi
    json=$(dw completion suggest --format json $empty $line 2>/dev/null) || return 0
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
  local empty=""
  if [[ -z "$PREFIX" ]]; then empty="--empty-token"; fi
  labels=(${(f)$(dw completion suggest --format json $empty $line 2>/dev/null | sed -n 's/.*"label":"\([^"]*\)".*/\1/p')})
  compadd -- $labels
}
_dw "$@"
""";

    private static string FishCompletionScript()
        => """
function __dw_complete
    set -l line (commandline -opc)[2..-1]
    set -l empty
    if test -z (commandline -ct)
        set empty --empty-token
    end
    dw completion suggest --format json $empty $line 2>/dev/null | string replace -ra '.*"label":"([^"]*)".*"description":"([^"]*)".*' '$1	$2'
end
complete -c dw -f -a '(__dw_complete)'
""";
}
