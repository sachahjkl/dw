use crate::{SqlGuardResult, query::QueryResult};
use dw_ui::TerminalTheme;

const MAX_CELL_WIDTH: usize = 48;

pub fn render_query_result_tsv(result: &QueryResult) -> String {
    let mut lines = vec![result.columns.join("\t")];
    lines.extend(result.rows.iter().map(|row| {
        row.iter()
            .map(|value| value.clone().unwrap_or_else(|| "NULL".into()))
            .collect::<Vec<_>>()
            .join("\t")
    }));
    lines.push(if result.truncated {
        format!("-- {} rows (truncated)", result.rows.len())
    } else {
        format!("-- {} rows", result.rows.len())
    });
    lines.join("\n")
}

pub fn render_query_result_table(result: &QueryResult, theme: &TerminalTheme) -> String {
    let columns = if result.columns.is_empty() {
        vec!["Result".to_string()]
    } else {
        result.columns.clone()
    };
    let rows = if result.columns.is_empty() && result.rows.is_empty() {
        Vec::new()
    } else {
        result.rows.clone()
    };
    let widths = column_widths(&columns, &rows);
    let mut lines = Vec::new();

    lines.push(theme.bold(&theme.cyan("DB query")));
    lines.push(format!(
        "Résultat  : {}",
        theme.bold(&row_count_label(result))
    ));
    lines.push(render_separator(&widths));
    lines.push(render_row(
        &columns
            .iter()
            .map(|column| Some(column.as_str()))
            .collect::<Vec<_>>(),
        &widths,
        Some(theme),
    ));
    lines.push(render_separator(&widths));
    lines.extend(rows.iter().map(|row| {
        let cells = row
            .iter()
            .map(|value| value.as_deref())
            .collect::<Vec<Option<&str>>>();
        render_row(&cells, &widths, None)
    }));
    lines.push(render_separator(&widths));
    if result.truncated {
        lines.push(theme.warning(&format!(
            "Résultat tronqué après {} ligne(s). Relancer avec --max-rows pour élargir.",
            result.rows.len()
        )));
    }
    lines.join("\n")
}

pub fn render_sql_guard(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    let mut lines = vec![theme.bold(&theme.cyan("DB guard"))];
    lines.push(format!("Statut    : {}", status_label(result, theme)));
    if result.is_allowed {
        lines.push(format!("Décision  : {}", theme.success("✓")));
        lines.push("Message   : Requête autorisée en lecture seule.".into());
        lines.push(format!(
            "Détail    : {}",
            theme.dim("Aucune exécution n'a été lancée par cette commande.")
        ));
    } else {
        lines.push(format!("Décision  : {}", theme.error("!")));
        lines.push("Message   : Requête bloquée avant exécution.".into());
        lines.push(format!(
            "Raison    : {}",
            result.reason.as_deref().unwrap_or("raison inconnue")
        ));
        lines.push(format!(
            "À faire   : {}",
            theme.warning("Utiliser uniquement SELECT/WITH ou les commandes d'introspection.")
        ));
    }
    lines.join("\n")
}

fn row_count_label(result: &QueryResult) -> String {
    let suffix = if result.rows.len() > 1 { "s" } else { "" };
    if result.truncated {
        format!(
            "{} ligne{suffix} affichée{suffix}, résultat tronqué",
            result.rows.len()
        )
    } else {
        format!("{} ligne{suffix}", result.rows.len())
    }
}

fn status_label(result: &SqlGuardResult, theme: &TerminalTheme) -> String {
    if result.is_allowed {
        theme.success("autorisé")
    } else {
        theme.error("bloqué")
    }
}

fn column_widths(columns: &[String], rows: &[Vec<Option<String>>]) -> Vec<usize> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let row_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(|value| display_cell(value.as_deref()).chars().count())
                .max()
                .unwrap_or(0);
            column
                .chars()
                .count()
                .max(row_width)
                .clamp(1, MAX_CELL_WIDTH)
        })
        .collect()
}

fn render_separator(widths: &[usize]) -> String {
    let cells = widths
        .iter()
        .map(|width| "-".repeat(width + 2))
        .collect::<Vec<_>>();
    format!("+{}+", cells.join("+"))
}

fn render_row(cells: &[Option<&str>], widths: &[usize], theme: Option<&TerminalTheme>) -> String {
    let rendered = widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            let value = display_cell(cells.get(index).copied().flatten());
            let value = truncate_cell(&value, *width);
            let padded = format!(" {value:<width$} ");
            if let Some(theme) = theme {
                theme.bold(&padded)
            } else if cells.get(index).copied().flatten().is_none() {
                TerminalTheme::plain().dim(&padded)
            } else {
                padded
            }
        })
        .collect::<Vec<_>>();
    format!("|{}|", rendered.join("|"))
}

fn display_cell(value: Option<&str>) -> String {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or("NULL")
        .replace(['\n', '\r'], " ")
}

fn truncate_cell(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.into();
    }
    let take = width.saturating_sub(1);
    format!("{}…", value.chars().take(take).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tsv_with_null_and_truncation() {
        let result = QueryResult {
            columns: vec!["Id".into(), "Name".into()],
            rows: vec![vec![Some("1".into()), None]],
            truncated: true,
        };

        assert_eq!(
            render_query_result_tsv(&result),
            "Id\tName\n1\tNULL\n-- 1 rows (truncated)"
        );
    }

    #[test]
    fn renders_terminal_table_with_summary_and_truncation_hint() {
        let result = QueryResult {
            columns: vec!["Id".into(), "Name".into()],
            rows: vec![vec![Some("1".into()), None]],
            truncated: true,
        };

        let output = render_query_result_table(&result, &TerminalTheme::plain());

        assert!(output.contains("DB query"));
        assert!(output.contains("Résultat  : 1 ligne affichée, résultat tronqué"));
        assert!(output.contains("| Id | Name |"));
        assert!(output.contains("| 1  | NULL |"));
        assert!(output.contains("Résultat tronqué après 1 ligne(s)."));
    }

    #[test]
    fn terminal_table_truncates_wide_cells() {
        let result = QueryResult {
            columns: vec!["Value".into()],
            rows: vec![vec![Some("x".repeat(80))]],
            truncated: false,
        };

        let output = render_query_result_table(&result, &TerminalTheme::plain());

        assert!(output.contains('…'));
    }

    #[test]
    fn renders_allowed_sql_guard_with_status_and_hint() {
        let output = render_sql_guard(
            &SqlGuardResult {
                is_allowed: true,
                reason: None,
            },
            &TerminalTheme::plain(),
        );

        assert!(output.contains("DB guard"));
        assert!(output.contains("Statut    : autorisé"));
        assert!(output.contains("Décision  : ✓"));
        assert!(output.contains("Message   : Requête autorisée en lecture seule."));
        assert!(output.contains("Aucune exécution n'a été lancée"));
    }

    #[test]
    fn renders_blocked_sql_guard_with_reason_and_remediation() {
        let output = render_sql_guard(
            &SqlGuardResult {
                is_allowed: false,
                reason: Some("Mot-clé SQL interdit en mode read-only: DROP.".into()),
            },
            &TerminalTheme::plain(),
        );

        assert!(output.contains("DB guard"));
        assert!(output.contains("Statut    : bloqué"));
        assert!(output.contains("Décision  : !"));
        assert!(output.contains("Message   : Requête bloquée avant exécution."));
        assert!(output.contains("Raison    : Mot-clé SQL interdit"));
        assert!(output.contains("Utiliser uniquement SELECT/WITH"));
    }
}
