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

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            println!("dw {}", informational_version());
        }
        Command::Guide => {
            println!(
                "dw - Dev Workflow {}\nDemarrer avec `dw init`, puis `dw task start <work-item-id>`.",
                informational_version()
            );
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
                println!("Dry-run init DevWorkflow: {}", report.root);
                println!("Profil: {}", report.profile);
                for path in &report.planned_paths {
                    println!("  would create/write: {path}");
                }
                if report.no_save {
                    println!("  would not modify user settings (--no-save).");
                } else {
                    println!("  would save user root: {}", report.root);
                }
            } else {
                println!("Root DevWorkflow initialise: {}", report.root);
                println!("Profil: {}", report.profile);
                if report.no_save {
                    println!("Settings utilisateur non modifies (--no-save).");
                }
                println!("Prochaine etape conseillee: dw doctor");
            }
        }
        Command::Refresh { root, profile } => {
            let root = resolve_root(root.as_deref());
            let report = refresh_root(RefreshRequest {
                root,
                profile: Some(profile),
            })?;
            println!("Root rafraichi: {}", report.root);
            println!("Profil: {}", report.profile);
            println!("Schemas et contextes agents regeneres.");
            println!(
                "Fichiers utilisateurs preserves: projects.json, workflow.json, databases.json, plan.md."
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
            } => crate::task::open_workspace(crate::task::OpenWorkspaceArgs {
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
                println!("Agent par defaut: {}", default_agent(&root));
            }
            AgentCommand::SetDefault { root, agent } => {
                let root = resolve_root(root.as_deref());
                let agent = set_default_agent(&root, &agent)?;
                println!("Agent par defaut: {agent}");
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
        Command::Task { command } => crate::task::handle_task(command)?,
    }

    Ok(())
}
