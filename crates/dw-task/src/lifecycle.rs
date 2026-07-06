use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{
    WorkItemSnapshot, WorkspaceChildTaskCreateResult, auth::require_token,
    create_child_task_authenticated as ado_create_child_task,
    get_work_item_snapshots_authenticated, run_blocking_ado,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{
    DevWorkflowRoot, ProjectKey, TaskSlug, WorkItemId, WorkItemTitle, WorkspacePath,
    WorkspaceRepositoryName,
};
use dw_workspace::{
    WorkspaceManifest, execute_add_child_task, execute_task_rename, execute_task_sync,
    plan_task_rename, read_manifest_path, requires_child_tasks, resolve_workspace_by_work_item_ids,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SyncArgs {
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub r#continue: bool,
}

#[derive(Debug, Clone)]
pub struct RenameArgs {
    pub slug: TaskSlug,
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub r#continue: bool,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct CreateChildTaskArgs {
    pub repo: WorkspaceRepositoryName,
    pub title: WorkItemTitle,
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub r#continue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncReport {
    pub workspace: WorkspacePath,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<WorkItemId>,
    pub snapshots: Vec<WorkItemSnapshot>,
    pub manifest: WorkspaceManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenamePlanReport {
    pub plan: dw_workspace::TaskRenamePlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameExecutionReport {
    pub plan: dw_workspace::TaskRenamePlan,
    pub manifest: WorkspaceManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateChildTaskReport {
    pub workspace: WorkspacePath,
    pub repository: WorkspaceRepositoryName,
    pub parent: dw_workspace::WorkspaceWorkItem,
    #[serde(rename = "requestedTitle")]
    pub requested_title: WorkItemTitle,
    pub created: WorkspaceChildTaskCreateResult,
    pub manifest: WorkspaceManifest,
}

pub async fn sync_report(args: SyncArgs) -> Result<SyncReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_by_work_item_ids(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.project.as_ref().map(ProjectKey::as_str),
        &args.work_item_ids,
        args.r#continue,
    )?;
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let projects = load_projects_config(&root);
    let mut options = dw_config::resolve_project(&projects, &manifest.project)
        .and_then(|project| project.azure_dev_ops)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Configuration azureDevOps manquante dans projects.json pour {}.",
                manifest.project
            )
        })?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let requested_ids = manifest
        .parent_work_items()
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let options_for_fetch = options.clone();
    let requested_ids_for_fetch = requested_ids.clone();
    let token_for_fetch = token.clone();
    let snapshots = run_blocking_ado(move || {
        get_work_item_snapshots_authenticated(
            &options_for_fetch,
            &requested_ids_for_fetch,
            &token_for_fetch,
        )
    })
    .await?;
    let updated = execute_task_sync(&workspace, &snapshots)?;

    Ok(SyncReport {
        workspace: WorkspacePath::from(workspace),
        requested_ids: requested_ids.into_iter().map(WorkItemId::from).collect(),
        snapshots,
        manifest: updated,
    })
}

pub fn rename_plan(args: RenameArgs) -> Result<RenamePlanReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let projects = load_projects_config(&root);
    let workspace = resolve_workspace_by_work_item_ids(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.project.as_ref().map(ProjectKey::as_str),
        &args.work_item_ids,
        args.r#continue,
    )?;
    let (_manifest, plan) = plan_task_rename(&root, &projects, &workspace, args.slug.as_str())?;

    Ok(RenamePlanReport { plan })
}

pub fn execute_rename(report: &RenamePlanReport) -> Result<RenameExecutionReport> {
    let manifest = read_manifest_path(&format!("{}/task.json", report.plan.workspace.as_str()))?;
    let updated = execute_task_rename(&manifest, &report.plan)?;
    Ok(RenameExecutionReport {
        plan: report.plan.clone(),
        manifest: updated,
    })
}

pub async fn create_child_task_report(args: CreateChildTaskArgs) -> Result<CreateChildTaskReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_by_work_item_ids(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.project.as_ref().map(ProjectKey::as_str),
        &args.work_item_ids,
        args.r#continue,
    )?;
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let parent = manifest.parent_work_items()[0].clone();
    if !requires_child_tasks(parent.kind.as_deref()) {
        return Err(anyhow::anyhow!(
            "Cette commande est réservée aux User Story et Anomalie."
        ));
    }
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let mut options = resolve_ado_options(&projects, &workflow, &manifest.project)?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let task_title = WorkItemTitle::from(child_task_title(args.repo.as_str(), args.title.as_str()));
    let parent_snapshot = WorkItemSnapshot {
        id: parent.id.clone(),
        kind: parent.kind.clone(),
        state: parent.state.clone(),
        title: parent.title.clone(),
        url: None,
    };
    let task_title_for_create = task_title.clone();
    let options_for_create = options.clone();
    let repo_for_create = args.repo.clone();
    let token_for_create = token.clone();
    let created = run_blocking_ado(move || {
        ado_create_child_task(
            &options_for_create,
            &parent_snapshot,
            repo_for_create.as_str(),
            task_title_for_create.as_str(),
            "task create-child-task",
            &token_for_create,
        )
    })
    .await?;
    let updated = execute_add_child_task(
        &workspace,
        args.repo.as_str(),
        &created.id,
        Some(created.title.clone()),
    )?;

    Ok(CreateChildTaskReport {
        workspace: WorkspacePath::from(workspace),
        repository: args.repo,
        parent,
        requested_title: task_title,
        created,
        manifest: updated,
    })
}

pub fn child_task_title(repository: &str, title: &str) -> String {
    let normalized = repository.to_ascii_lowercase();
    let prefix = match normalized.as_str() {
        "front" => "FRONT",
        "back" => "BACK",
        "sql" | "db" | "database" => "SQL",
        other => other,
    };
    format!("[{}] {}", prefix.to_ascii_uppercase(), title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_task_title_uses_domain_prefix() {
        assert_eq!(child_task_title("front", "Corriger"), "[FRONT] Corriger");
        assert_eq!(child_task_title("db", "Corriger"), "[SQL] Corriger");
        assert_eq!(child_task_title("api", "Corriger"), "[API] Corriger");
    }
}
