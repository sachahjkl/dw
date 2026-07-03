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
        format!(
            "Preflight: {}",
            validation_status_label(!report.has_blocking_issues)
        ),
        format!("Workspace: {}", report.workspace),
        format!("Projet: {}", report.project),
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
        lines.push("✓ Aucun warning ni blocage détecté.".into());
        return lines;
    }

    lines.push(format!("Issues: {}", report.issues.len()));
    lines.push(String::new());
    for issue in &report.issues {
        lines.push(format!(
            "{} [{}] {}: {}",
            severity_icon(&issue.severity),
            issue.severity,
            issue.code,
            issue.message
        ));
        if let Some(details) = &issue.details {
            lines.push(format!("  {details}"));
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
        format!(
            "Handoff validation: {}",
            validation_status_label(report.is_valid)
        ),
        format!("Workspace: {}", report.workspace),
        format!("Projet: {}", report.project),
        format!(
            "Handoffs: {}/{} valides",
            report.items.iter().filter(|item| item.valid).count(),
            report.items.len()
        ),
        String::new(),
    ];

    for item in &report.items {
        lines.push(format!(
            "{} [{}] {}: {}",
            handoff_status_icon(&item.status, item.valid),
            item.status,
            item.repository,
            item.message
        ));
        if item.valid {
            lines.push(format!(
                "  done={} decisions={} risks={} blockers={} follow_up={}",
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
    match severity.to_ascii_lowercase().as_str() {
        "blocking" | "error" => "✕",
        "warning" | "warn" => "!",
        _ => "-",
    }
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

        assert_eq!(lines[0], "Preflight: ✕ À corriger");
        assert!(lines.contains(&"Workspace: /tmp/ws".into()));
        assert!(lines.contains(&"Issues: 1".into()));
        assert!(lines.contains(&"✕ [blocking] missing_attachment: Piece jointe manquante".into()));
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

        assert_eq!(lines[0], "Handoff validation: ✕ À corriger");
        assert!(lines.contains(&"Handoffs: 1/1 valides".into()));
        assert!(lines.contains(&"✓ [done] front: OK".into()));
        assert!(lines.contains(&"  done=2 decisions=1 risks=0 blockers=0 follow_up=1".into()));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Validation handoff échouée"))
        );
    }
}
