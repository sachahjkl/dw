mod assigned;
mod changelog;
mod context;
mod project;
mod work_item;

use crate::cli::AdoCommand;
use anyhow::Result;

pub(crate) use project::{project_choices, resolve_ado_options, resolve_project_key_or_prompt};

pub(crate) fn handle_ado(command: AdoCommand) -> Result<()> {
    match command {
        AdoCommand::Assigned {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        } => assigned::handle(assigned::AssignedArgs {
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
        } => changelog::handle(changelog::ChangelogArgs {
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
        } => work_item::handle(work_item::WorkItemArgs {
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
        } => context::handle_context(context::ContextArgs {
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
        } => context::handle_ai_context(context::AiContextArgs {
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
