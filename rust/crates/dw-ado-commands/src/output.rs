use dw_ado::{WorkItemGroup, WorkItemSnapshot};
use dw_contracts::AdoAiContextItem;
use dw_ui::TerminalTheme;

pub fn terminal_theme() -> TerminalTheme {
    TerminalTheme::stdout_auto()
}

pub fn empty_assigned_message(include_final_states: bool) -> &'static str {
    if include_final_states {
        "Aucun work item assigne."
    } else {
        "Aucun work item assigne hors etats finaux."
    }
}

pub fn render_assigned_items(
    items: &[WorkItemSnapshot],
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let mut lines = vec![theme.success(&format!("Work items assignes ({})", items.len()))];
    for item in items {
        lines.push(format_work_item_summary(item, theme));
        lines.push(format!(
            "  {} {}",
            theme.command("Start:"),
            theme.command(&format!("dw task start {} --project {}", item.id, project))
        ));
    }
    lines.join("\n")
}

pub fn render_assigned_groups(
    groups: &[WorkItemGroup],
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let mut lines = vec![theme.success(&format!("Work items assignes ({})", groups.len()))];
    for group in groups {
        lines.push(format_work_item_summary(&group.parent, theme));
        if !group.items.is_empty() {
            lines.push(format!(
                "  {} {}",
                theme.command("Start:"),
                theme.command(&format!(
                    "dw task start {} --project {}",
                    suggested_start_ids(&group.parent, &group.items),
                    project
                ))
            ));
        }
        for item in &group.items {
            lines.push(format!("  - {}", format_work_item_summary(item, theme)));
        }
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines).join("\n")
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

fn format_work_item_summary(item: &WorkItemSnapshot, theme: &TerminalTheme) -> String {
    format!(
        "{} [{}] {} - {}",
        theme.success(&format!("#{}", item.id)),
        item.kind.as_deref().unwrap_or("inconnu"),
        item.state.as_deref().unwrap_or("inconnu"),
        item.title.as_deref().unwrap_or("inconnu")
    )
}

pub(crate) fn suggested_start_ids(
    parent: &WorkItemSnapshot,
    children: &[WorkItemSnapshot],
) -> String {
    let mut ids = vec![parent.id.clone()];
    for child in children {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(&child.id)) {
            ids.push(child.id.clone());
        }
    }
    ids.join(",")
}

fn trim_trailing_blank_line(mut lines: Vec<String>) -> Vec<String> {
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_items_render_start_command() {
        let output = render_assigned_items(
            &[WorkItemSnapshot {
                id: "42".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Corriger".into()),
                url: None,
            }],
            "ha",
            &TerminalTheme::plain(),
        );

        assert!(output.contains("Work items assignes (1)"));
        assert!(output.contains("dw task start 42 --project ha"));
    }

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
