using System.CommandLine;
using System.CommandLine.Completions;

namespace Dw.Cli.Cli;

internal static partial class SystemCommandLineApp
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

    internal static IReadOnlyList<CompletionItem> GetCompletionsForTesting(CommandContext context, string commandLine)
    {
        var root = BuildRoot(context);
        return root.Parse(commandLine, new ParserConfiguration { EnablePosixBundling = false }).GetCompletions().ToArray();
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
        root.Add(Upgrade(context));

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
        command.SetAction(parse => InitCommand.Run(context, new InitRequest(
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
            ProjectOption(context, "Filtre projet dw."),
            WorkItemOption(context, "Filtre work item ADO."),
            Flag("--continue", "Reprend la derniere session/workspace."),
            AgentOption(),
            RepoOption(context, "Repo cible dans le workspace."));
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
            ProjectOption(context, "Projet dw pour resoudre Azure DevOps."),
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

    private static Command Task(CommandContext context)
    {
        var command = Command("task", "Gere les workspaces, worktrees, commits, push et PR.");
        AddOptions(command,
            ProjectOption(context, "Projet dw."),
            Value("--task", "ID de tache ADO concrete."),
            Value("--slug", "Texte source du slug."),
            Value("--type", "Type de branche."),
            Value("--only", "Repos a creer, separes par virgule."),
            WorkspaceOption(context, "Chemin explicite du workspace."),
            WorkItemOption(context, "Filtre work item ADO."),
            RepoOption(context, "Repo cible dans le workspace."),
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
            Subcommand("start", "Cree un workspace et des worktrees.", parse => TaskStartService.Start(context, new TaskStartRequest(
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
            Subcommand("finish", "Dry-run ou commit/push/PR.", parse => TaskCommand.Finish(context, new TaskFinishRequest(
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
            ProjectOption(context, "Projet dw."),
            DatabaseOption(context, "Base de donnees cible."),
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

    private static Command Upgrade(CommandContext context)
    {
        var command = Command("upgrade", "Met a jour le binaire dw depuis la derniere release configuree.");
        AddOptions(command,
            Flag("--check", "Verifie la derniere release sans modifier le binaire."),
            Value("--rid", "Runtime identifier cible."));
        command.SetAction(parse => parse.GetValue<bool>("--check")
            ? UpgradeCommand.Check(context)
            : UpgradeCommand.Run(context, parse.GetValue<string>("--rid")));
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

}
