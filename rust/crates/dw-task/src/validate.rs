use anyhow::Result;
use dw_config::resolve_root;
use dw_contracts::{TaskHandoffValidationReport, TaskPreflightReport};
use dw_workspace::{
    build_handoff_validation_report, build_preflight_report_from_ai_context_files,
    resolve_workspace,
};
use std::path::Path;

use crate::render::print_styled_lines;

#[derive(Debug, Clone)]
pub struct PreflightArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub ai_context_file: Vec<String>,
    pub json: bool,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HandoffValidateArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub json: bool,
    pub positional_work_item: Option<String>,
}

pub fn preflight(args: PreflightArgs) -> Result<()> {
    let PreflightArgs {
        workspace,
        root,
        project,
        work_item,
        r#continue,
        ai_context_file,
        json,
        positional_work_item,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace(
        &root,
        workspace.as_deref(),
        project.as_deref(),
        work_item.as_deref(),
        positional_work_item.as_deref(),
        r#continue,
    )?;
    let files = if ai_context_file.is_empty() {
        discover_ai_context_files(&workspace)
    } else {
        ai_context_file
    };

    if files.is_empty() {
        return Err(anyhow::anyhow!(
            "Aucun fichier ai-context détecté. Fournir --ai-context-file ou placer des fichiers ai-context*.json dans le workspace."
        ));
    }

    let report = build_preflight_report_from_ai_context_files(&workspace, &files)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_styled_lines(&preflight_lines(&report));
    }
    Ok(())
}

pub fn handoff_validate(args: HandoffValidateArgs) -> Result<()> {
    let HandoffValidateArgs {
        workspace,
        root,
        project,
        work_item,
        r#continue,
        json,
        positional_work_item,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace(
        &root,
        workspace.as_deref(),
        project.as_deref(),
        work_item.as_deref(),
        positional_work_item.as_deref(),
        r#continue,
    )?;
    let report = build_handoff_validation_report(&workspace)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_styled_lines(&handoff_validation_lines(&report));
    }
    Ok(())
}

fn preflight_lines(report: &TaskPreflightReport) -> Vec<String> {
    let mut lines = vec![
        "Préflight task".into(),
        format!(
            "Statut    : {}",
            validation_status_label(!report.has_blocking_issues)
        ),
        format!("Workspace : {}", report.workspace),
        format!("Projet    : {}", report.project),
        format!(
            "Work items: {}",
            report
                .work_item_ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        String::new(),
    ];

    if report.issues.is_empty() {
        lines.push("✓ Aucun avertissement ni blocage détecté.".into());
        return lines;
    }

    let blocking_count = report
        .issues
        .iter()
        .filter(|issue| is_blocking_severity(&issue.severity))
        .count();
    let warning_count = report
        .issues
        .iter()
        .filter(|issue| is_warning_severity(&issue.severity))
        .count();
    let other_count = report
        .issues
        .len()
        .saturating_sub(blocking_count + warning_count);
    lines.push(format!(
        "Résumé   : {} blocage(s), {} avertissement(s), {} info(s)",
        blocking_count, warning_count, other_count
    ));
    lines.push(String::new());
    lines.push("Détails préflight".into());
    for issue in &report.issues {
        lines.push(format!(
            "{} [{}] #{} {} - {}",
            severity_icon(&issue.severity),
            issue.severity,
            issue.work_item_id,
            issue.code,
            issue.message
        ));
        if let Some(details) = &issue.details {
            lines.push(format!("  Détail : {details}"));
        }
        if !issue.related_ids.is_empty() {
            lines.push(format!(
                "  Liés: {}",
                issue
                    .related_ids
                    .iter()
                    .map(|id| format!("#{id}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    if report.has_blocking_issues {
        lines.push(String::new());
        lines.push(
            "Blocages détectés: demander confirmation utilisateur avant de forcer l'implémentation."
                .into(),
        );
    }

    lines
}

fn handoff_validation_lines(report: &TaskHandoffValidationReport) -> Vec<String> {
    let mut lines = vec![
        "Validation handoff".into(),
        format!("Statut    : {}", validation_status_label(report.is_valid)),
        format!("Workspace : {}", report.workspace),
        format!("Projet    : {}", report.project),
        format!(
            "Handoffs  : {}/{} valides",
            report.items.iter().filter(|item| item.valid).count(),
            report.items.len()
        ),
        String::new(),
    ];

    lines.push("Détails handoff".into());
    for item in &report.items {
        lines.push(format!(
            "{} [{}] {}",
            handoff_status_icon(&item.status, item.valid),
            item.status,
            item.repository
        ));
        lines.push(format!("  Message : {}", item.message));
        if !item.path.trim().is_empty() {
            lines.push(format!("  Fichier: {}", item.path));
        }
        if item.valid {
            lines.push(format!(
                "  Synthèse: done={} decisions={} risks={} blockers={} follow_up={}",
                item.done_count,
                item.decision_count,
                item.risk_count,
                item.blocker_count,
                item.follow_up_count
            ));
        }
    }

    if !report.is_valid {
        lines.push(String::new());
        lines.push(
            "Validation handoff échouée: compléter/corriger les handoffs avant task finish.".into(),
        );
    }

    lines
}

fn validation_status_label(valid: bool) -> &'static str {
    if valid { "✓ OK" } else { "✕ À corriger" }
}

fn severity_icon(severity: &str) -> &'static str {
    if is_blocking_severity(severity) {
        "✕"
    } else if is_warning_severity(severity) {
        "!"
    } else {
        "-"
    }
}

fn is_blocking_severity(severity: &str) -> bool {
    matches!(severity.to_ascii_lowercase().as_str(), "blocking" | "error")
}

fn is_warning_severity(severity: &str) -> bool {
    matches!(severity.to_ascii_lowercase().as_str(), "warning" | "warn")
}

fn handoff_status_icon(status: &str, valid: bool) -> &'static str {
    if valid {
        return "✓";
    }
    match status.to_ascii_lowercase().as_str() {
        "missing" | "invalid" | "blocked" => "✕",
        "todo" | "in_progress" => "!",
        _ => "-",
    }
}

fn discover_ai_context_files(workspace: &str) -> Vec<String> {
    let mut files = Vec::new();
    collect_ai_context_files(Path::new(workspace), &mut files);
    files.sort();
    files
}

fn collect_ai_context_files(root: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ai_context_files(&path, files);
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("ai-context") && name.ends_with(".json") {
            files.push(path.display().to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_contracts::{
        HANDOFF_VALIDATION_VERSION, PREFLIGHT_VERSION, TaskHandoffValidationItem,
        TaskPreflightIssue,
    };

    #[test]
    fn preflight_lines_include_blocking_guidance() {
        let report = TaskPreflightReport {
            schema_version: PREFLIGHT_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_ids: vec!["42".into()],
            has_blocking_issues: true,
            issues: vec![TaskPreflightIssue {
                code: "missing_attachment".into(),
                severity: "blocking".into(),
                work_item_id: "42".into(),
                message: "Piece jointe manquante".into(),
                details: Some("screenshot absent".into()),
                related_ids: vec![],
            }],
        };

        let lines = preflight_lines(&report);

        assert_eq!(lines[0], "Préflight task");
        assert!(lines.contains(&"Statut    : ✕ À corriger".into()));
        assert!(lines.contains(&"Workspace : /tmp/ws".into()));
        assert!(lines.contains(&"Résumé   : 1 blocage(s), 0 avertissement(s), 0 info(s)".into()));
        assert!(lines.contains(&"Détails préflight".into()));
        assert!(
            lines.contains(&"✕ [blocking] #42 missing_attachment - Piece jointe manquante".into())
        );
        assert!(lines.contains(
            &"Blocages détectés: demander confirmation utilisateur avant de forcer l'implémentation."
                .into()
        ));
    }

    #[test]
    fn handoff_validation_lines_include_counts_and_failure() {
        let report = TaskHandoffValidationReport {
            schema_version: HANDOFF_VALIDATION_VERSION.into(),
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            is_valid: false,
            items: vec![TaskHandoffValidationItem {
                repository: "front".into(),
                path: "/tmp/ws/front/handoff-front.md".into(),
                status: "done".into(),
                valid: true,
                message: "OK".into(),
                done_count: 2,
                decision_count: 1,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 1,
            }],
        };

        let lines = handoff_validation_lines(&report);

        assert_eq!(lines[0], "Validation handoff");
        assert!(lines.contains(&"Statut    : ✕ À corriger".into()));
        assert!(lines.contains(&"Handoffs  : 1/1 valides".into()));
        assert!(lines.contains(&"Détails handoff".into()));
        assert!(lines.contains(&"✓ [done] front".into()));
        assert!(lines.contains(&"  Message : OK".into()));
        assert!(lines.contains(&"  Fichier: /tmp/ws/front/handoff-front.md".into()));
        assert!(
            lines.contains(&"  Synthèse: done=2 decisions=1 risks=0 blockers=0 follow_up=1".into())
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Validation handoff échouée"))
        );
    }
}
