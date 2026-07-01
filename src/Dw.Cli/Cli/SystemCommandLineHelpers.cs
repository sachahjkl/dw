namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static Command Leaf(string name, string description, CommandContext context, Func<CommandContext, int> handler, string[]? aliases = null)
    {
        var command = new Command(name, description);
        foreach (var alias in aliases ?? [])
        {
            command.Aliases.Add(alias);
        }

        command.SetAction(_ => handler(context));
        return command;
    }

    private static Command Command(string name, string description)
        => new(name, description);

    private static void AddSubcommands(Command parent, params SubcommandSpec[] subcommands)
    {
        foreach (var spec in subcommands)
        {
            var command = Command(spec.Name, spec.Description);
            foreach (var argument in spec.Arguments)
            {
                command.Add(argument);
            }

            foreach (var option in spec.Options)
            {
                command.Add(option);
            }

            command.SetAction(parse =>
            {
                ValidateCommandInput(parent.Name, spec.Name, parse);
                return spec.Handler(parse, command);
            });
            parent.Add(command);
        }
    }

    private static SubcommandSpec Subcommand(string name, string description, Func<ParseResult, Command, int> handler, params Argument[] arguments)
        => new(name, description, handler, arguments);

    private static SubcommandSpec Subcommand(string name, string description, Func<ParseResult, int> handler, params Argument[] arguments)
        => new(name, description, (parse, _) => handler(parse), arguments);

    private static SubcommandSpec Subcommand(string name, string description, Func<ParseResult, Command, int> handler, Option[] options, params Argument[] arguments)
        => new(name, description, handler, arguments, options);

    private static SubcommandSpec Subcommand(string name, string description, Func<ParseResult, int> handler, Option[] options, params Argument[] arguments)
        => new(name, description, (parse, _) => handler(parse), arguments, options);

    private sealed record SubcommandSpec(string Name, string Description, Func<ParseResult, Command, int> Handler, Argument[] Arguments, Option[]? LocalOptions = null)
    {
        public Option[] Options { get; } = LocalOptions ?? [];
    }
}
