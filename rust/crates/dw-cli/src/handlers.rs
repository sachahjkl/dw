use crate::cli::*;
use crate::doctor::{run_agent_doctor, run_doctor};
use crate::simple_handlers::{
    handle_auth, handle_completion, handle_config, handle_secret, handle_upgrade,
};
use crate::version::informational_version;
use anyhow::Result;
use dw_agent::agent_context;
use dw_config::{
    InitRequest, RefreshRequest, default_agent, init_root, refresh_root, resolve_root,
    set_default_agent,
};
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
        } => {
            let report = init_root(InitRequest {
                root,
                profile,
                no_save,
                dry_run,
            })?;
            if report.dry_run {
                print_styled(&format!("Dry-run init DevWorkflow: {}", report.root));
                print_styled(&format!("Profil: {}", report.profile));
                for path in &report.planned_paths {
                    print_styled(&format!("  would create/write: {path}"));
                }
                if report.no_save {
                    print_styled("  would not modify user settings (--no-save).");
                } else {
                    print_styled(&format!("  would save user root: {}", report.root));
                }
            } else {
                print_styled(&format!("Root DevWorkflow initialise: {}", report.root));
                print_styled(&format!("Profil: {}", report.profile));
                if report.no_save {
                    print_styled("Settings utilisateur non modifies (--no-save).");
                }
                print_styled("Prochaine etape conseillee: dw doctor");
            }
        }
        Command::Refresh { root, profile } => {
            let root = resolve_root(root.as_deref());
            let report = refresh_root(RefreshRequest {
                root,
                profile: Some(profile),
            })?;
            print_styled(&format!("Root rafraichi: {}", report.root));
            print_styled(&format!("Profil: {}", report.profile));
            print_styled("Schemas et contextes agents regeneres.");
            print_styled(
                "Fichiers utilisateurs preserves: projects.json, workflow.json, databases.json, plan.md.",
            );
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
        Command::Auth { command } => handle_auth(command)?,
        Command::Completion { command } => handle_completion(command)?,
        Command::Config { command } => handle_config(command)?,

        Command::Ado { command } => crate::ado::handle_ado(command)?,
        Command::Db { command } => crate::db::handle_db(command)?,
        Command::Secret { command } => handle_secret(command)?,
        Command::Upgrade { check, rid } => handle_upgrade(check, rid)?,
        Command::Task { command } => dw_task::command::handle_task(command)?,
    }

    Ok(())
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}
