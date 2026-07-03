use anyhow::Result;
use dw_contracts::{TaskHandoffValidationReport, TaskPreflightReport};
use dw_workspace::{build_handoff_validation_report, build_preflight_report_from_ai_context_files};
use std::path::Path;

use crate::render::print_styled_lines;

pub fn preflight(workspace: String, ai_context_file: Vec<String>, json: bool) -> Result<()> {
    let files = if ai_context_file.is_empty() {
        discover_ai_context_files(&workspace)
    } else {
        ai_context_file
    };

    if files.is_empty() {
        return Err(anyhow::anyhow!(
            "Aucun fichier ai-context detecte. Fournir --ai-context-file ou placer des fichiers ai-context*.json dans le workspace."
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

pub fn handoff_validate(workspace: String, json: bool) -> Result<()> {
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
        format!("Preflight workspace: {}", report.workspace),
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
        lines.push("Aucun warning ni blocage detecte.".into());
        return lines;
    }

    for issue in &report.issues {
        lines.push(format!(
            "- [{}] {}: {}",
            issue.severity, issue.code, issue.message
        ));
        if let Some(details) = &issue.details {
            lines.push(format!("  {details}"));
        }
    }

    if report.has_blocking_issues {
        lines.push(String::new());
        lines.push(
            "Blocages detectes: demander confirmation utilisateur avant de forcer l'implementation."
                .into(),
        );
    }

    lines
}

fn handoff_validation_lines(report: &TaskHandoffValidationReport) -> Vec<String> {
    let mut lines = vec![
        format!("Handoff validation: {}", report.workspace),
        format!("Projet: {}", report.project),
        String::new(),
    ];

    for item in &report.items {
        lines.push(format!(
            "- [{}] {}: {}",
            item.status, item.repository, item.message
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

        assert_eq!(lines[0], "Preflight workspace: /tmp/ws");
        assert!(lines.contains(&"- [blocking] missing_attachment: Piece jointe manquante".into()));
        assert!(lines.contains(
            &"Blocages detectes: demander confirmation utilisateur avant de forcer l'implementation."
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

        assert!(lines.contains(&"- [done] front: OK".into()));
        assert!(lines.contains(&"  done=2 decisions=1 risks=0 blockers=0 follow_up=1".into()));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Validation handoff échouée"))
        );
    }
}
