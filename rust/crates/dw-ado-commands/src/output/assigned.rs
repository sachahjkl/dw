use dw_ado::{WorkItemGroup, WorkItemSnapshot};
use dw_ui::TerminalTheme;

pub fn empty_assigned_message(include_final_states: bool) -> &'static str {
    if include_final_states {
        "Aucun work item assigné."
    } else {
        "Aucun work item assigné hors états finaux."
    }
}

pub fn render_assigned_items(
    items: &[WorkItemSnapshot],
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let mut lines = vec![theme.success(&format!("Work items assignés ({})", items.len()))];
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
    let mut lines = vec![theme.success(&format!("Work items assignés ({})", groups.len()))];
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

fn format_work_item_summary(item: &WorkItemSnapshot, theme: &TerminalTheme) -> String {
    format!(
        "{} [{}] {} - {}",
        theme.success(&format!("#{}", item.id)),
        item.kind.as_deref().unwrap_or("inconnu"),
        item.state.as_deref().unwrap_or("inconnu"),
        item.title.as_deref().unwrap_or("inconnu")
    )
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

        assert!(output.contains("Work items assignés (1)"));
        assert!(output.contains("dw task start 42 --project ha"));
    }

    #[test]
    fn grouped_assigned_items_deduplicate_suggested_start_ids() {
        let parent = WorkItemSnapshot {
            id: "42".into(),
            kind: Some("User Story".into()),
            state: Some("Actif".into()),
            title: Some("Parent".into()),
            url: None,
        };
        let children = vec![
            WorkItemSnapshot {
                id: "42".into(),
                kind: Some("Task".into()),
                state: Some("Actif".into()),
                title: Some("Doublon".into()),
                url: None,
            },
            WorkItemSnapshot {
                id: "43".into(),
                kind: Some("Task".into()),
                state: Some("Actif".into()),
                title: Some("Enfant".into()),
                url: None,
            },
        ];

        assert_eq!(suggested_start_ids(&parent, &children), "42,43");
    }
}
