use anyhow::Result;
use inquire::{Confirm, MultiSelect};
use std::io::IsTerminal;

pub(crate) fn confirm_or_require_flag(flag: &str, prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "Confirmation interactive indisponible: ajouter {flag}."
        ));
    }

    Ok(Confirm::new(prompt)
        .with_default(false)
        .with_help_message("Répondre oui pour continuer.")
        .prompt()?)
}

pub(crate) fn confirm_when_interactive(prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        return Ok(true);
    }

    Ok(Confirm::new(prompt)
        .with_default(false)
        .with_help_message("Répondre oui pour continuer.")
        .prompt()?)
}

pub(crate) fn multiselect_or_require_flag(
    flag: &str,
    prompt: &str,
    options: Vec<String>,
) -> Result<Vec<String>> {
    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "Sélection interactive indisponible: ajouter {flag}."
        ));
    }

    Ok(MultiSelect::new(prompt, options)
        .with_help_message("Espace pour sélectionner, Entrée pour valider.")
        .prompt()?)
}
