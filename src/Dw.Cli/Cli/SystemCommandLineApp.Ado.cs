namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
{
    private static Command Ado(CommandContext context)
    {
        var command = Command("ado", "Lit Azure DevOps sans modifier.");
        AddSubcommands(command,
            Subcommand("assigned", "Liste les work items ADO assignes a @Me pour choisir le prochain sujet de travail.", parse => AdoCommand.Assigned(context, parse.GetValue<string>(OptionNames.Root), parse.GetValue<string>(OptionNames.Project), Math.Max(1, parse.GetValue<int?>(OptionNames.Top) ?? 20), parse.GetValue<bool>(OptionNames.All), parse.GetValue<bool>(OptionNames.GroupByParent), parse.GetValue<bool>(OptionNames.Json)),
                [
                    ProjectOption(context, "Projet dw pour resoudre Azure DevOps."),
                    Value(OptionNames.Root, "Root DevWorkflow a utiliser."),
                    OptionalInt(OptionNames.Top, "Nombre maximum d'items."),
                    Flag(OptionNames.All, "Inclut aussi les work items deja dans un etat final."),
                    Flag(OptionNames.GroupByParent, "Groupe les work items assignes par parent ADO."),
                    Flag(OptionNames.Json, "Sortie JSON.")
                ]),
            Subcommand("changelog", "Construit un changelog de work items a partir de PR ADO ou d'une plage git.", parse => AdoCommand.Changelog(context, parse.GetValue<string>(OptionNames.Root), parse.GetValue<string>(OptionNames.Project), parse.GetRequiredValue<string>("ids"), parse.GetValue<bool>(OptionNames.FromPr), parse.GetValue<bool>(OptionNames.FromGit), parse.GetValue<string>(OptionNames.Repo), parse.GetValue<bool>(OptionNames.GroupByParent), parse.GetValue<string>(OptionNames.Format), parse.GetValue<bool>(OptionNames.Table), parse.GetValue<bool>(OptionNames.IdsOnly), parse.GetValue<string>(OptionNames.GitTo)),
                [
                    ProjectOption(context, "Projet dw pour resoudre Azure DevOps."),
                    Value(OptionNames.Root, "Root DevWorkflow a utiliser."),
                    Flag(OptionNames.FromPr, "Interprete ids comme une liste d'IDs de pull request Azure DevOps (mode par defaut)."),
                    Flag(OptionNames.FromGit, "Interprete ids et --git-to comme deux refs git."),
                    RepoOption(context, "Repo dw a resoudre, ou nom de repo Azure DevOps. Recommande pour les PR si plusieurs repos existent."),
                    Value(OptionNames.Format, "Format de sortie.", ["raw", "markdown", "html"]),
                    Flag(OptionNames.Table, "En format markdown, rend le changelog sous forme de tableau."),
                    Flag(OptionNames.IdsOnly, "Affiche uniquement la liste des IDs de work items, separes par des espaces."),
                    Flag(OptionNames.GroupByParent, "Groupe les work items par parent ADO."),
                    Value(OptionNames.GitTo, "Ref git de fin pour --from-git.")
                ],
                Argument<string>("ids", "IDs de PR separes par virgules, ou ref git de depart pour --from-git.")),
            Subcommand("work-item", "Affiche le resume d'un ou plusieurs work items sans charger tout le contexte detaille.", parse => AdoCommand.WorkItem(context, parse.GetValue<string>(OptionNames.Root), parse.GetValue<string>(OptionNames.Project), parse.GetRequiredValue<string>("id"), parse.GetValue<bool>(OptionNames.Json)),
                [
                    ProjectOption(context, "Projet dw pour resoudre Azure DevOps."),
                    Value(OptionNames.Root, "Root DevWorkflow a utiliser."),
                    Flag(OptionNames.Json, "Sortie JSON.")
                ],
                Argument<string>("id", "ID du work item, ou liste separee par virgules.")),
            Subcommand("context", "Affiche le contexte complet d'un ou plusieurs work items: description, relations, commentaires et pieces jointes.", parse => AdoCommand.WorkItemContext(context, parse.GetValue<string>(OptionNames.Root), parse.GetValue<string>(OptionNames.Project), parse.GetRequiredValue<string>("id"), parse.GetValue<bool>(OptionNames.Summary), Math.Max(0, parse.GetValue<int?>(OptionNames.Comments) ?? 200), parse.GetValue<bool>(OptionNames.Json)),
                [
                    ProjectOption(context, "Projet dw pour resoudre Azure DevOps."),
                    Value(OptionNames.Root, "Root DevWorkflow a utiliser."),
                    Flag(OptionNames.Summary, "Limite la sortie au resume."),
                    Value(OptionNames.Comments, "Nombre de commentaires a charger."),
                    Flag(OptionNames.Json, "Sortie JSON.")
                ],
                Argument<string>("id", "ID du work item, ou liste separee par virgules.")));
        return command;
    }
}
