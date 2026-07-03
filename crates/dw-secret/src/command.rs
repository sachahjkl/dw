use anyhow::Result;
use clap::Subcommand;
use dw_ui::TerminalTheme;
use inquire::{Password, PasswordDisplayMode};
use std::io::IsTerminal;

use crate::{KeyringSecretStore, delete_secret, secret_exists, secret_from_env, store_secret};

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    #[command(about = "Enregistre un secret dans le keyring système.")]
    Set {
        #[arg(help = "Clé logique du secret, par exemple une credentialReference.")]
        key: String,
        #[arg(
            long,
            conflicts_with = "from_env",
            help = "Valeur du secret à enregistrer."
        )]
        value: Option<String>,
        #[arg(
            long = "from-env",
            conflicts_with = "value",
            help = "Nom de variable d'environnement contenant le secret."
        )]
        from_env: Option<String>,
    },
    #[command(about = "Vérifie si un secret existe sans afficher sa valeur.")]
    Get {
        #[arg(help = "Clé logique du secret à vérifier.")]
        key: String,
    },
    #[command(about = "Supprime un secret du keyring système.")]
    Delete {
        #[arg(help = "Clé logique du secret à supprimer.")]
        key: String,
    },
}

pub fn handle_secret(command: SecretCommand) -> Result<()> {
    let store = KeyringSecretStore;
    match command {
        SecretCommand::Set {
            key,
            value,
            from_env,
        } => {
            let secret = match (value, from_env) {
                (Some(secret), None) => secret,
                (None, Some(name)) => secret_from_env(&name)?,
                (None, None) if std::io::stdin().is_terminal() => Password::new("Secret")
                    .with_display_mode(PasswordDisplayMode::Hidden)
                    .without_confirmation()
                    .prompt()?,
                (None, None) => {
                    return Err(anyhow::anyhow!(
                        "secret set requiert --value ou --from-env en mode non interactif"
                    ));
                }
                (Some(_), Some(_)) => unreachable!("clap rejects --value with --from-env"),
            };
            store_secret(&store, &key, &secret)?;
            print_styled_lines(&secret_set_lines(&key));
        }
        SecretCommand::Get { key } => {
            print_styled_lines(&secret_get_lines(&key, secret_exists(&store, &key)?));
        }
        SecretCommand::Delete { key } => {
            delete_secret(&store, &key)?;
            print_styled_lines(&secret_delete_lines(&key));
        }
    }
    Ok(())
}

fn secret_set_lines(key: &str) -> Vec<String> {
    vec![
        "Secret".into(),
        "Statut    : enregistré".into(),
        format!("Clé       : {key}"),
        "Stockage  : keyring système".into(),
        "Valeur    : masquée".into(),
    ]
}

fn secret_get_lines(key: &str, exists: bool) -> Vec<String> {
    vec![
        "Secret".into(),
        format!(
            "Statut    : {}",
            if exists { "présent" } else { "introuvable" }
        ),
        format!("Clé       : {key}"),
        "Valeur    : masquée".into(),
    ]
}

fn secret_delete_lines(key: &str) -> Vec<String> {
    vec![
        "Secret".into(),
        "Statut    : supprimé si présent".into(),
        format!("Clé       : {key}"),
    ]
}

fn print_styled_lines(lines: &[String]) {
    let theme = TerminalTheme::stdout_auto();
    for line in lines {
        println!("{}", theme.style_line(line, false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_set_lines_never_include_secret_value() {
        let lines = secret_set_lines("db/password");

        assert_eq!(lines[0], "Secret");
        assert_eq!(lines[1], "Statut    : enregistré");
        assert_eq!(lines[2], "Clé       : db/password");
        assert_eq!(lines[3], "Stockage  : keyring système");
        assert_eq!(lines[4], "Valeur    : masquée");
        assert!(!lines.join("\n").contains("password-value"));
    }

    #[test]
    fn secret_get_lines_render_presence_without_value() {
        let present = secret_get_lines("db/password", true);
        let missing = secret_get_lines("db/password", false);

        assert_eq!(present[1], "Statut    : présent");
        assert_eq!(missing[1], "Statut    : introuvable");
        assert!(present.contains(&"Valeur    : masquée".into()));
    }

    #[test]
    fn secret_delete_lines_render_key() {
        let lines = secret_delete_lines("db/password");

        assert_eq!(lines[0], "Secret");
        assert_eq!(lines[1], "Statut    : supprimé si présent");
        assert_eq!(lines[2], "Clé       : db/password");
    }
}
