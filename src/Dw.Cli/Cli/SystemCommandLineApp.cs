using System.CommandLine;
using System.CommandLine.Completions;
using System.CommandLine.Invocation;
using System.CommandLine.Parsing;

namespace Dw.Cli.Cli;

internal static class SystemCommandLineApp
{
    public static async Task<int> RunAsync(string[] args, CommandContext context)
    {
        try
        {
            context.Debug($"Arguments: {string.Join(' ', args)}");
            var root = BuildRoot(context);
            var parseResult = root.Parse(args, new ParserConfiguration { EnablePosixBundling = false });
            return await parseResult.InvokeAsync(new InvocationConfiguration
            {
                Output = context.Out,
                Error = context.Error,
                EnableDefaultExceptionHandler = false
            }, CancellationToken.None);
        }
        catch (DwException ex)
        {
            context.Error.WriteLine($"Erreur: {ex.Message}");
            return ex.ExitCode;
        }
        catch (Exception ex)
        {
            context.Error.WriteLine("Erreur inattendue.");
            context.Error.WriteLine(ex.Message);
            return 1;
        }
    }

    private static RootCommand BuildRoot(CommandContext context)
    {
        var root = new RootCommand($"Dev Workflow {AppVersion.InformationalVersion()}")
        {
            HelpName = "dw"
        };
        root.Add(new SuggestDirective());

        var verbose = new Option<bool>("-vvv")
        {
            Description = "Active les traces de diagnostic.",
            Recursive = true
        };
        root.Add(verbose);

        root.Add(Leaf("version", "Affiche la version du CLI.", context, VersionCommand.Run));
        root.Add(Leaf("guide", "Explique le parcours de demarrage.", context, GuideCommand.Run, aliases: ["get-started"]));
        root.Add(Doctor(context));
        root.Add(Init(context));
        root.Add(Agent(context));
        root.Add(Ado(context));
        root.Add(Auth(context));
        root.Add(Completion(context));
        root.Add(Task(context));
        root.Add(Config(context));
        root.Add(Db(context));
        root.Add(Secret(context));
        root.Add(Update(context));

        return root;
    }

    private static Command Init(CommandContext context)
    {
        var command = Command("init", "Initialise un root DevWorkflow.");
        AddOptions(command,
            Value("--profile", "Profil d'initialisation.", ["ogf"]),
            Value("--root", "Root DevWorkflow a creer."),
            Flag("--dry-run", "Simule sans ecrire."),
            Flag("--no-save", "Ne sauvegarde pas le root utilisateur."));
        command.SetAction(parse => InitCommand.Run(context, new InitCommandOptions(
            parse.GetValue<string>("--root"),
            parse.GetValue<string>("--profile"),
            parse.GetValue<bool>("--no-save"),
            parse.GetValue<bool>("--dry-run"))));
        return command;
    }

    private static Command Doctor(CommandContext context)
    {
        var command = Command("doctor", "Diagnostique l'environnement local.");
        AddOptions(command, Flag("--fix", "Corrige ce qui peut etre corrige automatiquement."));
        command.SetAction(parse => DoctorCommand.RunAsync(context, parse.GetValue<bool>("--fix")));
        return command;
    }

    private static Command Agent(CommandContext context)
    {
        var command = Command("agent", "Affiche le contexte ou ouvre un agent.");
        AddOptions(command,
            Value("--root", "Root DevWorkflow a utiliser."),
            Value("--workspace", "Chemin explicite du workspace."),
            Value("--project", "Filtre projet dw."),
            Value("--work-item", "Filtre work item ADO."),
            Flag("--continue", "Reprend la derniere session/workspace."),
            AgentOption(),
            Value("--repo", "Repo cible dans le workspace."));
        AddSubcommands(command,
            Subcommand("context", "Affiche le contexte court pour agents IA.", (_, _) => AgentCommand.WriteContext(context)),
            Subcommand("open", "Ouvre un workspace dans l'agent configure.", (parse, _) => WorkspaceOpenService.Open(context, OpenOptions(parse))),
            Subcommand("config", "Lit ou modifie la configuration agent.", (parse, _) => AgentCommand.ShowConfig(context, parse.GetValue<string>("--root"))),
            Subcommand("show", "Affiche la configuration courante.", (parse, _) => AgentCommand.ShowConfig(context, parse.GetValue<string>("--root"))),
            Subcommand("set-default", "Definit l'agent par defaut.", (parse, _) => AgentCommand.SetDefaultAgent(context, parse.GetValue<string>("--root"), parse.GetRequiredValue<string>("agent")), Argument<string>("agent", "Agent a utiliser par defaut.")),
            Subcommand("doctor", "Verifie les agents disponibles.", (parse, _) => AgentCommand.Doctor(context, parse.GetValue<string>("--agent"))));
        return command;
    }

    private static Command Ado(CommandContext context)
    {
        var command = Command("ado", "Lit Azure DevOps sans modifier.");
        AddOptions(command,
            Value("--project", "Projet dw pour resoudre Azure DevOps."),
            Value("--root", "Root DevWorkflow a utiliser."),
            Flag("--summary", "Limite la sortie au resume."),
            Value("--comments", "Nombre de commentaires a charger."),
            Value("--top", "Nombre maximum d'items."));
        AddSubcommands(command,
            Subcommand("assigned", "Liste les work items assignes a @Me.", parse => AdoCommand.Assigned(context, parse.GetValue<string>("--root"), parse.GetValue<string>("--project"), Math.Max(1, parse.GetValue<int?>("--top") ?? 20))),
            Subcommand("work-item", "Affiche un resume de work item.", parse => AdoCommand.WorkItem(context, parse.GetValue<string>("--root"), parse.GetValue<string>("--project"), parse.GetRequiredValue<string>("id")), Argument<string>("id", "ID du work item.")),
            Subcommand("context", "Affiche le contexte complet d'un work item.", parse => AdoCommand.WorkItemContext(context, parse.GetValue<string>("--root"), parse.GetValue<string>("--project"), parse.GetRequiredValue<string>("id"), parse.GetValue<bool>("--summary"), Math.Max(0, parse.GetValue<int?>("--comments") ?? 200)), Argument<string>("id", "ID du work item.")));
        return command;
    }

    private static Command Auth(CommandContext context)
    {
        var command = Command("auth", "Gere la connexion Azure DevOps.");
        AddOptions(command, Value("--root", "Root DevWorkflow a utiliser."));
        AddSubcommands(command,
            Subcommand("login", "Connecte Azure DevOps.", (parse, _) => AuthCommand.Login(context, parse.GetValue<string>("--root"))),
            Subcommand("status", "Affiche l'etat de connexion.", (parse, _) => AuthCommand.Status(context, parse.GetValue<string>("--root"))),
            Subcommand("logout", "Supprime la connexion locale.", (parse, _) => AuthCommand.Logout(context, parse.GetValue<string>("--root"))));
        return command;
    }

    private static Command Completion(CommandContext context)
    {
        var command = Command("completion", "Explique l'autocompletion native System.CommandLine.");
        command.SetAction(_ =>
        {
            context.Out.WriteLine("System.CommandLine expose les completions via la directive [suggest].");
            context.Out.WriteLine("Exemples:");
            context.Out.WriteLine("  dw [suggest] \"dw task --\"");
            context.Out.WriteLine("  dotnet-suggest script powershell | Invoke-Expression");
            return 0;
        });
        return command;
    }

    private static Command Task(CommandContext context)
    {
        var command = Command("task", "Gere les workspaces, worktrees, commits, push et PR.");
        AddOptions(command,
            Value("--project", "Projet dw."),
            Value("--task", "ID de tache ADO concrete."),
            Value("--slug", "Texte source du slug."),
            Value("--type", "Type de branche."),
            Value("--only", "Repos a creer, separes par virgule."),
            Value("--workspace", "Chemin explicite du workspace."),
            Value("--work-item", "Filtre work item ADO."),
            Value("--repo", "Repo cible dans le workspace."),
            Flag("--continue", "Utilise le workspace le plus recent."),
            Flag("--yes", "Confirme sans prompt."),
            Flag("--no-sync", "Desactive le sync ADO automatique."),
            Flag("--json", "Sortie JSON."),
            AgentOption(),
            Flag("--execute", "Execute vraiment l'action."),
            Value("--message", "Message de commit."),
            Flag("--create-pr", "Ouvre une PR apres push."),
            Flag("--ready", "Cree une PR non draft."),
            Flag("--skip-ado", "Ignore Azure DevOps."),
            Flag("--skip-verify", "Ignore les verifications configurees."),
            Flag("--create-child-tasks", "Cree les taches ADO enfant."));
        AddSubcommands(command,
            Subcommand("start", "Cree un workspace et des worktrees.", parse => TaskStartService.Start(context, new TaskStartCommandOptions(
                parse.GetRequiredValue<string>("work-item-id"),
                parse.GetValue<string>("--project"),
                parse.GetValue<string>("--task"),
                parse.GetValue<string>("--type"),
                parse.GetValue<string>("--only"),
                parse.GetValue<string>("--slug"),
                parse.GetValue<bool>("--skip-ado"),
                parse.GetValue<bool>("--create-child-tasks"))), Argument<string>("work-item-id", "ID du work item parent.")),
            Subcommand("status", "Liste les chemins des workspaces.", (_, _) => TaskListService.Status(context)),
            Subcommand("list", "Liste les workspaces avec metadonnees.", parse => TaskListService.List(context, new TaskListOptions(parse.GetValue<string>("--project"), parse.GetValue<string>("--work-item"), parse.GetValue<bool>("--json")))),
            Subcommand("current", "Affiche le workspace courant.", (_, _) => TaskListService.Current(context)),
            Subcommand("sync", "Synchronise task.json depuis ADO.", parse => TaskSyncPruneService.Sync(context, OpenOptions(parse))),
            Subcommand("prune", "Nettoie les workspaces en etat final.", parse => TaskSyncPruneService.Prune(context, new TaskPruneOptions(parse.GetValue<string>("--project"), parse.GetValue<string>("--work-item"), parse.GetValue<bool>("--execute"), parse.GetValue<bool>("--yes"), !parse.GetValue<bool>("--no-sync")))),
            Subcommand("rename", "Renomme slug, branche et dossier workspace.", parse => TaskRenameService.Rename(context, new TaskRenameOptions(parse.GetRequiredValue<string>("--slug"), OpenOptions(parse), parse.GetValue<bool>("--execute")))),
            Subcommand("open", "Ouvre le workspace dans un agent.", (parse, _) => WorkspaceOpenService.Open(context, OpenOptions(parse))),
            Subcommand("teardown", "Supprime les worktrees et le workspace.", (parse, _) => WorkspaceTeardownService.Teardown(context, TeardownOptions(parse))),
            Subcommand("add-repo", "Ajoute un repo au workspace existant.", parse => TaskCommand.AddRepo(context, new TaskAddRepoOptions(parse.GetRequiredValue<string>("repo"), parse.GetValue<string>("--workspace"))), Argument<string>("repo", "Repo a ajouter.")),
            Subcommand("finish", "Dry-run ou commit/push/PR.", parse => TaskCommand.Finish(context, new TaskFinishCommandOptions(
                parse.GetValue<string>("--workspace"),
                parse.GetValue<bool>("--execute"),
                parse.GetValue<bool>("--create-pr"),
                parse.GetValue<bool>("--ready"),
                parse.GetValue<string>("--message"),
                parse.GetValue<bool>("--skip-verify"),
                parse.GetValue<bool>("--skip-ado")))));
        return command;
    }

    private static Command Config(CommandContext context)
    {
        var command = Command("config", "Valide et modifie la configuration.");
        AddOptions(command, Value("--root", "Root a utiliser pour cette commande."));
        AddSubcommands(command,
            Subcommand("show", "Affiche le root configure.", (_, _) => ConfigCommand.Show(context)),
            Subcommand("set-root", "Definit le root DevWorkflow.", (parse, _) => ConfigCommand.SetRoot(context, parse.GetRequiredValue<string>("path")), Argument<string>("path", "Chemin du root DevWorkflow.")),
            Subcommand("doctor", "Valide les fichiers config.", (parse, _) => ConfigCommand.Doctor(context, parse.GetValue<string>("--root"))));
        return command;
    }

    private static Command Db(CommandContext context)
    {
        var command = Command("db", "Explore SQL Server en lecture seule.");
        AddOptions(command,
            Value("--project", "Projet dw."),
            Value("--database", "Base de donnees cible."),
            Value("--env", "Alias legacy de --database."));
        AddSubcommands(command,
            Subcommand("schema", "Liste les tables disponibles.", parse => DbCommand.Schema(context, parse.GetValue<string>("--project"), parse.GetValue<string>("--database"), parse.GetValue<string>("--env"))),
            Subcommand("describe", "Decrit une table.", parse => DbCommand.Describe(context, parse.GetValue<string>("--project"), parse.GetValue<string>("--database"), parse.GetValue<string>("--env"), parse.GetRequiredValue<string>("table")), Argument<string>("table", "Nom de table, avec schema optionnel.")),
            Subcommand("query", "Execute une requete SELECT.", parse => DbCommand.Query(context, parse.GetValue<string>("--project"), parse.GetValue<string>("--database"), parse.GetValue<string>("--env"), parse.GetRequiredValue<string[]>("sql")), Remaining("sql", "Requete SQL SELECT.")));
        return command;
    }

    private static Command Secret(CommandContext context)
    {
        var command = Command("secret", "Stocke des secrets locaux via Windows Credential Manager.");
        AddOptions(command,
            Value("--value", "Valeur du secret."),
            Value("--from-env", "Nom de variable d'environnement source."));
        AddSubcommands(command,
            Subcommand("set", "Cree ou remplace un secret.", (parse, _) => SecretCommand.Set(context, parse.GetRequiredValue<string>("key"), parse.GetValue<string>("--value"), parse.GetValue<string>("--from-env")), Argument<string>("key", "Cle du secret.")),
            Subcommand("get", "Lit un secret.", (parse, _) => SecretCommand.Get(context, parse.GetRequiredValue<string>("key")), Argument<string>("key", "Cle du secret.")),
            Subcommand("delete", "Supprime un secret.", (parse, _) => SecretCommand.Delete(context, parse.GetRequiredValue<string>("key")), Argument<string>("key", "Cle du secret.")));
        return command;
    }

    private static Command Update(CommandContext context)
    {
        var command = Command("update", "Verifie ou telecharge une release configuree.");
        AddOptions(command,
            Value("--output", "Dossier de telechargement."),
            Value("--rid", "Runtime identifier cible."));
        AddSubcommands(command,
            Subcommand("check", "Verifie la derniere release.", (_, _) => UpdateCommand.Check(context)),
            Subcommand("download", "Telecharge la derniere release.", (parse, _) => UpdateCommand.Download(context, parse.GetValue<string>("--rid"), parse.GetValue<string>("--output"))));
        return command;
    }

    private static WorkspaceOpenOptions OpenOptions(ParseResult parse)
        => new(
            Workspace: parse.GetValue<string>("--workspace"),
            Project: parse.GetValue<string>("--project"),
            WorkItemId: parse.GetValue<string>("--work-item"),
            Continue: parse.GetValue<bool>("--continue"),
            Agent: parse.GetValue<string>("--agent"),
            Repository: parse.GetValue<string>("--repo"));

    private static WorkspaceTeardownOptions TeardownOptions(ParseResult parse)
        => new(
            Workspace: parse.GetValue<string>("--workspace"),
            Project: parse.GetValue<string>("--project"),
            WorkItemId: parse.GetValue<string>("--work-item"),
            Continue: parse.GetValue<bool>("--continue"),
            Execute: parse.GetValue<bool>("--execute"),
            Yes: parse.GetValue<bool>("--yes"));

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
            command.SetAction(parse => spec.Handler(parse, command));
            parent.Add(command);
        }
    }

    private static SubcommandSpec Subcommand(string name, string description, Func<ParseResult, Command, int> handler, params Argument[] arguments)
        => new(name, description, handler, arguments);

    private static SubcommandSpec Subcommand(string name, string description, Func<ParseResult, int> handler, params Argument[] arguments)
        => new(name, description, (parse, _) => handler(parse), arguments);

    private static void AddOptions(Command command, params Option[] options)
    {
        foreach (var option in options)
        {
            option.Recursive = true;
            command.Add(option);
        }
    }

    private static Option<bool> Flag(string name, string description)
        => new(name) { Description = description };

    private static Option<string> Value(string name, string description, string[]? completions = null)
    {
        var option = new Option<string>(name)
        {
            Description = description
        };
        if (completions is { Length: > 0 })
        {
            option.AcceptOnlyFromAmong(completions);
        }

        return option;
    }

    private static Option<string> AgentOption()
        => Value("--agent", "Agent a utiliser.", ["opencode", "cursor", "claude", "codex-cli", "codex", "copilot"]);

    private static Argument<T> Argument<T>(string name, string description)
        => new(name) { Description = description };

    private static Argument<string[]> Remaining(string name, string description)
        => new(name)
        {
            Arity = ArgumentArity.OneOrMore,
            CaptureRemainingTokens = true,
            Description = description
        };

    private sealed record SubcommandSpec(string Name, string Description, Func<ParseResult, Command, int> Handler, Argument[] Arguments);
}
