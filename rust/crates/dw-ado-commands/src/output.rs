use dw_ado::{ChangelogFormat, WorkItemSnapshot};
use dw_contracts::AdoAiContextItem;
use dw_ui::TerminalTheme;

mod assigned;

pub(crate) use assigned::suggested_start_ids;
pub use assigned::{empty_assigned_message, render_assigned_groups, render_assigned_items};

pub fn terminal_theme() -> TerminalTheme {
    TerminalTheme::stdout_auto()
}

pub fn render_work_item_snapshots(
    items: &[WorkItemSnapshot],
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let mut lines = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("---".into());
            lines.push(String::new());
        }
        lines.push(theme.success(&format!("#{}", item.id)));
        lines.push(format!(
            "Type: {}",
            item.kind.as_deref().unwrap_or("inconnu")
        ));
        lines.push(format!(
            "Etat: {}",
            item.state.as_deref().unwrap_or("inconnu")
        ));
        lines.push(format!(
            "Titre: {}",
            item.title.as_deref().unwrap_or("inconnu")
        ));
        lines.push(String::new());
        lines.push(format!(
            "Contexte complet: {}",
            theme.command(&format!("dw ado context {} --project {}", item.id, project))
        ));
    }
    lines.join("\n")
}

pub fn render_changelog_source_empty(from_git: bool, theme: &TerminalTheme) -> String {
    theme.warning(if from_git {
        "Aucun work item detecte dans les messages de commit de la plage git."
    } else {
        "Aucun work item detecte pour les pull requests donnees."
    })
}

pub fn render_changelog_resolved_empty(theme: &TerminalTheme) -> String {
    theme.warning("Aucun work item resolu dans Azure DevOps.")
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

pub fn render_context_items(
    items: &[AdoAiContextItem],
    comment_limit: i32,
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let mut lines = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
            lines.push("---".into());
            lines.push(String::new());
        }

        lines.push(theme.success(&format!("#{}", item.work_item.id)));
        lines.push(format!(
            "Type: {}",
            item.work_item.kind.as_deref().unwrap_or("inconnu")
        ));
        lines.push(format!(
            "Etat: {}",
            item.work_item.state.as_deref().unwrap_or("inconnu")
        ));
        lines.push(format!(
            "Titre: {}",
            item.work_item.title.as_deref().unwrap_or("inconnu")
        ));
        lines.push(format!(
            "Assigne a: {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("non assigne")
        ));

        if let Some(description) = &item.content.description
            && !description.trim().is_empty()
        {
            lines.push(String::new());
            lines.push(theme.bold("Description:"));
            lines.push(description.trim().into());
        }

        if !item.relations.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold("Relations:"));
            for relation in &item.relations {
                lines.push(format!(
                    "- {} {}",
                    relation.kind,
                    relation
                        .work_item_id
                        .as_deref()
                        .or(relation.url.as_deref())
                        .unwrap_or("")
                ));
            }
        }

        if comment_limit != 0 && !item.comments.is_empty() {
            lines.push(String::new());
            lines.push(theme.bold("Commentaires:"));
            for comment in item.comments.iter().take(comment_limit.max(0) as usize) {
                lines.push(format!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("inconnu"),
                    comment.text.as_deref().unwrap_or("").trim()
                ));
            }
        }

        lines.push(String::new());
        lines.push(format!(
            "AI context: {}",
            theme.command(&format!(
                "dw ado ai-context {} --project {}",
                item.work_item.id, project
            ))
        ));
    }
    lines.join("\n")
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
    fn work_item_render_keeps_context_command() {
        let output = render_work_item_snapshots(
            &[WorkItemSnapshot {
                id: "7".into(),
                kind: None,
                state: None,
                title: None,
                url: None,
            }],
            "ha",
            &TerminalTheme::plain(),
        );

        assert!(output.contains("Contexte complet: dw ado context 7 --project ha"));
    }

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
    fn context_render_includes_relations_comments_and_ai_context_command() {
        let output = render_context_items(
            &[AdoAiContextItem {
                schema_version: dw_contracts::AI_CONTEXT_VERSION.into(),
                work_item: dw_contracts::AdoAiContextWorkItem {
                    id: "42".into(),
                    url: None,
                    title: Some("Corriger".into()),
                    kind: Some("Bug".into()),
                    state: Some("Actif".into()),
                    assigned_to: Some("Sacha".into()),
                    area_path: None,
                    iteration_path: None,
                    tags: vec![],
                },
                core: dw_contracts::AdoAiContextCore {
                    created_by: None,
                    created_date: None,
                    changed_by: None,
                    changed_date: None,
                    priority: None,
                    value_area: None,
                },
                content: dw_contracts::AdoAiContextContent {
                    description: Some("Description courte".into()),
                    acceptance_criteria: None,
                    product_context: Default::default(),
                },
                links: dw_contracts::AdoAiContextLinks {
                    parent_ids: vec![],
                    child_ids: vec![],
                    predecessor_ids: vec![],
                    successor_ids: vec![],
                },
                attachments: dw_contracts::AdoAiContextAttachments {
                    directory_hint: "attachments/ado/42".into(),
                    items: vec![],
                },
                relations: vec![dw_contracts::AdoAiContextRelation {
                    kind: "Parent".into(),
                    rel: None,
                    work_item_id: Some("1".into()),
                    name: None,
                    url: None,
                    comment: None,
                    artifact: None,
                    display: "Parent #1".into(),
                }],
                comments: vec![dw_contracts::AdoAiContextComment {
                    author: Some("Bob".into()),
                    created_date: None,
                    text: Some("OK".into()),
                }],
            }],
            10,
            "ha",
            &TerminalTheme::plain(),
        );

        assert!(output.contains("#42"));
        assert!(output.contains("Description courte"));
        assert!(output.contains("- Parent 1"));
        assert!(output.contains("- Bob: OK"));
        assert!(output.contains("dw ado ai-context 42 --project ha"));
    }
}
