namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static Command Db(CommandContext context)
    {
        var command = Command("db", "Explore SQL Server en lecture seule.");
        AddOptions(command,
            ProjectOption(context, "Projet dw."),
            DatabaseOption(context, "Base de donnees cible."),
            Value(OptionNames.Env, "Alias legacy de --database."));
        var sqlArg = Remaining("sql", "Requete SQL SELECT.");
        sqlArg.CompletionSources.Add(completion => SqlQueryCompletions(context, completion));
        AddSubcommands(command,
            Subcommand("schema", "Liste les tables disponibles.", parse => DbCommand.Schema(context, parse.GetValue<string>(OptionNames.Project), parse.GetValue<string>(OptionNames.Database), parse.GetValue<string>(OptionNames.Env))),
            Subcommand("describe", "Decrit une table.", parse => DbCommand.Describe(context, parse.GetValue<string>(OptionNames.Project), parse.GetValue<string>(OptionNames.Database), parse.GetValue<string>(OptionNames.Env), parse.GetRequiredValue<string>("table")), WithCompletions(Argument<string>("table", "Nom de table, avec schema optionnel."), completion => TableCompletions(context, completion))),
            Subcommand("query", "Execute une requete SELECT.", (parse, command) =>
            {
                var maxRows = parse.GetValue<int?>(OptionNames.MaxRows);
                if (maxRows is <= 0)
                {
                    throw new DwException("--max-rows doit etre superieur a 0.", 2);
                }

                return DbCommand.Query(context, parse.GetValue<string>(OptionNames.Project), parse.GetValue<string>(OptionNames.Database), parse.GetValue<string>(OptionNames.Env), maxRows, parse.GetRequiredValue<string[]>("sql"));
            }, [OptionalInt(OptionNames.MaxRows, "Nombre maximum de lignes a afficher.")], sqlArg));
        return command;
    }
}
