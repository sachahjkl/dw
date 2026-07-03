use std::io::IsTerminal;

use anyhow::Result;
use inquire::{Confirm, MultiSelect, Select};

pub fn banner(title: &str) -> String {
    format!("== {} ==", title)
}

pub fn is_stdin_interactive() -> bool {
    std::io::stdin().is_terminal()
}

pub fn confirm_or_require_flag(flag: &str, prompt: &str) -> Result<bool> {
    if !is_stdin_interactive() {
        return Err(anyhow::anyhow!(
            "Confirmation interactive indisponible: ajouter {flag}."
        ));
    }

    Ok(Confirm::new(prompt)
        .with_default(false)
        .with_help_message("Répondre oui pour continuer.")
        .prompt()?)
}

pub fn confirm_destructive_or_require_flag(
    confirmed: bool,
    flag: &str,
    prompt: &str,
) -> Result<bool> {
    if confirmed {
        return Ok(true);
    }
    confirm_or_require_flag(flag, prompt)
}

pub fn confirm_when_interactive(prompt: &str) -> Result<bool> {
    if !is_stdin_interactive() {
        return Ok(true);
    }

    Ok(Confirm::new(prompt)
        .with_default(false)
        .with_help_message("Répondre oui pour continuer.")
        .prompt()?)
}

pub fn multiselect_or_require_flag(
    flag: &str,
    prompt: &str,
    options: Vec<String>,
) -> Result<Vec<String>> {
    if !is_stdin_interactive() {
        return Err(anyhow::anyhow!(
            "Sélection interactive indisponible: ajouter {flag}."
        ));
    }

    Ok(MultiSelect::new(prompt, options)
        .with_help_message("Espace pour sélectionner, Entrée pour valider.")
        .prompt()?)
}

pub fn multiselect_optional(prompt: &str, options: Vec<String>) -> Result<Option<Vec<String>>> {
    if !is_stdin_interactive() || options.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        MultiSelect::new(prompt, options)
            .with_help_message("Espace pour sélectionner, Entrée pour valider.")
            .prompt()?,
    ))
}

pub fn select_optional(prompt: &str, options: Vec<String>) -> Result<Option<String>> {
    if !is_stdin_interactive() || options.is_empty() {
        return Ok(None);
    }

    Ok(Some(Select::new(prompt, options).prompt()?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalTheme {
    enabled: bool,
}

impl TerminalTheme {
    pub fn stdout(mode: ColorMode) -> Self {
        Self::new(
            mode,
            std::io::stdout().is_terminal(),
            std::env::var_os("NO_COLOR").is_some(),
        )
    }

    pub fn stdout_auto() -> Self {
        Self::stdout(ColorMode::Auto)
    }

    pub fn new(mode: ColorMode, is_terminal: bool, no_color: bool) -> Self {
        let enabled = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_terminal && !no_color,
        };
        Self { enabled }
    }

    pub fn plain() -> Self {
        Self { enabled: false }
    }

    pub fn success(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Green.on_default().bold(), text)
    }

    pub fn warning(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Yellow.on_default().bold(), text)
    }

    pub fn error(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Red.on_default().bold(), text)
    }

    pub fn path(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Cyan.on_default(), text)
    }

    pub fn command(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Magenta.on_default(), text)
    }

    pub fn cyan(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Cyan.on_default(), text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(anstyle::Effects::DIMMED.into(), text)
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(anstyle::Effects::BOLD.into(), text)
    }

    pub fn style_line(&self, line: &str, is_error: bool) -> String {
        if line.is_empty() || is_json_like(line) {
            return line.into();
        }

        if is_error || line.starts_with_ignore_ascii_case("Erreur") {
            return self.bold(&self.error(line));
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            return self.bold(&self.cyan(line));
        }

        let styled = line
            .replace(": Done", &format!(": {}", self.bold(&self.success("Done"))))
            .replace("Done:", &format!("{}:", self.bold(&self.success("Done"))));

        if starts_with_any_ignore_ascii_case(
            &styled,
            &["Dry-run", "Relancer", "PR non créée", "Prévisualisation"],
        ) {
            return self.warning(&styled);
        }

        if let Some(styled_key_value) = self.style_known_key_value(&styled) {
            return styled_key_value;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let indent_length = line.len() - trimmed.len();
            let indent = &line[..indent_length];
            return format!("{indent}{}{}", self.dim(&trimmed[..2]), &trimmed[2..]);
        }

        if let Some(separator_index) = styled.find(':')
            && separator_index > 0
            && separator_index <= 40
        {
            let label = &styled[..separator_index];
            let suffix = &styled[separator_index..];
            if !label.contains("//") && !label.contains('\\') {
                return format!("{}{}", self.bold(&self.cyan(label)), suffix);
            }
        }

        if is_success_status_line(&styled) {
            return self.success(&styled);
        }

        if starts_with_any_ignore_ascii_case(
            &styled,
            &["Aucun", "Sync ignorée", "PR ignorée", "ADO ignore"],
        ) {
            return self.warning(&styled);
        }

        if starts_with_any_ignore_ascii_case(
            &styled,
            &[
                "Prochaine étape",
                "Puis, pour",
                "Et pour terminer",
                "Workspaces disponibles",
                "Project  WorkItem",
                "Mise à jour",
                "Préparation de la mise à jour",
                "Schémas et contextes agents régénérés",
                "Configuration DevWorkflow",
                "Diagnostic configuration",
                "Configuration mise à jour",
                "Workspaces task",
                "Synchronisation task",
                "Renommage workspace",
                "Nettoyage workspaces",
                "Work items workspace",
                "Workspace courant",
                "Mise à jour repositories",
                "Commit des repositories",
                "Ajout repository",
                "Suppression workspace",
                "Finalisation workspace",
                "Handoff ",
                "Commit à créer",
                "Pull requests à créer",
                "Préflight task",
                "Détails préflight",
                "Validation handoff",
                "Détails handoff",
                "ADO assignés",
                "ADO work item",
                "ADO context",
                "Connexion ADO",
                "Requête DB",
                "Garde SQL",
                "Sous-tâche ADO",
                "Secret",
            ],
        ) {
            return self.bold(&self.cyan(&styled));
        }

        if trimmed.starts_with_ignore_ascii_case("dw ") {
            return self.bold(&styled);
        }

        styled
    }

    fn style_known_key_value(&self, line: &str) -> Option<String> {
        let separator_index = line.find(':')?;
        if separator_index == 0 || separator_index > 40 {
            return None;
        }

        let label = &line[..separator_index];
        let label_name = label.trim();
        if !matches!(label_name, "Statut" | "Résultat" | "Décision" | "À faire") {
            return None;
        }

        let suffix = &line[separator_index + 1..];
        let value_start = suffix.len() - suffix.trim_start().len();
        let value_padding = &suffix[..value_start];
        let value = &suffix[value_start..];
        let styled_label = self.bold(&self.cyan(label));
        let styled_value = self.style_known_value(label_name, value);

        Some(format!("{styled_label}:{value_padding}{styled_value}"))
    }

    fn style_known_value(&self, label_name: &str, value: &str) -> String {
        match label_name {
            "À faire" => {
                if value.trim_start().starts_with_ignore_ascii_case("dw ") {
                    self.bold(&self.command(value))
                } else {
                    self.warning(value)
                }
            }
            "Décision" => {
                if value.contains('✓') {
                    self.success(value)
                } else if value.contains('!') || value.contains('✕') {
                    self.error(value)
                } else {
                    self.bold(value)
                }
            }
            "Statut" | "Résultat" => self.style_status_value(value),
            _ => value.into(),
        }
    }

    fn style_status_value(&self, value: &str) -> String {
        let normalized = value.to_lowercase();
        if contains_any(
            &normalized,
            &[
                "non connecté",
                "à corriger",
                "bloqué",
                "introuvable",
                "incomplète",
                "erreur",
                "échec",
            ],
        ) {
            self.error(value)
        } else if contains_any(
            &normalized,
            &["changement", "tronqué", "ignoré", "ignorée", "à faire"],
        ) {
            self.warning(value)
        } else if contains_any(
            &normalized,
            &[
                "connecté",
                "terminé",
                "valide",
                "autorisé",
                "enregistré",
                "présent",
                "supprimé",
                "ok",
                "done",
            ],
        ) {
            self.success(value)
        } else {
            value.into()
        }
    }

    fn paint(&self, style: anstyle::Style, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", style.render(), text, style.render_reset())
        } else {
            text.into()
        }
    }
}

fn is_json_like(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[') || trimmed.starts_with('"')
}

fn is_success_status_line(line: &str) -> bool {
    starts_with_any_ignore_ascii_case(
        line,
        &[
            "Workspace créé",
            "Worktree créé",
            "Workspace renommé",
            "Workspace synchronisé",
            "Workspace supprimé",
            "Repository ajouté",
            "Work items ajoutés",
            "Work items retirés",
            "Binaire remplacé",
            "Commits/push terminés",
            "PR créée",
            "Root rafraîchi",
            "Workspace mis à jour",
        ],
    ) || (line.starts_with_ignore_ascii_case("Repo ") && line.contains(':'))
}

fn starts_with_any_ignore_ascii_case(value: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| value.starts_with_ignore_ascii_case(prefix))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

trait StartsWithIgnoreAsciiCase {
    fn starts_with_ignore_ascii_case(&self, prefix: &str) -> bool;
}

impl StartsWithIgnoreAsciiCase for str {
    fn starts_with_ignore_ascii_case(&self, prefix: &str) -> bool {
        self.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, TerminalTheme};

    #[test]
    fn never_keeps_plain_text() {
        let theme = TerminalTheme::new(ColorMode::Never, true, false);
        assert_eq!(theme.success("OK"), "OK");
    }

    #[test]
    fn auto_disables_color_when_no_color_is_set() {
        let theme = TerminalTheme::new(ColorMode::Auto, true, true);
        assert_eq!(theme.warning("WARN"), "WARN");
    }

    #[test]
    fn always_emits_ansi() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        assert!(theme.error("ERR").contains("\u{1b}"));
    }

    #[test]
    fn style_line_colors_status_lines() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        let styled = theme.style_line("Workspace créé: S:/dw", false);

        assert!(styled.contains("\u{1b}"));
        assert!(styled.contains("Workspace créé"));
    }

    #[test]
    fn style_line_colors_known_status_values() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        let ok = theme.style_line("Statut    : connecté", false);
        let blocked = theme.style_line("Statut    : bloqué", false);

        assert!(ok.contains("\u{1b}[1m\u{1b}[32mconnecté"));
        assert!(blocked.contains("\u{1b}[1m\u{1b}[31mbloqué"));
    }

    #[test]
    fn style_line_colors_action_commands() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        let styled = theme.style_line("À faire   : dw task commit --execute", false);

        assert!(styled.contains("\u{1b}[1m\u{1b}[35mdw task commit --execute"));
    }

    #[test]
    fn style_line_keeps_known_key_values_plain_without_color() {
        let theme = TerminalTheme::plain();

        assert_eq!(
            theme.style_line("À faire   : dw task commit --execute", false),
            "À faire   : dw task commit --execute"
        );
        assert_eq!(
            theme.style_line("Statut    : bloqué", false),
            "Statut    : bloqué"
        );
    }

    #[test]
    fn style_line_preserves_json_lines() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);

        assert_eq!(
            theme.style_line(r#"{"schema":1}"#, false),
            r#"{"schema":1}"#
        );
    }

    #[test]
    fn style_line_keeps_plain_when_color_disabled() {
        let theme = TerminalTheme::plain();

        assert_eq!(
            theme.style_line("Workspace créé: S:/dw", false),
            "Workspace créé: S:/dw"
        );
    }

    #[test]
    fn stdout_auto_is_constructible() {
        let theme = TerminalTheme::stdout_auto();

        assert_eq!(
            theme.style_line(r#"{"schema":1}"#, false),
            r#"{"schema":1}"#
        );
    }
}
