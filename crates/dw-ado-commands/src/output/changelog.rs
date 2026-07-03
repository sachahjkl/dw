use dw_ado::ChangelogFormat;
use dw_ui::TerminalTheme;

pub fn render_changelog_source_empty(from_git: bool, theme: &TerminalTheme) -> String {
    theme.warning(if from_git {
        "Aucun work item détecté dans les messages de commit de la plage git."
    } else {
        "Aucun work item détecté pour les pull requests données."
    })
}

pub fn render_changelog_resolved_empty(theme: &TerminalTheme) -> String {
    theme.warning("Aucun work item résolu dans Azure DevOps.")
}

pub fn render_changelog_ids(ids: &[String]) -> String {
    ids.join(" ")
}

pub fn render_changelog_document(
    document: &str,
    format: ChangelogFormat,
    theme: &TerminalTheme,
) -> String {
    match format {
        ChangelogFormat::Raw => document
            .lines()
            .map(|line| render_raw_changelog_line(line, theme))
            .collect::<Vec<_>>()
            .join("\n"),
        ChangelogFormat::Markdown | ChangelogFormat::Html => document.into(),
    }
}

fn render_raw_changelog_line(line: &str, theme: &TerminalTheme) -> String {
    let Some(hash_index) = line.find('#') else {
        return theme.style_line(line, false);
    };
    let id_end = line[hash_index + 1..]
        .char_indices()
        .find_map(|(index, character)| {
            (!character.is_ascii_digit()).then_some(hash_index + 1 + index)
        })
        .unwrap_or(line.len());

    if id_end == hash_index + 1 {
        return theme.style_line(line, false);
    }

    let prefix = &line[..hash_index];
    let id = &line[hash_index..id_end];
    let suffix = &line[id_end..];
    format!("{prefix}{}{}", theme.success(id), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_ids_remain_script_friendly() {
        let output = render_changelog_ids(&["42".into(), "43".into()]);

        assert_eq!(output, "42 43");
    }

    #[test]
    fn raw_changelog_styles_work_item_ids_only() {
        let theme = TerminalTheme::new(dw_ui::ColorMode::Always, false, false);
        let output = render_changelog_document(
            "#42 [Bug] Actif - Corriger\n  - #43 [Task] Actif - Tester",
            ChangelogFormat::Raw,
            &theme,
        );

        assert!(output.contains("\u{1b}"));
        assert!(output.contains("[Bug] Actif - Corriger"));
        assert!(output.contains("  - "));
    }

    #[test]
    fn markdown_changelog_is_not_colored() {
        let theme = TerminalTheme::new(dw_ui::ColorMode::Always, false, false);
        let markdown = "# Changelog\n\n- [#42](https://example.invalid)";

        let output = render_changelog_document(markdown, ChangelogFormat::Markdown, &theme);

        assert_eq!(output, markdown);
    }

    #[test]
    fn empty_changelog_messages_are_accented() {
        let theme = TerminalTheme::plain();

        assert_eq!(
            render_changelog_source_empty(true, &theme),
            "Aucun work item détecté dans les messages de commit de la plage git."
        );
        assert_eq!(
            render_changelog_source_empty(false, &theme),
            "Aucun work item détecté pour les pull requests données."
        );
        assert_eq!(
            render_changelog_resolved_empty(&theme),
            "Aucun work item résolu dans Azure DevOps."
        );
    }
}
