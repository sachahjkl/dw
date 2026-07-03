use anyhow::Result;
use dw_workspace::{build_handoff_validation_report, build_preflight_report_from_ai_context_files};
use std::path::Path;

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
        println!("Preflight workspace: {}", report.workspace);
        println!("Projet: {}", report.project);
        println!(
            "Work items: {}",
            report
                .work_item_ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!();
        if report.issues.is_empty() {
            println!("Aucun warning ni blocage detecte.");
        } else {
            for issue in &report.issues {
                println!("- [{}] {}: {}", issue.severity, issue.code, issue.message);
                if let Some(details) = &issue.details {
                    println!("  {}", details);
                }
            }
            if report.has_blocking_issues {
                println!();
                println!(
                    "Blocages detectes: demander confirmation utilisateur avant de forcer l'implementation."
                );
            }
        }
    }
    Ok(())
}

pub fn handoff_validate(workspace: String, json: bool) -> Result<()> {
    let report = build_handoff_validation_report(&workspace)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Handoff validation: {}", report.workspace);
        println!("Projet: {}", report.project);
        println!();
        for item in &report.items {
            println!("- [{}] {}: {}", item.status, item.repository, item.message);
            if item.valid {
                println!(
                    "  done={} decisions={} risks={} blockers={} follow_up={}",
                    item.done_count,
                    item.decision_count,
                    item.risk_count,
                    item.blocker_count,
                    item.follow_up_count
                );
            }
        }

        if !report.is_valid {
            println!();
            println!(
                "Validation handoff echouee: completer/corriger les handoffs avant task finish."
            );
        }
    }
    Ok(())
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
