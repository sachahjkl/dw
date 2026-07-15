use anyhow::Result;
use dw_config::resolve_root;
use dw_contracts::{TaskHandoffValidationReport, TaskPreflightReport};
use dw_core::{AiContextFilePath, DevWorkflowRoot, ProjectKey, WorkItemId, WorkspacePath};
use dw_workspace::{
    build_handoff_validation_report, build_preflight_report_from_ai_context_files,
    resolve_workspace_by_work_item_ids,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PreflightArgs {
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub r#continue: bool,
    pub ai_context_files: Vec<AiContextFilePath>,
}

#[derive(Debug, Clone)]
pub struct HandoffValidateArgs {
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub r#continue: bool,
}

pub fn preflight_report(args: PreflightArgs) -> Result<TaskPreflightReport> {
    let PreflightArgs {
        workspace,
        root,
        project,
        work_item_ids,
        r#continue,
        ai_context_files,
    } = args;
    let root = resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_by_work_item_ids(
        &root,
        workspace.as_ref().map(WorkspacePath::as_str),
        project.as_ref().map(ProjectKey::as_str),
        &work_item_ids,
        r#continue,
    )?;
    let files = if ai_context_files.is_empty() {
        discover_ai_context_files(&workspace)
    } else {
        ai_context_files
    };

    if files.is_empty() {
        return Err(anyhow::anyhow!(
            "No ai-context file detected. Provide explicit ai-context files or place ai-context*.json files in the workspace."
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
        work_item_ids,
        r#continue,
    } = args;
    let root = resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_by_work_item_ids(
        &root,
        workspace.as_ref().map(WorkspacePath::as_str),
        project.as_ref().map(ProjectKey::as_str),
        &work_item_ids,
        r#continue,
    )?;
    Ok(build_handoff_validation_report(&workspace)?)
}

fn discover_ai_context_files(workspace: &WorkspacePath) -> Vec<AiContextFilePath> {
    let mut files = Vec::new();
    collect_ai_context_files(Path::new(workspace.as_str()), &mut files);
    files.sort();
    files
}

fn collect_ai_context_files(root: &Path, files: &mut Vec<AiContextFilePath>) {
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
            files.push(AiContextFilePath::from(path.display().to_string()));
        }
    }
}
