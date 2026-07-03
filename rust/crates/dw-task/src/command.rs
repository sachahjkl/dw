use anyhow::Result;
use clap::Subcommand;

pub use crate::open::{OpenWorkspaceArgs, open_workspace};

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    Status {
        #[arg(long)]
        root: Option<String>,
    },
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
    Current {
        #[arg(long)]
        json: bool,
    },
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
    Preflight {
        #[arg(long)]
        workspace: String,
        #[arg(long = "ai-context-file")]
        ai_context_file: Vec<String>,
        #[arg(long)]
        json: bool,
    },
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
    HandoffValidate {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        json: bool,
    },
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
