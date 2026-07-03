use std::io::IsTerminal;

pub fn banner(title: &str) -> String {
    format!("== {} ==", title)
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
            &[
                "Dry-run",
                "Relancer",
                "Teardown dry-run",
                "PR non créée",
                "Teardown annulé",
                "Prévisualisation",
            ],
        ) {
            return self.warning(&styled);
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
                "Upgrade",
                "Préparation de l'upgrade",
                "Schémas et contextes agents régénérés",
                "Config DevWorkflow",
                "Doctor config",
                "Config mise à jour",
                "Task workspaces",
                "Task sync",
                "Task rename",
                "Task prune",
                "Work items task",
                "Workspace courant",
                "Repo latest",
                "Commit workspace",
                "Ajout repo",
                "Teardown",
                "Finish workspace",
                "Handoff validation",
                "Handoff ",
                "Commit à créer",
                "Pull requests",
                "Preflight task",
                "Détails preflight",
                "Validation handoff",
                "Détails handoff",
                "ADO assignés",
                "ADO work item",
                "ADO context",
                "ADO auth",
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
            "Repo ajouté",
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
