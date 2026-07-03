use crate::cli::*;
use crate::completion::{
    generate_completion, print_completion_complete, print_completion_install, print_completion_show,
};
use crate::doctor::{run_agent_doctor, run_doctor};
use crate::version::informational_version;
use anyhow::Result;
use dw_agent::agent_context;
use dw_config::{default_agent, resolve_root, set_default_agent};
use dw_ui::TerminalTheme;

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            println!("dw {}", informational_version());
        }
        Command::Guide => {
            print_styled(&format!("dw - Dev Workflow {}", informational_version()));
            print_styled("Demarrer avec `dw init`, puis `dw task start <work-item-id>`.");
        }
        Command::Doctor { fix } => run_doctor(fix)?,
        Command::Init {
            profile,
            root,
            dry_run,
            no_save,
        } => dw_config::command::handle_init(dw_config::command::InitCommandArgs {
            root,
            profile,
            no_save,
            dry_run,
        })?,
        Command::Refresh { root, profile } => {
            dw_config::command::handle_refresh(dw_config::command::RefreshCommandArgs {
                root,
                profile,
            })?
        }
        Command::Agent { command } => match command {
            AgentCommand::Context => {
                let root = resolve_root(None);
                println!("{}", agent_context(&root));
            }
            AgentCommand::Open {
                workspace,
                project,
                work_item,
                positional_work_item,
                r#continue,
                repo,
                agent,
                root,
            } => dw_task::command::open_workspace(dw_task::command::OpenWorkspaceArgs {
                workspace,
                project,
                work_item,
                positional_work_item,
                r#continue,
                repo,
                agent,
                json: false,
                root,
            })?,
            AgentCommand::Config { root } | AgentCommand::Show { root } => {
                let root = resolve_root(root.as_deref());
                print_styled(&format!("Agent par defaut: {}", default_agent(&root)));
            }
            AgentCommand::SetDefault { root, agent } => {
                let root = resolve_root(root.as_deref());
                let agent = set_default_agent(&root, &agent)?;
                print_styled(&format!("Agent par defaut: {agent}"));
            }
            AgentCommand::Doctor { agent } => run_agent_doctor(agent.as_deref())?,
        },
        Command::Auth { command } => dw_ado_commands::auth::handle_auth(command)?,
        Command::Completion { command } => handle_completion(command)?,
        Command::Config { command } => dw_config::command::handle_config(command)?,

        Command::Ado { command } => dw_ado_commands::command::handle_ado(command)?,
        Command::Db { command } => dw_db::command::handle_db(command)?,
        Command::Secret { command } => dw_secret::command::handle_secret(command)?,
        Command::Upgrade { check, rid } => crate::upgrade::handle_upgrade(check, rid)?,
        Command::Task { command } => dw_task::command::handle_task(command)?,
    }

    Ok(())
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

fn handle_completion(command: CompletionCommand) -> Result<()> {
    match command {
        CompletionCommand::Show => print_completion_show(),
        CompletionCommand::Generate { shell } => generate_completion(shell),
        CompletionCommand::Install { shell } => print_completion_install(shell),
        CompletionCommand::Complete { format, words } => print_completion_complete(format, words)?,
    }
    Ok(())
}
