use crate::cli::*;
use crate::completion::{
    generate_completion, print_completion_complete, print_completion_install, print_completion_show,
};
use crate::doctor::run_doctor;
use crate::version::informational_version;
use anyhow::Result;
use dw_agent::command::AgentAction;
use dw_ui::TerminalTheme;

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            println!("Dev Workflow {}", informational_version());
        }
        Command::Guide => {
            print_styled_lines(&render_guide(
                &informational_version(),
                &TerminalTheme::stdout_auto(),
            ));
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
        Command::Agent { command } => match dw_agent::command::handle_agent(command)? {
            AgentAction::Handled => {}
            AgentAction::Open(args) => dw_task::command::handle_agent_open(args)?,
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

fn print_styled_lines(lines: &[String]) {
    for line in lines {
        print_styled(line);
    }
}

fn render_guide(version: &str, theme: &TerminalTheme) -> Vec<String> {
    vec![
        theme.command(&format!("Dev Workflow {version}")),
        "Parcours recommandé".into(),
        format!("  1. {}", theme.command("dw init")),
        format!("  2. {}", theme.command("dw doctor")),
        format!("  3. {}", theme.command("dw task start <work-item-id>")),
        String::new(),
        "Commandes utiles".into(),
        format!("  - {}", theme.command("dw ado assigned")),
        format!("  - {}", theme.command("dw task current")),
        format!("  - {}", theme.command("dw completion show")),
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_renders_version_and_next_steps() {
        let lines = render_guide("2026.07.02.3+54011f0", &TerminalTheme::plain());

        assert_eq!(lines[0], "Dev Workflow 2026.07.02.3+54011f0");
        assert!(lines.contains(&"Parcours recommandé".into()));
        assert!(lines.iter().any(|line| line.contains("dw init")));
        assert!(lines.iter().any(|line| line.contains("dw doctor")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("dw task start <work-item-id>"))
        );
        assert!(lines.iter().any(|line| line.contains("dw ado assigned")));
        assert!(lines.iter().any(|line| line.contains("dw completion show")));
    }
}
