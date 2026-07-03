use anyhow::Result;
use clap::Subcommand;
use inquire::{Password, PasswordDisplayMode};
use std::io::IsTerminal;

use crate::{KeyringSecretStore, delete_secret, secret_exists, secret_from_env, store_secret};

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    Set {
        key: String,
        #[arg(long, conflicts_with = "from_env")]
        value: Option<String>,
        #[arg(long = "from-env", conflicts_with = "value")]
        from_env: Option<String>,
    },
    Get {
        key: String,
    },
    Delete {
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
            println!("Secret enregistre dans le keyring systeme.");
        }
        SecretCommand::Get { key } => {
            if secret_exists(&store, &key)? {
                println!("Secret present.");
            } else {
                println!("Secret introuvable.");
            }
        }
        SecretCommand::Delete { key } => {
            delete_secret(&store, &key)?;
            println!("Secret supprime si present.");
        }
    }
    Ok(())
}
