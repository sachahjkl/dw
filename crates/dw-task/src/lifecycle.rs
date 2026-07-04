use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{
    WorkItemSnapshot, WorkspaceChildTaskCreateResult, auth::require_token,
    create_child_task_authenticated as ado_create_child_task,
    get_work_item_snapshots_authenticated,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_workspace::{
    WorkspaceManifest, execute_add_child_task, execute_task_rename, execute_task_sync,
    plan_task_rename, read_manifest_path, requires_child_tasks, resolve_workspace,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SyncArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RenameArgs {
    pub slug: String,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub mode: dw_core::ExecutionMode,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateChildTaskArgs {
    pub repo: String,
    pub title: String,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncReport {
    pub workspace: String,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<String>,
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
    pub workspace: String,
    pub repository: String,
    pub parent: dw_workspace::WorkspaceWorkItem,
    #[serde(rename = "requestedTitle")]
    pub requested_title: String,
    pub created: WorkspaceChildTaskCreateResult,
    pub manifest: WorkspaceManifest,
}

pub async fn sync_report(args: SyncArgs) -> Result<SyncReport> {
    let root = resolve_root(args.root.as_deref());
    let workspace = resolve_workspace(
        &root,
        args.workspace.as_deref(),
        args.project.as_deref(),
        args.work_item.as_deref(),
        args.positional_work_item.as_deref(),
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
    let snapshots = get_work_item_snapshots_authenticated(&options, &requested_ids, &token)?;
    let updated = execute_task_sync(&workspace, &snapshots)?;

    Ok(SyncReport {
        workspace,
        requested_ids,
        snapshots,
        manifest: updated,
    })
}

pub fn rename_plan(args: RenameArgs) -> Result<RenamePlanReport> {
    let root = resolve_root(args.root.as_deref());
    let projects = load_projects_config(&root);
    let workspace = resolve_workspace(
        &root,
        args.workspace.as_deref(),
        args.project.as_deref(),
        args.work_item.as_deref(),
        args.positional_work_item.as_deref(),
        args.r#continue,
    )?;
    let (_manifest, plan) = plan_task_rename(&root, &projects, &workspace, &args.slug)?;

    Ok(RenamePlanReport { plan })
}

pub fn execute_rename(report: &RenamePlanReport) -> Result<RenameExecutionReport> {
    let manifest = read_manifest_path(&format!("{}/task.json", report.plan.workspace))?;
    let updated = execute_task_rename(&manifest, &report.plan)?;
    Ok(RenameExecutionReport {
        plan: report.plan.clone(),
        manifest: updated,
    })
}

pub async fn create_child_task_report(args: CreateChildTaskArgs) -> Result<CreateChildTaskReport> {
    let root = resolve_root(args.root.as_deref());
    let workspace = resolve_workspace(
        &root,
        args.workspace.as_deref(),
        args.project.as_deref(),
        args.work_item.as_deref(),
        args.positional_work_item.as_deref(),
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
    let task_title = child_task_title(&args.repo, &args.title);
    let created = ado_create_child_task(
        &options,
        &WorkItemSnapshot {
            id: parent.id.clone(),
            kind: parent.kind.clone(),
            state: parent.state.clone(),
            title: parent.title.clone(),
            url: None,
        },
        &args.repo,
        &task_title,
        "task create-child-task",
        &token,
    )?;
    let updated = execute_add_child_task(
        &workspace,
        &args.repo,
        &created.id,
        Some(created.title.clone()),
    )?;

    Ok(CreateChildTaskReport {
        workspace,
        repository: args.repo,
        parent,
        requested_title: task_title,
        created,
        manifest: updated,
    })
}

pub fn sync_fetch_line(item_count: usize) -> String {
    match item_count {
        0 => "Synchronisation ADO: aucun work item parent à charger.".into(),
        1 => "Synchronisation ADO: chargement de 1 work item parent...".into(),
        count => format!("Synchronisation ADO: chargement de {count} work items parents..."),
    }
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
    fn sync_fetch_line_handles_counts() {
        assert_eq!(
            sync_fetch_line(0),
            "Synchronisation ADO: aucun work item parent à charger."
        );
        assert_eq!(
            sync_fetch_line(1),
            "Synchronisation ADO: chargement de 1 work item parent..."
        );
        assert_eq!(
            sync_fetch_line(3),
            "Synchronisation ADO: chargement de 3 work items parents..."
        );
    }

    #[test]
    fn child_task_title_uses_domain_prefix() {
        assert_eq!(child_task_title("front", "Corriger"), "[FRONT] Corriger");
        assert_eq!(child_task_title("db", "Corriger"), "[SQL] Corriger");
        assert_eq!(child_task_title("api", "Corriger"), "[API] Corriger");
    }
}
