use anyhow::Result;
use clap::Subcommand;
use dw_ado::auth::{
    AdoAuthStatus, AdoToken, login_browser_interactive, login_device_code, logout, status,
};
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
                AuthLoginMode::Browser => print_auth_token(login_browser_interactive(auth)?),
                AuthLoginMode::DeviceCode => print_auth_token(login_device_code(auth)?),
                AuthLoginMode::EnvironmentPat => {
                    print_styled_lines(&environment_pat_lines());
                }
            }
        }
        AuthCommand::Status { root } => {
            let auth = load_auth_options(root.as_deref())?;
            let status = status(auth)?;
            if status.connected {
                print_styled_lines(&auth_status_lines(&status));
            } else {
                print_styled_lines(&auth_status_lines(&status));
                std::process::exit(1);
            }
        }
        AuthCommand::Logout { root } => {
            let _ = load_auth_options(root.as_deref())?;
            let removed = logout()?;
            print_styled_lines(&logout_lines(removed));
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

fn print_auth_token(token: AdoToken) {
    print_styled_lines(&auth_token_lines(&token));
}

fn auth_token_lines(token: &AdoToken) -> Vec<String> {
    vec![
        "Connexion ADO".into(),
        "Statut    : connecté".into(),
        format!("Source    : {}", token.source),
        expiration_line(token.expires_on.as_deref()),
    ]
}

fn auth_status_lines(status: &AdoAuthStatus) -> Vec<String> {
    if status.connected {
        vec![
            "Connexion ADO".into(),
            "Statut    : connecté".into(),
            format!(
                "Source    : {}",
                status.source.as_deref().unwrap_or("source inconnue")
            ),
            expiration_line(status.expires_on.as_deref()),
        ]
    } else {
        vec![
            "Connexion ADO".into(),
            "Statut    : non connecté".into(),
            "À faire   : dw auth login ou définir DW_ADO_TOKEN.".into(),
        ]
    }
}

fn environment_pat_lines() -> Vec<String> {
    vec![
        "Connexion ADO".into(),
        "Mode      : PAT via environnement".into(),
        "À faire   : définir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT.".into(),
        "Sécurité : aucun secret n'est saisi ni stocké par cette commande.".into(),
    ]
}

fn logout_lines(removed: bool) -> Vec<String> {
    vec![
        "Connexion ADO".into(),
        format!(
            "Sessions  : {}",
            if removed {
                "MSAL supprimées"
            } else {
                "aucune session MSAL locale"
            }
        ),
        "PAT       : les variables DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent gérées par l'environnement.".into(),
    ]
}

fn expiration_line(expires_on: Option<&str>) -> String {
    format!(
        "Expiration: {}",
        expires_on.unwrap_or("expiration inconnue")
    )
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
}

fn print_styled_lines(lines: &[String]) {
    for line in lines {
        print_styled(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_ado::auth::AdoAuthScheme;

    #[test]
    fn auth_login_choices_keep_browser_first() {
        let choices = auth_login_choices();

        assert_eq!(choices[0].label, "Navigateur automatique (recommandé)");
        assert!(matches!(choices[0].mode, AuthLoginMode::Browser));
    }

    #[test]
    fn auth_token_lines_render_connected_source_and_expiration() {
        let lines = auth_token_lines(&AdoToken {
            access_token: "secret".into(),
            source: "MSAL keyring".into(),
            scheme: AdoAuthScheme::Bearer,
            expires_on: Some("2026-07-03T12:14:07Z".into()),
        });

        assert_eq!(lines[0], "Connexion ADO");
        assert_eq!(lines[1], "Statut    : connecté");
        assert_eq!(lines[2], "Source    : MSAL keyring");
        assert_eq!(lines[3], "Expiration: 2026-07-03T12:14:07Z");
    }

    #[test]
    fn auth_status_lines_render_disconnected_action() {
        let lines = auth_status_lines(&AdoAuthStatus {
            connected: false,
            source: None,
            expires_on: None,
        });

        assert_eq!(lines[0], "Connexion ADO");
        assert_eq!(lines[1], "Statut    : non connecté");
        assert_eq!(
            lines[2],
            "À faire   : dw auth login ou définir DW_ADO_TOKEN."
        );
    }

    #[test]
    fn logout_lines_keep_pat_warning() {
        let lines = logout_lines(true);

        assert_eq!(lines[0], "Connexion ADO");
        assert_eq!(lines[1], "Sessions  : MSAL supprimées");
        assert!(lines[2].contains("DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT restent gérées"));
    }
}
