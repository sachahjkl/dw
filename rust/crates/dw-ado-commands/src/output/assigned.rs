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
    let mut lines = vec![
        theme.success("ADO assignés"),
        format!("Items     : {}", items.len()),
    ];
    for item in items {
        lines.push(String::new());
        lines.push(format!(
            "Item      : {}",
            format_work_item_summary(item, theme)
        ));
        lines.push(start_command_line(&item.id, project, theme));
        lines.push(String::new());
    }
    trim_trailing_blank_line(lines).join("\n")
}

pub fn render_assigned_groups(
    groups: &[WorkItemGroup],
    project: &str,
    theme: &TerminalTheme,
) -> String {
    let total_items = groups
        .iter()
        .map(|group| 1 + group.items.len())
        .sum::<usize>();
    let mut lines = vec![theme.success(&format!(
        "Work items assignés: {} groupe(s), {} item(s)",
        groups.len(),
        total_items
    ))];
    for group in groups {
        lines.push(String::new());
        lines.push(format!(
            "Parent    : {}",
            format_work_item_summary(&group.parent, theme)
        ));
        if !group.items.is_empty() {
            lines.push(start_command_line(
                &suggested_start_ids(&group.parent, &group.items),
                project,
                theme,
            ));
        }
        for item in &group.items {
            lines.push(format!(
                "  Enfant  : {}",
                format_work_item_summary(item, theme)
            ));
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

fn start_command_line(ids: &str, project: &str, theme: &TerminalTheme) -> String {
    format!(
        "Démarrer  : {}",
        theme.command(&format!("dw task start {ids} --project {project}"))
    )
}

fn format_work_item_summary(item: &WorkItemSnapshot, theme: &TerminalTheme) -> String {
    format!(
        "{} {} {}",
        theme.success(&format!("#{}", item.id)),
        theme.dim(&format!(
            "[{} / {}]",
            item.kind.as_deref().unwrap_or("type inconnu"),
            item.state.as_deref().unwrap_or("état inconnu")
        )),
        item.title.as_deref().unwrap_or("(sans titre)")
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

        assert!(output.contains("ADO assignés"));
        assert!(output.contains("Items     : 1"));
        assert!(output.contains("Item      : #42 [Bug / En developpement] Corriger"));
        assert!(output.contains("Démarrer  : dw task start 42 --project ha"));
    }

    #[test]
    fn grouped_assigned_items_render_parent_children_and_start_command() {
        let parent = WorkItemSnapshot {
            id: "42".into(),
            kind: Some("User Story".into()),
            state: Some("Actif".into()),
            title: Some("Parent".into()),
            url: None,
        };
        let child = WorkItemSnapshot {
            id: "43".into(),
            kind: Some("Task".into()),
            state: Some("Actif".into()),
            title: Some("Enfant".into()),
            url: None,
        };

        let output = render_assigned_groups(
            &[WorkItemGroup {
                parent,
                items: vec![child],
            }],
            "ha",
            &TerminalTheme::plain(),
        );

        assert!(output.contains("Work items assignés: 1 groupe(s), 2 item(s)"));
        assert!(output.contains("Parent    : #42 [User Story / Actif] Parent"));
        assert!(output.contains("Démarrer  : dw task start 42,43 --project ha"));
        assert!(output.contains("  Enfant  : #43 [Task / Actif] Enfant"));
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
