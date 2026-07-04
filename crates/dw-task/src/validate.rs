use anyhow::Result;
use dw_config::resolve_root;
use dw_contracts::{TaskHandoffValidationReport, TaskPreflightReport};
use dw_workspace::{
    build_handoff_validation_report, build_preflight_report_from_ai_context_files,
    resolve_workspace,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PreflightArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub ai_context_file: Vec<String>,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HandoffValidateArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
}

pub fn preflight_report(args: PreflightArgs) -> Result<TaskPreflightReport> {
    let PreflightArgs {
        workspace,
        root,
        project,
        work_item,
        r#continue,
        ai_context_file,
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
            "Aucun fichier ai-context détecté. Fournir des fichiers ai-context explicites ou placer des fichiers ai-context*.json dans le workspace."
        ));
    }

    Ok(build_preflight_report_from_ai_context_files(
        &workspace, &files,
    )?)
}

pub fn handoff_validation_report(args: HandoffValidateArgs) -> Result<TaskHandoffValidationReport> {
    let HandoffValidateArgs {
        workspace,
        root,
        project,
        work_item,
        r#continue,
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
    Ok(build_handoff_validation_report(&workspace)?)
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
