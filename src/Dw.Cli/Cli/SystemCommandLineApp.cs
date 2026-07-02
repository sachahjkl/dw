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
            context.Error.WriteLine($"{TerminalOutput.Bold(TerminalOutput.Red("Erreur", context.Error), context.Error)}: {ex.Message}");
            return ex.ExitCode;
        }
        catch (Exception ex)
        {
            context.Error.WriteLine(TerminalOutput.Bold(TerminalOutput.Red("Erreur inattendue.", context.Error), context.Error));
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
        root.Add(Refresh(context));
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
        var command = Command("init", "Initialise un root DevWorkflow local avec configs, schemas et instructions agents.");
        AddOptions(command,
            Value(OptionNames.Profile, "Profil d'initialisation.", ["business"]),
            Value(OptionNames.Root, "Root DevWorkflow a creer."),
            Flag(OptionNames.DryRun, "Simule sans ecrire."),
            Flag(OptionNames.NoSave, "Ne sauvegarde pas le root utilisateur."));
        command.SetAction(parse => InitCommand.Run(context, new InitRequest(
            parse.GetValue<string>(OptionNames.Root),
            parse.GetValue<string>(OptionNames.Profile),
            parse.GetValue<bool>(OptionNames.NoSave),
            parse.GetValue<bool>(OptionNames.DryRun))));
        return command;
    }

    private static Command Doctor(CommandContext context)
    {
        var command = Command("doctor", "Diagnostique les prerequis machine et la configuration locale du workflow.");
        AddOptions(command, Flag(OptionNames.Fix, "Corrige ce qui peut etre corrige automatiquement."));
        command.SetAction(parse => DoctorCommand.RunAsync(context, parse.GetValue<bool>(OptionNames.Fix)));
        return command;
    }

    private static Command Refresh(CommandContext context)
    {
        var command = Command("refresh", "Regenere les schemas et contextes agents sans toucher aux fichiers utilisateurs.");
        AddOptions(command,
            Value(OptionNames.Root, "Root DevWorkflow a utiliser."),
            Value(OptionNames.Profile, "Profil a utiliser pour les fichiers d'agents.", ["business", "default"]));
        command.SetAction(parse => RefreshCommand.Run(context, parse.GetValue<string>(OptionNames.Root), parse.GetValue<string>(OptionNames.Profile)));
        return command;
    }

    private static Command Agent(CommandContext context)
    {
        var command = Command("agent", "Affiche le contexte workflow pour IA, ouvre un agent, ou gere sa configuration.");
        AddSubcommands(command,
            Subcommand("context", "Affiche le contexte court pour agents IA.", (_, _) => AgentCommand.WriteContext(context)),
            Subcommand("open", "Ouvre un workspace dans l'agent configure.", (parse, _) => WorkspaceOpenService.Open(context, OpenOptions(parse)),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Filtre projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Reprend la derniere session/workspace."),
                    AgentOption(),
                    RepoOption(context, "Repo cible dans le workspace.")
                ]),
            Subcommand("config", "Lit ou modifie la configuration agent.", (parse, _) => AgentCommand.ShowConfig(context, parse.GetValue<string>(OptionNames.Root)),
                [Value(OptionNames.Root, "Root DevWorkflow a utiliser.")]),
            Subcommand("show", "Affiche la configuration courante.", (parse, _) => AgentCommand.ShowConfig(context, parse.GetValue<string>(OptionNames.Root)),
                [Value(OptionNames.Root, "Root DevWorkflow a utiliser.")]),
            Subcommand("set-default", "Definit l'agent par defaut.", (parse, _) => AgentCommand.SetDefaultAgent(context, parse.GetValue<string>(OptionNames.Root), parse.GetRequiredValue<string>("agent")),
                [Value(OptionNames.Root, "Root DevWorkflow a utiliser.")],
                Argument<string>("agent", "Agent a utiliser par defaut.")),
            Subcommand("doctor", "Verifie les agents disponibles.", (parse, _) => AgentCommand.Doctor(context, parse.GetValue<string>(OptionNames.Agent)),
                [AgentOption()]));
        return command;
    }

    private static Command Auth(CommandContext context)
    {
        var command = Command("auth", "Gere la connexion Azure DevOps utilisee par les commandes ado/task.");
        AddSubcommands(command,
            Subcommand("login", "Connecte Azure DevOps.", (parse, _) => AuthCommand.Login(context, parse.GetValue<string>(OptionNames.Root)),
                [Value(OptionNames.Root, "Root DevWorkflow a utiliser.")]),
            Subcommand("status", "Affiche l'etat de connexion.", (parse, _) => AuthCommand.Status(context, parse.GetValue<string>(OptionNames.Root)),
                [Value(OptionNames.Root, "Root DevWorkflow a utiliser.")]),
            Subcommand("logout", "Supprime la connexion locale.", (parse, _) => AuthCommand.Logout(context, parse.GetValue<string>(OptionNames.Root)),
                [Value(OptionNames.Root, "Root DevWorkflow a utiliser.")]));
        return command;
    }

    private static Command Task(CommandContext context)
    {
        var command = Command("task", "Gere le cycle de travail: workspace, worktrees, plan, commits, push, PR et cleanup.");
        AddSubcommands(command,
            Subcommand("start", "Cree le workspace de travail, les worktrees associes et le contexte initial.", parse => TaskStartService.Start(context, new TaskStartRequest(
                parse.GetRequiredValue<string>("work-item-id"),
                parse.GetValue<string>(OptionNames.Project),
                parse.GetValue<string>(OptionNames.Task),
                parse.GetValue<string>(OptionNames.Type),
                parse.GetValue<string>(OptionNames.Only),
                parse.GetValue<string>(OptionNames.Slug),
                parse.GetValue<bool>(OptionNames.SkipAdo),
                parse.GetValue<bool>(OptionNames.CreateChildTasks),
                parse.GetValue<bool>(OptionNames.WithActiveChildren))),
                [
                    ProjectOption(context, "Projet dw."),
                    Value(OptionNames.Task, "ID de tache ADO concrete."),
                    Value(OptionNames.Slug, "Texte source du slug."),
                    Value(OptionNames.Type, "Type de branche."),
                    Value(OptionNames.Only, "Repos a creer, separes par virgule."),
                    Flag(OptionNames.SkipAdo, "Ignore Azure DevOps."),
                    Flag(OptionNames.CreateChildTasks, "Cree les taches ADO enfant."),
                    Flag(OptionNames.WithActiveChildren, "Inclut automatiquement les enfants ADO non finaux du sujet selectionne.")
                ],
                Argument<string>("work-item-id", "ID du work item parent principal, ou liste separee par virgules.")),
            Subcommand("status", "Liste rapidement les chemins de workspaces detectes.", (_, _) => TaskListService.Status(context)),
            Subcommand("list", "Liste les workspaces avec projet, work items, branche et metadonnees utiles.", parse => TaskListService.List(context, new TaskListOptions(parse.GetValue<string>(OptionNames.Project), parse.GetValue<string>(OptionNames.WorkItem), parse.GetValue<bool>(OptionNames.Json))),
                [
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Json, "Sortie JSON.")
                ]),
            Subcommand("current", "Affiche le workspace courant et son contexte principal.", parse => TaskListService.Current(context, parse.GetValue<bool>(OptionNames.Json)),
                [Flag(OptionNames.Json, "Sortie JSON.")]),
            Subcommand("sync", "Rafraichit le contexte ADO local du workspace dans task.json.", parse => TaskSyncPruneService.Sync(context, OpenOptions(parse)),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent.")
                ]),
            Subcommand("prune", "Supprime les workspaces dont tous les work items sont dans un etat final.", parse => TaskSyncPruneService.Prune(context, new TaskPruneOptions(parse.GetValue<string>(OptionNames.Project), parse.GetValue<string>(OptionNames.WorkItem), parse.GetValue<bool>(OptionNames.Execute), parse.GetValue<bool>(OptionNames.Yes), !parse.GetValue<bool>(OptionNames.NoSync))),
                [
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Execute, "Execute vraiment l'action."),
                    Flag(OptionNames.Yes, "Confirme sans prompt."),
                    Flag(OptionNames.NoSync, "Desactive le sync ADO automatique.")
                ]),
            Subcommand("rename", "Renomme slug, branche et dossier workspace.", parse => TaskRenameService.Rename(context, new TaskRenameOptions(parse.GetRequiredValue<string>(OptionNames.Slug), OpenOptions(parse), parse.GetValue<bool>(OptionNames.Execute))),
                [
                    Value(OptionNames.Slug, "Texte source du slug."),
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent."),
                    Flag(OptionNames.Execute, "Execute vraiment l'action.")
                ]),
            Subcommand("open", "Ouvre ou reprend le workspace dans l'agent choisi pour travailler dessus.", (parse, _) => WorkspaceOpenService.Open(context, OpenOptions(parse)),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    RepoOption(context, "Repo cible dans le workspace."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent."),
                    AgentOption()
                ],
                Argument<string?>("work-item-id", "ID du work item a ouvrir, ou liste separee par virgules.")),
            Subcommand("teardown", "Supprime les worktrees et le workspace.", (parse, _) => WorkspaceTeardownService.Teardown(context, TeardownOptions(parse)),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent."),
                    Flag(OptionNames.Execute, "Execute vraiment l'action."),
                    Flag(OptionNames.Yes, "Confirme sans prompt.")
                ]),
            Subcommand("add-repo", "Ajoute un repo au workspace existant.", parse => TaskCommand.AddRepo(context, new TaskAddRepoOptions(parse.GetRequiredValue<string>("repo"), parse.GetValue<string>(OptionNames.Workspace))),
                [WorkspaceOption(context, "Chemin explicite du workspace.")],
                Argument<string>("repo", "Repo a ajouter.")),
            Subcommand("create-child-task", "Cree une sous-tache ADO liee au work item principal du workspace.", parse => TaskChildTaskService.Create(context, new TaskChildTaskCreateOptions(
                    parse.GetRequiredValue<string>(OptionNames.Repo),
                    parse.GetRequiredValue<string>(OptionNames.Title),
                    OpenOptions(parse))),
                [
                    Value(OptionNames.Repo, "Repo ou domaine de la sous-tache, par exemple front, back, db ou foo."),
                    Value(OptionNames.Title, "Titre explicite de la sous-tache, sans prefixe [FRONT]/[BACK]."),
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent.")
                ]),
            Subcommand("add-work-item", "Ajoute un ou plusieurs work items au workspace existant.", parse => TaskWorkItemService.Add(context, new TaskWorkItemUpdateOptions(parse.GetRequiredValue<string>("ids"), OpenOptions(parse))),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent.")
                ],
                Argument<string>("ids", "ID du work item a ajouter, ou liste separee par virgules.")),
            Subcommand("remove-work-item", "Retire un ou plusieurs work items du workspace existant.", parse => TaskWorkItemService.Remove(context, new TaskWorkItemUpdateOptions(parse.GetRequiredValue<string>("ids"), OpenOptions(parse))),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    ProjectOption(context, "Projet dw."),
                    WorkItemOption(context, "Filtre work item ADO."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent.")
                ],
                Argument<string>("ids", "ID du work item a retirer, ou liste separee par virgules.")),
            Subcommand("commit", "Cree un commit intermediaire dans le workspace sans push ni PR.", parse => TaskCommand.Commit(context, new TaskCommitRequest(
                parse.GetValue<string>(OptionNames.Workspace),
                parse.GetValue<bool>(OptionNames.Continue),
                parse.GetValue<bool>(OptionNames.Execute),
                parse.GetValue<string>(OptionNames.Message))),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent."),
                    Flag(OptionNames.Execute, "Execute vraiment l'action."),
                    Value(OptionNames.Message, "Override explicite du message de commit genere.")
                ]),
            Subcommand("finish", "Prepare ou execute la fin du workflow: verification, commit, push et eventuelle PR.", parse => TaskCommand.Finish(context, new TaskFinishRequest(
                parse.GetValue<string>(OptionNames.Workspace),
                parse.GetValue<bool>(OptionNames.Continue),
                parse.GetValue<bool>(OptionNames.Execute),
                parse.GetValue<bool>(OptionNames.CreatePr),
                parse.GetValue<bool>(OptionNames.Ready),
                parse.GetValue<string>(OptionNames.Message),
                parse.GetValue<bool>(OptionNames.SkipVerify),
                parse.GetValue<bool>(OptionNames.SkipAdo))),
                [
                    WorkspaceOption(context, "Chemin explicite du workspace."),
                    Flag(OptionNames.Continue, "Utilise le workspace le plus recent."),
                    Flag(OptionNames.Execute, "Execute vraiment l'action."),
                    Flag(OptionNames.CreatePr, "Ouvre une PR apres push."),
                    Flag(OptionNames.Ready, "Cree une PR non draft."),
                    Value(OptionNames.Message, "Override explicite du message de commit genere."),
                    Flag(OptionNames.SkipVerify, "Ignore les verifications configurees."),
                    Flag(OptionNames.SkipAdo, "Ignore Azure DevOps.")
                ]));
        return command;
    }

    private static Command Config(CommandContext context)
    {
        var command = Command("config", "Valide et modifie la configuration.");
        AddSubcommands(command,
            Subcommand("show", "Affiche le root configure.", (_, _) => ConfigCommand.Show(context)),
            Subcommand("set-root", "Definit le root DevWorkflow.", (parse, _) => ConfigCommand.SetRoot(context, parse.GetRequiredValue<string>("path")), Argument<string>("path", "Chemin du root DevWorkflow.")),
            Subcommand("set-color", "Definit le mode de couleur du terminal.", (parse, _) => ConfigCommand.SetColor(context, parse.GetRequiredValue<string>("mode")), Argument<string>("mode", "Mode couleur: auto, always, never.")),
            Subcommand("doctor", "Valide les fichiers config.", (parse, _) => ConfigCommand.Doctor(context, parse.GetValue<string>(OptionNames.Root)),
                [Value(OptionNames.Root, "Root a utiliser pour cette commande.")]));
        return command;
    }

    private static Command Secret(CommandContext context)
    {
        var command = Command("secret", "Stocke des secrets locaux via Windows Credential Manager.");
        AddSubcommands(command,
            Subcommand("set", "Cree ou remplace un secret.", (parse, _) => SecretCommand.Set(context, parse.GetRequiredValue<string>("key"), parse.GetValue<string>(OptionNames.Value), parse.GetValue<string>(OptionNames.FromEnv)),
                [
                    Value(OptionNames.Value, "Valeur du secret."),
                    Value(OptionNames.FromEnv, "Nom de variable d'environnement source.")
                ],
                Argument<string>("key", "Cle du secret.")),
            Subcommand("get", "Lit un secret.", (parse, _) => SecretCommand.Get(context, parse.GetRequiredValue<string>("key")), Argument<string>("key", "Cle du secret.")),
            Subcommand("delete", "Supprime un secret.", (parse, _) => SecretCommand.Delete(context, parse.GetRequiredValue<string>("key")), Argument<string>("key", "Cle du secret.")));
        return command;
    }

    private static Command Upgrade(CommandContext context)
    {
        var command = Command("upgrade", "Met a jour le binaire dw depuis la derniere release configuree.");
        AddOptions(command,
            Flag(OptionNames.Check, "Verifie la derniere release sans modifier le binaire."),
            Value(OptionNames.Rid, "Runtime identifier cible."));
        command.SetAction(parse =>
        {
            EnsureMutuallyExclusive(parse, OptionNames.Check, OptionNames.Rid);
            return parse.GetValue<bool>(OptionNames.Check)
                ? UpgradeCommand.Check(context)
                : UpgradeCommand.Run(context, parse.GetValue<string>(OptionNames.Rid));
        });
        return command;
    }

    private static WorkspaceOpenOptions OpenOptions(ParseResult parse)
        => new(
            Workspace: parse.GetValue<string>(OptionNames.Workspace),
            Project: parse.GetValue<string>(OptionNames.Project),
            WorkItemId: parse.GetValue<string>(OptionNames.WorkItem),
            Continue: parse.GetValue<bool>(OptionNames.Continue),
            PositionalWorkItemId: parse.GetValue<string>("work-item-id"),
            Agent: parse.GetValue<string>(OptionNames.Agent),
            Repository: parse.GetValue<string>(OptionNames.Repo));

    private static WorkspaceTeardownOptions TeardownOptions(ParseResult parse)
        => new(
            Workspace: parse.GetValue<string>(OptionNames.Workspace),
            Project: parse.GetValue<string>(OptionNames.Project),
            WorkItemId: parse.GetValue<string>(OptionNames.WorkItem),
            Continue: parse.GetValue<bool>(OptionNames.Continue),
            Execute: parse.GetValue<bool>(OptionNames.Execute),
            Yes: parse.GetValue<bool>(OptionNames.Yes));

}
