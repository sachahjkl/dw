use anyhow::Result;
use clap::Subcommand;
use dw_agent::command::OpenAgentArgs;

pub use crate::open::{OpenWorkspaceArgs, open_workspace};

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    #[command(about = "Liste les workspaces task detectes sous le root.")]
    Status {
        #[arg(long)]
        root: Option<String>,
    },
    #[command(about = "Liste les workspaces task avec filtres projet/work item.")]
    List {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Affiche le workspace task courant depuis le repertoire actuel.")]
    Current {
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Ouvre ou reprend un workspace task avec l'agent configure.")]
    Open {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"])]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Prepare ou cree un workspace task depuis des work items ADO.")]
    Start {
        work_item_id: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "task")]
        task: Option<String>,
        #[arg(long = "type")]
        type_name: Option<String>,
        #[arg(long = "only")]
        only: Option<String>,
        #[arg(long)]
        slug: Option<String>,
        #[arg(long = "skip-ado")]
        skip_ado: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        execute: bool,
    },
    #[command(about = "Valide les bloqueurs et avertissements avant implementation.")]
    Preflight {
        #[arg(long)]
        workspace: String,
        #[arg(long = "ai-context-file")]
        ai_context_file: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Synchronise task.json avec les work items Azure DevOps.")]
    Sync {
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"])]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Renomme un workspace task et sa branche selon un nouveau slug.")]
    Rename {
        slug: String,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"])]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        execute: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Met les repositories du workspace a jour depuis leur branche cible.")]
    RepoLatest {
        #[arg(long, conflicts_with = "continue")]
        workspace: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long = "only")]
        only: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(
        about = "Prepare ou cree un commit intermediaire pour les repositories du workspace."
    )]
    Commit {
        #[arg(long, conflicts_with = "continue")]
        workspace: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Ajoute des work items au workspace task courant.")]
    AddWorkItem {
        work_item_ids: String,
        #[arg(long, conflicts_with_all = ["project", "work_item", "continue"])]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long = "skip-ado")]
        skip_ado: bool,
        #[arg(long = "type")]
        type_name: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Retire des work items du workspace task courant.")]
    RemoveWorkItem {
        work_item_ids: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue")]
        r#continue: bool,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Ajoute un repository au workspace task.")]
    AddRepo {
        repo: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Cree une tache enfant ADO et l'ajoute au handoff repository.")]
    CreateChildTask {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue")]
        r#continue: bool,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Verifie, commit, push et ouvre une PR pour terminer le workspace.")]
    Finish {
        #[arg(long, conflicts_with = "continue")]
        workspace: Option<String>,
        #[arg(long = "continue", conflicts_with = "workspace")]
        r#continue: bool,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        message: Option<String>,
        #[arg(long = "create-pr")]
        create_pr: bool,
        #[arg(long, requires = "create_pr")]
        ready: bool,
        #[arg(long = "skip-verify")]
        skip_verify: bool,
        #[arg(long = "skip-ado")]
        skip_ado: bool,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Valide les fichiers handoff avant sous-agents ou finition.")]
    HandoffValidate {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        json: bool,
    },
    #[command(about = "Supprime les worktrees et nettoie un workspace task.")]
    Teardown {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue")]
        r#continue: bool,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    #[command(about = "Nettoie les workspaces dont les work items sont termines.")]
    Prune {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long = "no-sync")]
        no_sync: bool,
        #[arg(long)]
        json: bool,
    },
}

pub fn handle_task(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Status { root } => crate::open::status(root),
        TaskCommand::List {
            root,
            project,
            work_item,
            json,
        } => crate::open::list(root, project, work_item, json)?,
        TaskCommand::Current { json } => crate::open::current(json)?,
        TaskCommand::Open {
            workspace,
            project,
            work_item,
            positional_work_item,
            r#continue,
            repo,
            agent,
            json,
            root,
        } => crate::open::open_workspace(crate::open::OpenWorkspaceArgs {
            workspace,
            project,
            work_item,
            positional_work_item,
            r#continue,
            repo,
            agent,
            json,
            root,
        })?,
        TaskCommand::Start {
            work_item_id,
            root,
            project,
            task,
            type_name,
            only,
            slug,
            skip_ado,
            json,
            execute,
        } => crate::start::handle(crate::start::StartArgs {
            work_item_id,
            root,
            project,
            task,
            type_name,
            only,
            slug,
            skip_ado,
            json,
            execute,
        })?,
        TaskCommand::Preflight {
            workspace,
            ai_context_file,
            json,
        } => crate::validate::preflight(workspace, ai_context_file, json)?,
        TaskCommand::Sync {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        } => crate::lifecycle::sync(crate::lifecycle::SyncArgs {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        })?,
        TaskCommand::Rename {
            slug,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            json,
            execute,
            positional_work_item,
        } => crate::lifecycle::rename(crate::lifecycle::RenameArgs {
            slug,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            json,
            execute,
            positional_work_item,
        })?,
        TaskCommand::RepoLatest {
            workspace,
            r#continue,
            only,
            root,
            json,
        } => crate::repo::repo_latest(crate::repo::RepoLatestArgs {
            workspace,
            r#continue,
            only,
            root,
            json,
        })?,
        TaskCommand::Commit {
            workspace,
            r#continue,
            root,
            execute,
            message,
            json,
        } => crate::repo::commit(crate::repo::CommitArgs {
            workspace,
            r#continue,
            root,
            execute,
            message,
            json,
        })?,
        TaskCommand::AddWorkItem {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            skip_ado,
            type_name,
            title,
            state,
            execute,
            json,
        } => crate::work_item::add(crate::work_item::AddWorkItemArgs {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            skip_ado,
            type_name,
            title,
            state,
            execute,
            json,
        })?,
        TaskCommand::RemoveWorkItem {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            json,
        } => crate::work_item::remove(crate::work_item::RemoveWorkItemArgs {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            json,
        })?,
        TaskCommand::AddRepo {
            repo,
            workspace,
            root,
            execute,
            json,
        } => crate::repo::add_repo(crate::repo::AddRepoArgs {
            repo,
            workspace,
            root,
            execute,
            json,
        })?,
        TaskCommand::CreateChildTask {
            repo,
            title,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        } => crate::lifecycle::create_child_task(crate::lifecycle::CreateChildTaskArgs {
            repo,
            title,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        })?,
        TaskCommand::Finish {
            workspace,
            r#continue,
            root,
            execute,
            message,
            create_pr,
            ready,
            skip_verify,
            skip_ado,
            json,
        } => crate::finish::handle(crate::finish::FinishArgs {
            workspace,
            r#continue,
            root,
            execute,
            message,
            create_pr,
            ready,
            skip_verify,
            skip_ado,
            json,
        })?,
        TaskCommand::HandoffValidate { workspace, json } => {
            crate::validate::handoff_validate(workspace, json)?
        }
        TaskCommand::Teardown {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            yes,
            json,
        } => crate::repo::teardown(crate::repo::TeardownArgs {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            yes,
            json,
        })?,
        TaskCommand::Prune {
            root,
            project,
            work_item,
            execute,
            yes,
            no_sync,
            json,
        } => crate::prune::handle(crate::prune::PruneArgs {
            root,
            project,
            work_item,
            execute,
            yes,
            no_sync,
            json,
        })?,
    }

    Ok(())
}

pub fn handle_agent_open(args: OpenAgentArgs) -> Result<()> {
    open_workspace(OpenWorkspaceArgs {
        workspace: args.workspace,
        project: args.project,
        work_item: args.work_item,
        positional_work_item: args.positional_work_item,
        r#continue: args.r#continue,
        repo: args.repo,
        agent: args.agent,
        json: false,
        root: args.root,
    })
}
