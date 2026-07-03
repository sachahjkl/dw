use crate::cli::AdoCommand;
use anyhow::Result;

pub(crate) fn handle_ado(command: AdoCommand) -> Result<()> {
    match command {
        AdoCommand::Assigned {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        } => dw_ado_commands::commands::assigned::handle(
            dw_ado_commands::commands::assigned::AssignedArgs {
                root,
                project,
                top,
                all,
                group_by_parent,
                json,
            },
        )?,
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
        } => dw_ado_commands::commands::changelog::handle(
            dw_ado_commands::commands::changelog::ChangelogArgs {
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
            },
        )?,
        AdoCommand::WorkItem {
            id,
            root,
            project,
            json,
        } => dw_ado_commands::commands::work_item::handle(
            dw_ado_commands::commands::work_item::WorkItemArgs {
                id,
                root,
                project,
                json,
            },
        )?,
        AdoCommand::Context {
            id,
            root,
            project,
            summary,
            comments,
            json,
        } => dw_ado_commands::commands::context::handle_context(
            dw_ado_commands::commands::context::ContextArgs {
                id,
                root,
                project,
                summary,
                comments,
                json,
            },
        )?,
        AdoCommand::AiContext {
            root,
            organization,
            project,
            id,
            summary,
            comments: _,
            include_comments,
        } => dw_ado_commands::commands::context::handle_ai_context(
            dw_ado_commands::commands::context::AiContextArgs {
                root,
                organization,
                project,
                id,
                summary,
                include_comments,
            },
        )?,
    }

    Ok(())
}
