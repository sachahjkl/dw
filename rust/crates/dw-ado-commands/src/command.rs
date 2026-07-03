use anyhow::Result;
use clap::Subcommand;

use crate::commands;

#[derive(Debug, Subcommand)]
pub enum AdoCommand {
    #[command(about = "Liste les work items Azure DevOps assignes a l'utilisateur courant.")]
    Assigned {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value_t = 20)]
        top: i32,
        #[arg(long)]
        all: bool,
        #[arg(long = "group-by-parent")]
        group_by_parent: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Construit un changelog depuis des PR, une plage git ou des work items.")]
    Changelog {
        ids: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "from-pr", conflicts_with = "from_git")]
        from_pr: bool,
        #[arg(long = "from-git", conflicts_with = "from_pr")]
        from_git: bool,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long = "group-by-parent")]
        group_by_parent: bool,
        #[arg(long, value_parser = ["raw", "markdown", "html"])]
        format: Option<String>,
        #[arg(long, requires = "format")]
        table: bool,
        #[arg(long = "ids-only")]
        ids_only: bool,
        #[arg(long = "git-to", requires = "from_git")]
        git_to: Option<String>,
    },
    #[command(about = "Affiche un resume lisible de work items Azure DevOps.")]
    WorkItem {
        id: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Affiche le contexte detaille d'un work item pour lecture humaine.")]
    Context {
        id: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        summary: bool,
        #[arg(long, default_value_t = 200)]
        comments: i32,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Emet le contexte IA structure et deterministe d'un work item.")]
    AiContext {
        id: String,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        summary: bool,
        #[arg(long, default_value_t = 200)]
        comments: i32,
        #[arg(long = "include-comments")]
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
            comments: _,
            include_comments,
        } => commands::context::handle_ai_context(commands::context::AiContextArgs {
            root,
            organization,
            project,
            id,
            summary,
            include_comments,
        })?,
    }

    Ok(())
}
