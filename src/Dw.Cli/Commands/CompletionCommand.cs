namespace Dw.Cli.Commands;

internal static class CompletionCommand
{
    private static IReadOnlyDictionary<string, string[]> Commands => CliCatalog.CompletionMap;
    private static IReadOnlyDictionary<string, IReadOnlyDictionary<string, string>> Descriptions => CliCatalog.CompletionDescriptionMap;

    public static int Run(CommandContext context, string[] args)
    {
        var shell = args.FirstOrDefault()?.ToLowerInvariant() ?? "powershell";
        return shell switch
        {
            "powershell" or "pwsh" => PowerShell(context),
            "bash" => Bash(context),
            "zsh" => Zsh(context),
            "fish" => Fish(context),
            "nushell" or "nu" => Nushell(context),
            _ => Help(context)
        };
    }

    private static int Help(CommandContext context)
    {
        context.Out.WriteLine("Usage: dw completion <powershell|bash|zsh|fish|nushell>");
        return 0;
    }

    private static int PowerShell(CommandContext context)
    {
        context.Out.WriteLine($$"""
Register-ArgumentCompleter -Native -CommandName dw -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commands = @{
{{PowerShellEntries()}}
    }
    $descriptions = @{
{{PowerShellDescriptionEntries()}}
    }

    $tokens = @($commandAst.CommandElements | ForEach-Object { $_.ToString() })
    $rootCommand = ""
    if ($tokens.Count -gt 1) {
        $rootCommand = $tokens[1]
    }

    $candidates = if ($commands.ContainsKey($rootCommand)) {
        $commands[$rootCommand]
    } else {
        $commands[""]
    }

    $descriptionMap = if ($descriptions.ContainsKey($rootCommand)) {
        $descriptions[$rootCommand]
    } else {
        $descriptions[""]
    }

    $candidates |
        Where-Object { $_ -like "$wordToComplete*" } |
        ForEach-Object {
            $description = if ($descriptionMap.ContainsKey($_)) { $descriptionMap[$_] } else { $_ }
            [System.Management.Automation.CompletionResult]::new($_, $_, "ParameterValue", $description)
        }
}
""");
        return 0;
    }

    private static int Bash(CommandContext context)
    {
        context.Out.WriteLine($$"""
_dw_completion()
{
    local cur root candidates
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    root=""
    if [[ ${#COMP_WORDS[@]} -gt 1 ]]; then
        root="${COMP_WORDS[1]}"
    fi

    case "$root" in
{{CaseEntries("bash")}}
        *) candidates="{{Join(Commands[""])}}" ;;
    esac

    COMPREPLY=( $(compgen -W "$candidates" -- "$cur") )
    return 0
}
complete -F _dw_completion dw
""");
        return 0;
    }

    private static int Zsh(CommandContext context)
    {
        context.Out.WriteLine($$"""
#compdef dw

_dw() {
  local -a candidates
  local root="${words[2]}"

  case "$root" in
{{CaseEntries("zsh")}}
    *) candidates=({{ZshArray(Commands[""])}}) ;;
  esac

  compadd -- $candidates
}

_dw "$@"
""");
        return 0;
    }

    private static int Fish(CommandContext context)
    {
        context.Out.WriteLine("""
complete -c dw -f
""");
        foreach (var command in Commands[""])
        {
            var description = EscapeFish(Descriptions[""].GetValueOrDefault(command, command));
            context.Out.WriteLine($"complete -c dw -n '__fish_use_subcommand' -a '{command}' -d '{description}'");
        }

        foreach (var (root, candidates) in Commands.Where(pair => pair.Key.Length > 0))
        {
            foreach (var candidate in candidates)
            {
                var option = candidate.StartsWith("--", StringComparison.Ordinal)
                    ? $"-l {candidate[2..]}"
                    : $"-a '{candidate}'";
                var description = EscapeFish(Descriptions.GetValueOrDefault(root)?.GetValueOrDefault(candidate, candidate) ?? candidate);
                context.Out.WriteLine($"complete -c dw -n '__fish_seen_subcommand_from {root}' {option} -d '{description}'");
            }
        }

        return 0;
    }

    private static int Nushell(CommandContext context)
    {
        context.Out.WriteLine($$"""
def "nu-complete dw" [context: string] {
  let words = ($context | split row " ")
  let root = if (($words | length) > 1) { $words | get 1 } else { "" }
  match $root {
{{NuEntries()}}
    _ => [{{NuRecords("", Commands[""])}}]
  }
}

export extern "dw" [
  ...args: string@"nu-complete dw"
]
""");
        return 0;
    }

    private static string PowerShellEntries()
        => string.Join(Environment.NewLine, Commands.Select(pair =>
            $"        \"{pair.Key}\" = @({string.Join(", ", pair.Value.Select(value => $"\"{value}\""))})"));

    private static string PowerShellDescriptionEntries()
        => string.Join(Environment.NewLine, Descriptions.Select(pair =>
            $"        \"{pair.Key}\" = @{{ {string.Join("; ", pair.Value.Select(value => $"\"{value.Key}\" = \"{EscapePowerShell(value.Value)}\""))} }}"));

    private static string CaseEntries(string shell)
    {
        return string.Join(Environment.NewLine, Commands
            .Where(pair => pair.Key.Length > 0)
            .Select(pair => shell == "zsh"
                ? $"    {pair.Key}) candidates=({ZshArray(pair.Value)}) ;;"
                : $"        {pair.Key}) candidates=\"{Join(pair.Value)}\" ;;"));
    }

    private static string NuEntries()
        => string.Join(Environment.NewLine, Commands
            .Where(pair => pair.Key.Length > 0)
            .Select(pair => $"    \"{pair.Key}\" => [{NuRecords(pair.Key, pair.Value)}]"));

    private static string Join(IEnumerable<string> values)
        => string.Join(' ', values);

    private static string ZshArray(IEnumerable<string> values)
        => string.Join(' ', values.Select(value => $"'{value}'"));

    private static string NuList(IEnumerable<string> values)
        => string.Join(' ', values.Select(value => $"\"{value}\""));

    private static string NuRecords(string root, IEnumerable<string> values)
    {
        var descriptions = Descriptions.GetValueOrDefault(root) ?? new Dictionary<string, string>();
        return string.Join(' ', values.Select(value => $"{{ value: \"{value}\", description: \"{EscapeNu(descriptions.GetValueOrDefault(value, value))}\" }}"));
    }

    private static string EscapePowerShell(string value)
        => value.Replace("`", "``", StringComparison.Ordinal).Replace("\"", "`\"", StringComparison.Ordinal);

    private static string EscapeFish(string value)
        => value.Replace("'", "\\'", StringComparison.Ordinal);

    private static string EscapeNu(string value)
        => value.Replace("\\", "\\\\", StringComparison.Ordinal).Replace("\"", "\\\"", StringComparison.Ordinal);
}
