use clap::{Arg, ArgAction, CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::Shell;
use dw_completion::CompletionOutput;

#[derive(Debug, Parser)]
#[command(name = "dw")]
#[command(bin_name = "dw")]
#[command(version = crate::version::PACKAGE_VERSION)]
#[command(propagate_version = true)]
#[command(about = "Dev Workflow")]
#[command(help_template = "{about} {version}\n\n{usage-heading} {usage}\n\n{all-args}")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

impl Cli {
    pub(crate) fn parse_localized() -> Self {
        let command = Self::localized_command();
        let matches = command.get_matches();
        Self::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
    }

    pub(crate) fn localized_command() -> clap::Command {
        let display_version: &'static str =
            Box::leak(crate::version::informational_version().into_boxed_str());
        localize_command(Self::command().version(display_version))
    }
}

fn localize_command(command: clap::Command) -> clap::Command {
    let help_template = if command.get_name() == "dw" {
        "{about} {version}\n\nUtilisation: {usage}\n\n{all-args}"
    } else {
        "{about-with-newline}\nUtilisation: {usage}\n\n{all-args}"
    };

    command
        .help_template(help_template)
        .subcommand_help_heading("Commandes")
        .disable_help_subcommand(true)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("help")
                .short('h')
                .long("help")
                .action(ArgAction::Help)
                .help("Afficher l'aide."),
        )
        .arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .action(ArgAction::Version)
                .help("Afficher la version."),
        )
        .mut_subcommands(localize_command)
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(about = "Affiche la version du CLI.")]
    Version,
    #[command(about = "Explique le parcours de démarrage.", alias = "get-started")]
    Guide,
    #[command(about = "Diagnostique les prérequis machine et la configuration locale.")]
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    #[command(about = "Initialise un root DevWorkflow local.")]
    Init {
        #[arg(long, default_value = "business")]
        profile: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long = "dry-run")]
        dry_run: bool,
        #[arg(long = "no-save")]
        no_save: bool,
    },
    #[command(about = "Régénère schémas et contextes agents.")]
    Refresh {
        #[arg(long)]
        root: Option<String>,
        #[arg(long, default_value = "business")]
        profile: String,
    },
    #[command(about = "Ouvre le dashboard TUI DevWorkflow.")]
    Tui {
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
    },
    #[command(about = "Affiche le contexte workflow IA, ouvre un agent, ou gère sa configuration.")]
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    #[command(about = "Gère la connexion Azure DevOps.")]
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    #[command(about = "Installe ou interroge l'autocomplétion shell.")]
    Completion {
        #[command(subcommand)]
        command: CompletionCommand,
    },
    #[command(about = "Valide et modifie la configuration.")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(about = "Commandes Azure DevOps.")]
    Ado {
        #[command(subcommand)]
        command: AdoCommand,
    },
    #[command(about = "Explore et protège les accès base de données.")]
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    #[command(about = "Stocke des secrets locaux.")]
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    #[command(about = "Met à jour le binaire dw.")]
    Upgrade {
        #[arg(long, conflicts_with = "rid")]
        check: bool,
        #[arg(long, conflicts_with = "check")]
        rid: Option<String>,
    },
    #[command(about = "Gère le cycle de travail: workspace, worktrees, commits, PR et cleanup.")]
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthCommand {
    #[command(about = "Connecte Azure DevOps.")]
    Login {
        #[arg(long, help = "Root DevWorkflow à utiliser pour la configuration auth.")]
        root: Option<String>,
    },
    #[command(about = "Affiche l'état de connexion Azure DevOps.")]
    Status {
        #[arg(long, help = "Root DevWorkflow à utiliser pour la configuration auth.")]
        root: Option<String>,
    },
    #[command(about = "Supprime la session Azure DevOps locale.")]
    Logout {
        #[arg(long, help = "Root DevWorkflow à utiliser pour la configuration auth.")]
        root: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AdoCommand {
    #[command(about = "Liste les work items Azure DevOps assignés à l'utilisateur courant.")]
    Assigned {
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Projet configuré à interroger; ouvre un choix interactif si omis."
        )]
        project: Option<String>,
        #[arg(
            long,
            default_value_t = 20,
            help = "Nombre maximum de work items à charger."
        )]
        top: i32,
        #[arg(long, help = "Inclure aussi les work items en état final.")]
        all: bool,
        #[arg(
            long = "group-by-parent",
            help = "Regrouper les work items par parent ADO."
        )]
        group_by_parent: bool,
        #[arg(long, help = "Émettre la réponse JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Liste les pull requests actives Azure DevOps des repositories configurés.")]
    Prs {
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à interroger.")]
        project: String,
        #[arg(
            long,
            help = "Repository local ou Azure DevOps à interroger; répétition via virgules."
        )]
        repo: Option<String>,
        #[arg(long, help = "Émettre la réponse JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Construit un changelog depuis des PR, une plage git ou des work items.")]
    Changelog {
        #[arg(help = "IDs de work items, PRs, ou plage git selon le mode choisi.")]
        ids: String,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à utiliser.")]
        project: Option<String>,
        #[arg(
            long = "from-pr",
            conflicts_with = "from_git",
            help = "Interpréter les IDs comme des pull requests Azure DevOps."
        )]
        from_pr: bool,
        #[arg(
            long = "from-git",
            conflicts_with = "from_pr",
            help = "Extraire les work items depuis les commits git."
        )]
        from_git: bool,
        #[arg(long, help = "Repository local utilisé pour le mode --from-git.")]
        repo: Option<String>,
        #[arg(
            long = "group-by-parent",
            help = "Regrouper le changelog par parent ADO."
        )]
        group_by_parent: bool,
        #[arg(long, value_parser = ["raw", "markdown", "html"], help = "Format de sortie.")]
        format: Option<String>,
        #[arg(
            long,
            requires = "format",
            help = "Rendre le changelog markdown/html en table."
        )]
        table: bool,
        #[arg(
            long = "ids-only",
            help = "Afficher uniquement les IDs résolus, séparés par espaces."
        )]
        ids_only: bool,
        #[arg(
            long = "git-to",
            requires = "from_git",
            help = "Revision de fin pour la plage git."
        )]
        git_to: Option<String>,
    },
    #[command(about = "Affiche un résumé lisible de work items Azure DevOps.")]
    WorkItem {
        #[arg(help = "ID du work item Azure DevOps, ou liste séparée par virgules.")]
        id: String,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à utiliser.")]
        project: Option<String>,
        #[arg(long, help = "Émettre la réponse JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Change l'état d'un ou plusieurs work items Azure DevOps.")]
    SetState {
        #[arg(help = "ID du work item Azure DevOps, ou liste séparée par virgules.")]
        id: String,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à utiliser.")]
        project: Option<String>,
        #[arg(long, help = "Nouvel état ADO exact à appliquer.")]
        state: String,
        #[arg(long, help = "Message d'historique ADO; par défaut: dw ado set-state.")]
        history: Option<String>,
        #[arg(long, help = "Confirmer le changement d'état destructif.")]
        yes: bool,
        #[arg(long, help = "Émettre la réponse JSON déterministe; requiert --yes.")]
        json: bool,
    },
    #[command(
        about = "Affiche le contexte détaillé d'un ou plusieurs work items pour lecture humaine."
    )]
    Context {
        #[arg(help = "ID du work item Azure DevOps, ou liste séparée par virgules.")]
        id: String,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à utiliser.")]
        project: Option<String>,
        #[arg(long, help = "Limiter le contexte aux champs essentiels.")]
        summary: bool,
        #[arg(
            long,
            default_value_t = 200,
            help = "Nombre maximum de commentaires à afficher; 0 pour aucun."
        )]
        comments: i32,
        #[arg(long, help = "Émettre la réponse JSON déterministe.")]
        json: bool,
    },
    #[command(
        about = "Émet le contexte IA structuré et déterministe d'un ou plusieurs work items."
    )]
    AiContext {
        #[arg(help = "ID du work item Azure DevOps, ou liste séparée par virgules.")]
        id: String,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Organisation Azure DevOps explicite.")]
        organization: Option<String>,
        #[arg(long, help = "Projet configuré ou projet Azure DevOps explicite.")]
        project: Option<String>,
        #[arg(long, help = "Limiter le contrat aux champs essentiels.")]
        summary: bool,
        #[arg(
            long,
            default_value_t = 200,
            help = "Nombre maximum de commentaires à inclure."
        )]
        comments: i32,
        #[arg(
            long = "include-comments",
            help = "Inclure les commentaires dans le contexte IA."
        )]
        include_comments: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    #[command(about = "Liste les workspaces task détectés sous le root.")]
    Status {
        #[arg(long, help = "Root DevWorkflow à scanner.")]
        root: Option<String>,
    },
    #[command(about = "Liste les workspaces task avec filtres projet/work item.")]
    List {
        #[arg(long, help = "Root DevWorkflow à scanner.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à filtrer.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item à filtrer.")]
        work_item: Option<String>,
        #[arg(long, help = "Émettre la liste JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Affiche le workspace task courant depuis le répertoire actuel.")]
    Current {
        #[arg(long, help = "Émettre le workspace courant en JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Ouvre ou reprend un workspace task avec l'agent configuré.")]
    Open {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace à ouvrir directement.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré utilisé pour résoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Repository à ouvrir dans le workspace.")]
        repo: Option<String>,
        #[arg(
            long,
            help = "Agent à lancer: opencode, cursor, claude, codex, codex-cli ou copilot."
        )]
        agent: Option<String>,
        #[arg(long, help = "Émettre la résolution JSON au lieu de lancer l'agent.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Prépare ou crée un workspace task depuis des work items ADO.")]
    Start {
        #[arg(help = "Identifiant du work item ADO parent ou enfant à démarrer.")]
        work_item_id: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à utiliser.")]
        project: Option<String>,
        #[arg(
            long = "task",
            help = "Identifiant de la tâche enfant à ajouter au workspace."
        )]
        task: Option<String>,
        #[arg(
            long = "type",
            help = "Type de branche/workspace: feature, bugfix, hotfix ou chore."
        )]
        type_name: Option<String>,
        #[arg(
            long = "only",
            help = "Repository à inclure; répétable via sélection interactive si omis."
        )]
        only: Option<String>,
        #[arg(long, help = "Slug explicite pour le nom de branche et workspace.")]
        slug: Option<String>,
        #[arg(
            long = "skip-ado",
            help = "Ne pas interroger Azure DevOps; utiliser les valeurs locales fournies."
        )]
        skip_ado: bool,
        #[arg(
            long = "with-active-children",
            conflicts_with = "skip_ado",
            help = "Inclure automatiquement les enfants ADO non finaux du sujet sélectionné."
        )]
        with_active_children: bool,
        #[arg(
            long = "create-child-tasks",
            conflicts_with = "skip_ado",
            help = "Créer une sous-tâche ADO par repository inclus avant de créer le workspace."
        )]
        create_child_tasks: bool,
        #[arg(long, help = "Émet le plan ou résultat en JSON déterministe.")]
        json: bool,
        #[arg(
            long,
            help = "Crée vraiment le workspace; sans ce flag, affiche le plan."
        )]
        execute: bool,
    },
    #[command(
        about = "Prépare ou crée un workspace depuis les work items liés à une pull request."
    )]
    StartPr {
        #[arg(help = "ID de pull request Azure DevOps.")]
        pull_request_id: String,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à utiliser.")]
        project: String,
        #[arg(long, help = "Repository local ou Azure DevOps de la PR.")]
        repo: Option<String>,
        #[arg(
            long = "type",
            help = "Type de branche/workspace: feature, bugfix, hotfix ou chore."
        )]
        type_name: Option<String>,
        #[arg(long, help = "Slug explicite pour le nom de branche et workspace.")]
        slug: Option<String>,
        #[arg(long, help = "Émet le plan ou résultat en JSON déterministe.")]
        json: bool,
        #[arg(
            long,
            help = "Crée vraiment le workspace; sans ce flag, affiche le plan."
        )]
        execute: bool,
    },
    #[command(about = "Valide les bloqueurs et avertissements avant implémentation.")]
    Preflight {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace à auditer.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré utilisé pour résoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long = "ai-context-file",
            help = "Fichier de contexte IA additionnel à vérifier; option répétable."
        )]
        ai_context_file: Vec<String>,
        #[arg(long, help = "Émettre le rapport preflight JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Synchronise task.json avec les work items Azure DevOps.")]
    Sync {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace à synchroniser.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré utilisé pour résoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Renomme un workspace task et sa branche selon un nouveau slug.")]
    Rename {
        #[arg(help = "Nouveau slug pour le workspace et la branche.")]
        slug: String,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace à renommer.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré utilisé pour résoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
        #[arg(
            long,
            help = "Appliquer vraiment le rename; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Met les repositories du workspace à jour depuis leur branche cible.")]
    RepoLatest {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Chemin du workspace à synchroniser."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long = "only",
            help = "Limiter la synchronisation à un repository du workspace."
        )]
        only: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
    },
    #[command(
        about = "Prépare ou crée un commit intermédiaire pour les repositories du workspace."
    )]
    Commit {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Chemin du workspace à committer."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Créer vraiment les commits; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(
            long,
            help = "Message de commit explicite; sinon généré depuis le manifeste task."
        )]
        message: Option<String>,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Ajoute des work items au workspace task courant.")]
    AddWorkItem {
        #[arg(help = "IDs de work items à ajouter, séparés par virgules.")]
        work_item_ids: Option<String>,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace à modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré utilisé pour résoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long = "skip-ado",
            help = "Ne pas interroger Azure DevOps; utiliser les valeurs fournies localement."
        )]
        skip_ado: bool,
        #[arg(
            long = "type",
            help = "Type local à utiliser si ADO est ignoré ou incomplet."
        )]
        type_name: Option<String>,
        #[arg(long, help = "Titre local à utiliser si ADO est ignoré ou incomplet.")]
        title: Option<String>,
        #[arg(long, help = "État local à utiliser si ADO est ignoré ou incomplet.")]
        state: Option<String>,
        #[arg(
            long,
            help = "Appliquer vraiment la modification; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Retire des work items du workspace task courant.")]
    RemoveWorkItem {
        #[arg(help = "IDs de work items à retirer, séparés par virgules.")]
        work_item_ids: Option<String>,
        #[arg(long, help = "Chemin du workspace à modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré utilisé pour résoudre le workspace.")]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long,
            help = "Appliquer vraiment la modification; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Ajoute un repository au workspace task.")]
    AddRepo {
        #[arg(help = "Repository configuré à ajouter au workspace.")]
        repo: Option<String>,
        #[arg(long, help = "Chemin du workspace à modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Créer le worktree et modifier task.json; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Crée une tâche enfant ADO et l'ajoute au handoff repository.")]
    CreateChildTask {
        #[arg(
            long,
            help = "Repository du workspace qui portera le handoff de la tâche."
        )]
        repo: String,
        #[arg(long, help = "Titre de la tâche enfant ADO à créer.")]
        title: String,
        #[arg(long, help = "Chemin du workspace à modifier.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré utilisé pour résoudre le workspace.")]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Vérifie, commit, push et ouvre une PR pour terminer le workspace.")]
    Finish {
        #[arg(
            long,
            conflicts_with = "continue",
            help = "Chemin du workspace à terminer."
        )]
        workspace: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            help = "Exécuter les commits, pushs, PR et mises à jour ADO; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirmer la finalisation destructive avec --execute.")]
        yes: bool,
        #[arg(
            long,
            help = "Message de commit explicite; sinon généré depuis le manifeste task."
        )]
        message: Option<String>,
        #[arg(
            long = "create-pr",
            help = "Créer ou vérifier les pull requests Azure DevOps après push."
        )]
        create_pr: bool,
        #[arg(
            long,
            requires = "create_pr",
            help = "Créer les PR en état ready au lieu de draft."
        )]
        ready: bool,
        #[arg(
            long = "skip-verify",
            help = "Ignorer les commandes de vérification configurées avant PR."
        )]
        skip_verify: bool,
        #[arg(
            long = "skip-ado",
            help = "Ne pas appeler Azure DevOps; incompatible avec --create-pr."
        )]
        skip_ado: bool,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Valide les fichiers handoff avant sous-agents ou finition.")]
    HandoffValidate {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"], help = "Chemin du workspace dont les handoffs doivent être valides.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré utilisé pour résoudre le workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Supprime les worktrees et nettoie un workspace task.")]
    Teardown {
        #[arg(long, help = "Chemin du workspace à supprimer.")]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré utilisé pour résoudre le workspace.")]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item utilisé pour résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(
            long,
            help = "Supprimer vraiment les worktrees et le workspace; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirmer la suppression destructive avec --execute.")]
        yes: bool,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Nettoie les workspaces dont les work items sont terminés.")]
    Prune {
        #[arg(long, help = "Root DevWorkflow à scanner.")]
        root: Option<String>,
        #[arg(long, help = "Projet configuré à filtrer.")]
        project: Option<String>,
        #[arg(long = "work-item", help = "Work item à filtrer.")]
        work_item: Option<String>,
        #[arg(
            long,
            help = "Supprimer vraiment les workspaces éligibles; sans ce flag, affiche le plan."
        )]
        execute: bool,
        #[arg(long, help = "Confirmer la suppression destructive avec --execute.")]
        yes: bool,
        #[arg(
            long = "no-sync",
            help = "Ne pas synchroniser les états ADO avant de déterminer l'éligibilité."
        )]
        no_sync: bool,
        #[arg(long, help = "Émettre le plan/résultat JSON déterministe.")]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    #[command(about = "Affiche le root, le mode couleur et les chemins de configuration.")]
    Show {
        #[arg(long, help = "Root DevWorkflow à inspecter.")]
        root: Option<String>,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Vérifie les fichiers de configuration et les schémas locaux.")]
    Doctor {
        #[arg(long, help = "Root DevWorkflow à vérifier.")]
        root: Option<String>,
        #[arg(long, help = "Émettre le rapport JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Enregistre le root DevWorkflow utilisateur.")]
    SetRoot {
        #[arg(help = "Chemin du root DevWorkflow à enregistrer.")]
        path: String,
    },
    #[command(about = "Configure le mode couleur: auto, always ou never.")]
    SetColor {
        #[arg(help = "Mode couleur à enregistrer: auto, always ou never.")]
        mode: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum AgentCommand {
    #[command(about = "Affiche le contexte DevWorkflow injecté aux agents IA.")]
    Context,
    #[command(about = "Ouvre ou reprend un agent sur un workspace task.")]
    Open {
        #[arg(
            long,
            conflicts_with_all = ["project", "work_item", "continue"],
            help = "Chemin du workspace à ouvrir directement."
        )]
        workspace: Option<String>,
        #[arg(long, help = "Root DevWorkflow à utiliser.")]
        root: Option<String>,
        #[arg(
            long,
            conflicts_with = "workspace",
            help = "Projet configuré à utiliser pour résoudre un workspace."
        )]
        project: Option<String>,
        #[arg(
            long = "work-item",
            help = "Work item servant à résoudre le workspace."
        )]
        work_item: Option<String>,
        #[arg(
            long = "continue",
            conflicts_with = "workspace",
            help = "Reprendre le workspace task le plus récent correspondant."
        )]
        r#continue: bool,
        #[arg(long, help = "Repository à ouvrir dans le workspace, si applicable.")]
        repo: Option<String>,
        #[arg(
            long,
            help = "Agent à lancer: opencode, cursor, claude, codex, codex-cli ou copilot."
        )]
        agent: Option<String>,
        #[arg(help = "Alias positionnel du work item pour résoudre le workspace.")]
        positional_work_item: Option<String>,
    },
    #[command(about = "Affiche la configuration agent effective.")]
    Config {
        #[arg(long, help = "Root DevWorkflow à lire.")]
        root: Option<String>,
    },
    #[command(about = "Affiche la configuration agent effective.")]
    Show {
        #[arg(long, help = "Root DevWorkflow à lire.")]
        root: Option<String>,
    },
    #[command(about = "Définit l'agent par défaut du root DevWorkflow.")]
    SetDefault {
        #[arg(
            help = "Agent à utiliser par défaut: opencode, cursor, claude, codex, codex-cli ou copilot."
        )]
        agent: String,
        #[arg(long, help = "Root DevWorkflow à modifier.")]
        root: Option<String>,
    },
    #[command(about = "Diagnostique la disponibilité des agents installés.")]
    Doctor {
        #[arg(long, help = "Limiter le diagnostic à un agent.")]
        agent: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SecretCommand {
    #[command(about = "Enregistre un secret dans le keyring système.")]
    Set {
        #[arg(help = "Clé logique du secret, par exemple une credentialReference.")]
        key: String,
        #[arg(
            long,
            conflicts_with = "from_env",
            help = "Valeur du secret à enregistrer."
        )]
        value: Option<String>,
        #[arg(
            long = "from-env",
            conflicts_with = "value",
            help = "Nom de variable d'environnement contenant le secret."
        )]
        from_env: Option<String>,
    },
    #[command(about = "Vérifie si un secret existe sans afficher sa valeur.")]
    Get {
        #[arg(help = "Clé logique du secret à vérifier.")]
        key: String,
    },
    #[command(about = "Supprime un secret du keyring système.")]
    Delete {
        #[arg(help = "Clé logique du secret à supprimer.")]
        key: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DbCommand {
    #[command(about = "Vérifie qu'une requête SQL respecte le mode read-only.")]
    Guard {
        #[arg(long, help = "Requête SQL à analyser sans exécution.")]
        sql: String,
    },
    #[command(about = "Liste les tables et vues accessibles sur une base configurée.")]
    Schema {
        #[arg(long, help = "Projet configuré contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion déclaré dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base déclaré dans databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Décrit les colonnes d'une table SQL.")]
    Describe {
        #[arg(help = "Table à décrire, au format table ou schema.table.")]
        table: Option<String>,
        #[arg(long, help = "Projet configuré contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion déclaré dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base déclaré dans databases.json."
        )]
        env: Option<String>,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
    },
    #[command(about = "Exécute une requête SQL read-only avec garde-fous et limite de lignes.")]
    Query {
        #[arg(long, help = "Requête SQL read-only à exécuter.")]
        sql: Option<String>,
        #[arg(long, help = "Projet configuré contenant la connexion base.")]
        project: Option<String>,
        #[arg(
            long,
            conflicts_with = "env",
            help = "Nom de connexion déclaré dans databases.json."
        )]
        database: Option<String>,
        #[arg(
            long,
            conflicts_with = "database",
            help = "Alias d'environnement base déclaré dans databases.json."
        )]
        env: Option<String>,
        #[arg(long = "max-rows", help = "Nombre maximum de lignes à afficher.")]
        #[arg(value_parser = parse_positive_usize)]
        max_rows: Option<usize>,
        #[arg(long, help = "Émettre le résultat JSON déterministe.")]
        json: bool,
        #[arg(
            value_name = "SQL",
            trailing_var_arg = true,
            help = "Requête SQL read-only à exécuter."
        )]
        sql_parts: Vec<String>,
    },
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "max-rows doit être un entier positif.".to_string())?;
    if parsed == 0 {
        return Err("max-rows doit être supérieur à 0.".into());
    }
    Ok(parsed)
}

#[derive(Debug, Subcommand)]
pub(crate) enum CompletionCommand {
    Show,
    Generate {
        shell: Shell,
    },
    Install {
        shell: Shell,
    },
    #[command(hide = true)]
    Complete {
        #[arg(long, value_enum, default_value_t = CompletionOutput::Bash)]
        format: CompletionOutput,
        words: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::Cli;

    #[test]
    fn localized_help_uses_french_builtin_labels() {
        let mut command = Cli::localized_command();
        let mut output = Vec::new();
        command.write_long_help(&mut output).expect("help output");
        let help = String::from_utf8(output).expect("utf8 help");

        assert!(help.contains("Afficher l'aide."));
        assert!(help.contains("Afficher la version."));
        assert!(!help.contains("Print help"));
        assert!(!help.contains("Print version"));
        assert!(!help.contains("Print this message"));
    }

    #[test]
    fn localized_subcommand_help_uses_french_builtin_labels() {
        let error = Cli::localized_command()
            .try_get_matches_from(["dw", "ado", "ai-context", "--help"])
            .expect_err("help exits through clap");
        let help = error.to_string();

        assert!(help.contains("Afficher l'aide."));
        assert!(help.contains("Afficher la version."));
        assert!(!help.contains("Print help"));
        assert!(!help.contains("Print version"));
    }
}
