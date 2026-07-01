namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static void ValidateCommandInput(string parentName, string commandName, ParseResult parse)
    {
        switch (parentName, commandName)
        {
            case ("ado", "changelog"):
                EnsureMutuallyExclusive(parse, OptionNames.FromPr, OptionNames.FromGit);
                EnsureMutuallyExclusive(parse, OptionNames.Table, OptionNames.IdsOnly);
                EnsureOptionRequires(parse, OptionNames.GitTo, OptionNames.FromGit);
                EnsureOptionValue(parse, OptionNames.Table, OptionNames.Format, "markdown", "L'option --table est uniquement disponible avec --format markdown.");
                break;
            case ("task", "open"):
                EnsureWorkspaceSelectionIsUnambiguous(parse, positionalArgumentName: "work-item-id");
                break;
            case ("task", "sync"):
            case ("task", "rename"):
            case ("task", "teardown"):
            case ("task", "add-work-item"):
            case ("task", "remove-work-item"):
                EnsureWorkspaceSelectionIsUnambiguous(parse);
                break;
            case ("task", "commit"):
                EnsureMutuallyExclusive(parse, OptionNames.Workspace, OptionNames.Continue);
                break;
            case ("task", "finish"):
                EnsureMutuallyExclusive(parse, OptionNames.Workspace, OptionNames.Continue);
                EnsureMutuallyExclusive(parse, OptionNames.CreatePr, OptionNames.SkipAdo);
                EnsureOptionRequires(parse, OptionNames.Ready, OptionNames.CreatePr);
                break;
            case ("agent", "open"):
                EnsureWorkspaceSelectionIsUnambiguous(parse, positionalArgumentName: "work-item-id");
                break;
            case ("db", "schema"):
            case ("db", "describe"):
            case ("db", "query"):
                EnsureMutuallyExclusive(parse, OptionNames.Database, OptionNames.Env);
                break;
            case ("secret", "set"):
                EnsureMutuallyExclusive(parse, OptionNames.Value, OptionNames.FromEnv);
                break;
        }
    }

    private static void EnsureWorkspaceSelectionIsUnambiguous(ParseResult parse, string? positionalArgumentName = null)
    {
        EnsureMutuallyExclusive(parse, OptionNames.Workspace, OptionNames.Continue);
        EnsureMutuallyExclusive(parse, OptionNames.Workspace, OptionNames.Project);
        EnsureMutuallyExclusive(parse, OptionNames.Workspace, OptionNames.WorkItem);
        if (!string.IsNullOrWhiteSpace(positionalArgumentName) && HasValue(parse, OptionNames.Workspace) && !string.IsNullOrWhiteSpace(parse.GetValue<string>(positionalArgumentName)))
        {
            throw new DwException($"{OptionNames.Workspace} ne peut pas etre combine avec <{positionalArgumentName}>.", 2);
        }
    }

    private static void EnsureMutuallyExclusive(ParseResult parse, string left, string right)
    {
        if (HasValue(parse, left) && HasValue(parse, right))
        {
            throw new DwException($"{left} ne peut pas etre combine avec {right}.", 2);
        }
    }

    private static void EnsureOptionRequires(ParseResult parse, string option, string requiredOption)
    {
        if (HasValue(parse, option) && !HasValue(parse, requiredOption))
        {
            throw new DwException($"{option} requiert {requiredOption}.", 2);
        }
    }

    private static void EnsureOptionValue(ParseResult parse, string option, string valueOption, string expected, string message)
    {
        if (HasValue(parse, option) && !string.Equals(parse.GetValue<string>(valueOption), expected, StringComparison.OrdinalIgnoreCase))
        {
            throw new DwException(message, 2);
        }
    }

    private static bool HasValue(ParseResult parse, string optionName)
        => parse.Tokens.Any(token => string.Equals(token.Value, optionName, StringComparison.OrdinalIgnoreCase));
}
