use anyhow::Result;

use crate::cli::{AuthCommand, CompletionCommand, ConfigCommand, SecretCommand};
use crate::completion::{
    generate_completion, print_completion_complete, print_completion_install, print_completion_show,
};
use crate::support::{flag_value, unsupported_command_with_args};
use dw_ado::auth::{AdoAuthOptions, login_browser_interactive, login_device_code, logout, status};
use dw_config::{
    config_doctor, config_show, load_workflow_config, resolve_root, set_color_mode, set_user_root,
};
use inquire::Select;

#[derive(Debug, Clone, Copy)]
enum AuthLoginMode {
    Browser,
    DeviceCode,
    EnvironmentPat,
}

#[derive(Debug, Clone)]
struct AuthLoginChoice {
    label: &'static str,
    mode: AuthLoginMode,
}

impl std::fmt::Display for AuthLoginChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label)
    }
}

pub(crate) fn handle_completion(command: CompletionCommand) -> Result<()> {
    match command {
        CompletionCommand::Show => print_completion_show(),
        CompletionCommand::Generate { shell } => generate_completion(shell),
        CompletionCommand::Install { shell } => print_completion_install(shell),
        CompletionCommand::Complete { format, words } => print_completion_complete(format, words)?,
    }
    Ok(())
}

pub(crate) fn handle_config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show { root, json } => {
            let report = config_show(root.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Root: {}", report.root);
                println!("Color: {}", report.color);
            }
        }
        ConfigCommand::Doctor { root, json } => {
            let report = config_doctor(root.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for check in &report.checks {
                    println!(
                        "{} {}",
                        if check.passed { "[OK]  " } else { "[WARN]" },
                        check.path
                    );
                    if let Some(message) = &check.message {
                        println!("      {message}");
                    }
                }
            }
            if !report.passed {
                std::process::exit(1);
            }
        }
        ConfigCommand::SetRoot { path } => println!("Root: {}", set_user_root(&path)?),
        ConfigCommand::SetColor { mode } => {
            println!("Color: {}", set_color_mode(&mode)?);
        }
    }
    Ok(())
}

pub(crate) fn handle_auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login { root } => {
            let auth = load_auth_options(root.as_deref())?;
            match prompt_auth_login_mode()?.mode {
                AuthLoginMode::Browser => print_auth_token(login_browser_interactive(auth)?)?,
                AuthLoginMode::DeviceCode => print_auth_token(login_device_code(auth)?)?,
                AuthLoginMode::EnvironmentPat => {
                    println!("Definir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT dans l'environnement.");
                    println!("Aucun secret n'est saisi ni stocke par cette commande.");
                }
            }
        }
        AuthCommand::Status { root } => {
            let auth = load_auth_options(root.as_deref())?;
            let status = status(auth)?;
            if status.connected {
                println!("Connecte via {}.", status.source.unwrap_or_default());
                println!(
                    "{}",
                    status
                        .expires_on
                        .map(|value| format!("Expire le {value}."))
                        .unwrap_or_else(|| "Expiration inconnue.".into())
                );
            } else {
                println!("Non connecte.");
                println!("Executer dw auth login ou definir DW_ADO_TOKEN.");
                std::process::exit(1);
            }
        }
        AuthCommand::Logout { root } => {
            let _ = load_auth_options(root.as_deref())?;
            let removed = logout()?;
            println!("Sessions MSAL supprimees: {}.", usize::from(removed));
            println!(
                "Les PAT definis via DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent geres par l'environnement."
            );
        }
    }
    Ok(())
}

fn prompt_auth_login_mode() -> Result<AuthLoginChoice> {
    let options = vec![
        AuthLoginChoice {
            label: "Navigateur automatique (recommande)",
            mode: AuthLoginMode::Browser,
        },
        AuthLoginChoice {
            label: "Code appareil manuel",
            mode: AuthLoginMode::DeviceCode,
        },
        AuthLoginChoice {
            label: "PAT via variable d'environnement",
            mode: AuthLoginMode::EnvironmentPat,
        },
    ];
    Ok(Select::new("Mode de connexion Azure DevOps", options).prompt()?)
}

fn print_auth_token(token: dw_ado::auth::AdoToken) -> Result<()> {
    println!("Connecte via {}.", token.source);
    println!(
        "{}",
        token
            .expires_on
            .map(|value| format!("Expire le {value}."))
            .unwrap_or_else(|| "Expiration inconnue.".into())
    );
    Ok(())
}

pub(crate) fn load_auth_options(root: Option<&str>) -> Result<Option<AdoAuthOptions>> {
    let root = resolve_root(root);
    let workflow = load_workflow_config(&root);
    workflow
        .auth
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

pub(crate) fn handle_secret(command: SecretCommand) -> Result<()> {
    match command {
        SecretCommand::Set {
            key,
            value,
            from_env,
        } => unsupported_command_with_args(
            "secret set",
            &[
                ("key", Some(key.as_str())),
                ("value", value.as_deref()),
                ("from-env", from_env.as_deref()),
            ],
        )?,
        SecretCommand::Get { key } => {
            unsupported_command_with_args("secret get", &[("key", Some(key.as_str()))])?
        }
        SecretCommand::Delete { key } => {
            unsupported_command_with_args("secret delete", &[("key", Some(key.as_str()))])?
        }
    }
    Ok(())
}

pub(crate) fn handle_upgrade(check: bool, rid: Option<String>) -> Result<()> {
    unsupported_command_with_args(
        "upgrade",
        &[("check", flag_value(check)), ("rid", rid.as_deref())],
    )
}
