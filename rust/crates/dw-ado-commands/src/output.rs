use dw_ado::{WorkItemGroup, WorkItemSnapshot};
use dw_ui::{ColorMode, TerminalTheme};
use std::io::IsTerminal;

pub fn terminal_theme() -> TerminalTheme {
    TerminalTheme::new(
        ColorMode::Auto,
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
    )
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
}
