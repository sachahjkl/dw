use anyhow::Result;
use clap::Subcommand;

use crate::commands;

#[derive(Debug, Subcommand)]
pub enum AdoCommand {
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
        #[arg(long, help = "Repository local utilise pour le mode --from-git.")]
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

pub fn handle_ado(command: AdoCommand) -> Result<()> {
    match command {
        AdoCommand::Assigned {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        } => commands::assigned::handle(commands::assigned::AssignedArgs {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        })?,
        AdoCommand::Changelog {
            ids,
            root,
            project,
            from_pr,
            from_git,
            repo,
            group_by_parent,
            format,
            table,
            ids_only,
            git_to,
        } => commands::changelog::handle(commands::changelog::ChangelogArgs {
            ids,
            root,
            project,
            from_pr,
            from_git,
            repo,
            group_by_parent,
            format,
            table,
            ids_only,
            git_to,
        })?,
        AdoCommand::WorkItem {
            id,
            root,
            project,
            json,
        } => commands::work_item::handle(commands::work_item::WorkItemArgs {
            id,
            root,
            project,
            json,
        })?,
        AdoCommand::Context {
            id,
            root,
            project,
            summary,
            comments,
            json,
        } => commands::context::handle_context(commands::context::ContextArgs {
            id,
            root,
            project,
            summary,
            comments,
            json,
        })?,
        AdoCommand::AiContext {
            root,
            organization,
            project,
            id,
            summary,
            comments,
            include_comments,
        } => commands::context::handle_ai_context(commands::context::AiContextArgs {
            root,
            organization,
            project,
            id,
            summary,
            comments,
            include_comments,
        })?,
    }

    Ok(())
}
