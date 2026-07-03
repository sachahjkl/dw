use dw_ado::WorkItemSnapshot;
use dw_ui::TerminalTheme;

mod assigned;
mod changelog;
mod context;

pub(crate) use assigned::suggested_start_ids;
pub use assigned::{empty_assigned_message, render_assigned_groups, render_assigned_items};
pub use changelog::{
    render_changelog_document, render_changelog_ids, render_changelog_resolved_empty,
    render_changelog_source_empty,
};
pub use context::render_context_items;

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
        lines.push(format_work_item_snapshot(item, theme));
        lines.push(String::new());
        lines.push(format!(
            "Contexte complet: {}",
            theme.command(&format!("dw ado context {} --project {}", item.id, project))
        ));
    }
    lines.join("\n")
}

fn format_work_item_snapshot(item: &WorkItemSnapshot, theme: &TerminalTheme) -> String {
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

        assert!(output.contains("#7 [type inconnu / état inconnu] (sans titre)"));
        assert!(output.contains("Contexte complet: dw ado context 7 --project ha"));
    }
}
