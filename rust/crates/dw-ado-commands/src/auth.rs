use anyhow::Result;
use clap::Subcommand;
use dw_ado::auth::{AdoToken, login_browser_interactive, login_device_code, logout, status};
use dw_ui::TerminalTheme;
use inquire::Select;

use crate::load_auth_options;

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    Login {
        #[arg(long)]
        root: Option<String>,
    },
    Status {
        #[arg(long)]
        root: Option<String>,
    },
    Logout {
        #[arg(long)]
        root: Option<String>,
    },
}

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

pub fn handle_auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login { root } => {
            let auth = load_auth_options(root.as_deref())?;
            match prompt_auth_login_mode()?.mode {
                AuthLoginMode::Browser => print_auth_token(login_browser_interactive(auth)?)?,
                AuthLoginMode::DeviceCode => print_auth_token(login_device_code(auth)?)?,
                AuthLoginMode::EnvironmentPat => {
                    print_styled(
                        "Définir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT dans l'environnement.",
                    );
                    print_styled("Aucun secret n'est saisi ni stocké par cette commande.");
                }
            }
        }
        AuthCommand::Status { root } => {
            let auth = load_auth_options(root.as_deref())?;
            let status = status(auth)?;
            if status.connected {
                print_styled(&format!(
                    "Connecté via {}.",
                    status.source.unwrap_or_default()
                ));
                print_styled(
                    &status
                        .expires_on
                        .map(|value| format!("Expire le {value}."))
                        .unwrap_or_else(|| "Expiration inconnue.".into()),
                );
            } else {
                print_styled("Non connecté.");
                print_styled("Exécuter dw auth login ou définir DW_ADO_TOKEN.");
                std::process::exit(1);
            }
        }
        AuthCommand::Logout { root } => {
            let _ = load_auth_options(root.as_deref())?;
            let removed = logout()?;
            print_styled(&format!(
                "Sessions MSAL supprimées: {}.",
                usize::from(removed)
            ));
            print_styled(
                "Les PAT définis via DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent gérés par l'environnement.",
            );
        }
    }
    Ok(())
}

fn prompt_auth_login_mode() -> Result<AuthLoginChoice> {
    Ok(Select::new("Mode de connexion Azure DevOps", auth_login_choices()).prompt()?)
}

fn auth_login_choices() -> Vec<AuthLoginChoice> {
    vec![
        AuthLoginChoice {
            label: "Navigateur automatique (recommandé)",
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
    ]
}

fn print_auth_token(token: AdoToken) -> Result<()> {
    print_styled(&format!("Connecté via {}.", token.source));
    print_styled(
        &token
            .expires_on
            .map(|value| format!("Expire le {value}."))
            .unwrap_or_else(|| "Expiration inconnue.".into()),
    );
    Ok(())
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_login_choices_keep_browser_first() {
        let choices = auth_login_choices();

        assert_eq!(choices[0].label, "Navigateur automatique (recommandé)");
        assert!(matches!(choices[0].mode, AuthLoginMode::Browser));
    }
}
