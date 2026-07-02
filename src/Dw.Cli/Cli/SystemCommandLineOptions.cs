namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static void AddOptions(Command command, params Option[] options)
    {
        foreach (var option in options)
        {
            command.Add(option);
        }
    }

    private static Option<bool> Flag(string name, string description)
        => new(name)
        {
            Arity = ArgumentArity.Zero,
            Description = description
        };

    private static Option<string> Value(string name, string description, string[]? completions = null)
    {
        var option = new Option<string>(name)
        {
            Arity = ArgumentArity.ExactlyOne,
            Description = description
        };
        if (completions is { Length: > 0 })
        {
            option.AcceptOnlyFromAmong(completions);
        }

        return option;
    }

    private static Option<int?> OptionalInt(string name, string description)
        => new(name)
        {
            Arity = ArgumentArity.ExactlyOne,
            Description = description
        };

    private static Option<string> AgentOption()
        => Value(OptionNames.Agent, "Agent a utiliser.", ["opencode", "cursor", "claude", "codex-cli", "codex", "copilot"]);

    private static Option<string> ProjectOption(CommandContext context, string description)
        => Value(OptionNames.Project, description);

    private static Option<string> WorkspaceOption(CommandContext context, string description)
        => Value(OptionNames.Workspace, description);

    private static Option<string> WorkItemOption(CommandContext context, string description)
        => Value(OptionNames.WorkItem, description);

    private static Option<string> RepoOption(CommandContext context, string description)
        => Value(OptionNames.Repo, description);

    private static Option<string> DatabaseOption(CommandContext context, string description)
        => Value(OptionNames.Database, description);

    private static Option<string> WithCompletions(Option<string> option, Func<CompletionContext, IEnumerable<CompletionItem>> completions)
    {
        option.CompletionSources.Add(completions);
        return option;
    }

    private static Argument<T> WithCompletions<T>(Argument<T> argument, Func<CompletionContext, IEnumerable<CompletionItem>> completions)
    {
        argument.CompletionSources.Add(completions);
        return argument;
    }

    private static Argument<T> Argument<T>(string name, string description)
        => new(name) { Description = description };

    private static Argument<string[]> Remaining(string name, string description)
        => new(name)
        {
            Arity = ArgumentArity.OneOrMore,
            CaptureRemainingTokens = true,
            Description = description
        };
}
