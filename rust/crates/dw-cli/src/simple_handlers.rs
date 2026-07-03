use anyhow::Result;

use crate::cli::CompletionCommand;
use crate::completion::{
    generate_completion, print_completion_complete, print_completion_install, print_completion_show,
};
use crate::upgrade;

pub(crate) fn handle_completion(command: CompletionCommand) -> Result<()> {
    match command {
        CompletionCommand::Show => print_completion_show(),
        CompletionCommand::Generate { shell } => generate_completion(shell),
        CompletionCommand::Install { shell } => print_completion_install(shell),
        CompletionCommand::Complete { format, words } => print_completion_complete(format, words)?,
    }
    Ok(())
}

pub(crate) fn handle_upgrade(check: bool, rid: Option<String>) -> Result<()> {
    upgrade::handle_upgrade(check, rid)
}
