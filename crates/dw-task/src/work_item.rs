use crate::{load_auth_options, resolve_ado_options, write_workspace_agent_configs};
use anyhow::Result;
use dw_ado::WorkItemSnapshot;
use dw_ado::auth::require_token;
use dw_ado::{get_work_item_snapshots_authenticated, query_assigned_work_items};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_workspace::{
    WorkspaceManifest, WorkspaceWorkItem, execute_work_item_update,
    parse_work_item_ids as parse_workspace_work_item_ids, plan_add_work_item_snapshots,
    plan_add_work_items, plan_remove_work_items, read_manifest_path, resolve_workspace,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AddWorkItemArgs {
    pub work_item_ids: Option<String>,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub skip_ado: bool,
    pub type_name: Option<String>,
    pub title: Option<String>,
    pub state: Option<String>,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct RemoveWorkItemArgs {
    pub work_item_ids: Option<String>,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct WorkItemChoicesArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkItemUpdateAction {
    Add,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItemChoicesReport {
    pub workspace: String,
    pub project: String,
    pub choices: Vec<WorkspaceWorkItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItemUpdatePlanReport {
    pub action: WorkItemUpdateAction,
    pub workspace: String,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<String>,
    #[serde(rename = "skippedExistingIds")]
    pub skipped_existing_ids: Vec<String>,
    pub snapshots: Vec<WorkItemSnapshot>,
    pub plan: Option<dw_workspace::TaskWorkItemUpdatePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItemUpdateExecutionReport {
    pub action: WorkItemUpdateAction,
    pub plan: dw_workspace::TaskWorkItemUpdatePlan,
    pub manifest: WorkspaceManifest,
    #[serde(rename = "newWorkspace")]
    pub new_workspace: String,
}

pub async fn add_work_item_choices_report(
    args: WorkItemChoicesArgs,
) -> Result<WorkItemChoicesReport> {
    let root = resolve_root(args.root.as_deref());
    let workspace = resolve_workspace_from_args(&root, &args)?;
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let mut options = resolve_ado_options(&projects, &workflow, &manifest.project)?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let items = query_assigned_work_items(&options, 50, &token).await?;
    let choices = items
        .into_iter()
        .filter(|item| !manifest.matches_work_item(&item.id))
        .filter(|item| !dw_workspace::is_final_state(item.kind.as_deref(), item.state.as_deref()))
        .map(|item| WorkspaceWorkItem {
            id: item.id,
            kind: item.kind,
            title: item.title,
            state: item.state,
        })
        .collect::<Vec<_>>();

    Ok(WorkItemChoicesReport {
        workspace,
        project: manifest.project,
        choices,
    })
}

pub fn removable_work_item_choices_report(
    args: WorkItemChoicesArgs,
) -> Result<WorkItemChoicesReport> {
    let root = resolve_root(args.root.as_deref());
    let workspace = resolve_workspace_from_args(&root, &args)?;
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    Ok(WorkItemChoicesReport {
        workspace,
        project: manifest.project.clone(),
        choices: removable_work_item_choices(&manifest),
    })
}

pub async fn add_plan(args: AddWorkItemArgs) -> Result<WorkItemUpdatePlanReport> {
    let work_item_ids = args
        .work_item_ids
        .as_deref()
        .filter(|ids| !ids.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("Work items à ajouter manquants. Fournir au moins un identifiant.")
        })?
        .to_owned();
    let root = resolve_root(args.root.as_deref());
    let workspace = resolve_workspace(
        &root,
        args.workspace.as_deref(),
        args.project.as_deref(),
        args.work_item.as_deref(),
        args.positional_work_item.as_deref(),
        args.r#continue,
    )?;
    let current_manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let requested_ids = parse_workspace_work_item_ids(&work_item_ids);
    let missing_ids = requested_ids
        .iter()
        .filter(|id| !current_manifest.matches_work_item(id))
        .cloned()
        .collect::<Vec<_>>();
    let skipped_existing_ids = requested_ids
        .iter()
        .filter(|id| current_manifest.matches_work_item(id))
        .cloned()
        .collect::<Vec<_>>();
    if missing_ids.is_empty() {
        return Ok(WorkItemUpdatePlanReport {
            action: WorkItemUpdateAction::Add,
            workspace,
            requested_ids,
            skipped_existing_ids,
            snapshots: Vec::new(),
            plan: None,
        });
    }

    let (plan, snapshots) = if args.skip_ado {
        let (_manifest, plan) = plan_add_work_items(
            &root,
            &workspace,
            &work_item_ids,
            args.type_name.as_deref(),
            args.title.as_deref(),
            args.state.as_deref(),
        )?;
        (plan, Vec::new())
    } else {
        let projects = load_projects_config(&root);
        let workflow = load_workflow_config(&root);
        let mut options = resolve_ado_options(&projects, &workflow, &current_manifest.project)?;
        if options.project.trim().is_empty() {
            options.project = current_manifest.project.clone();
        }
        let token = require_token(load_auth_options(Some(&root))?).await?;
        let snapshots = get_work_item_snapshots_authenticated(&options, &missing_ids, &token)?;
        ensure_all_snapshots_resolved(&missing_ids, &snapshots)?;
        ensure_no_final_snapshots(&snapshots)?;
        let (_manifest, plan) = plan_add_work_item_snapshots(&root, &workspace, &snapshots)?;
        (plan, snapshots)
    };

    Ok(WorkItemUpdatePlanReport {
        action: WorkItemUpdateAction::Add,
        workspace,
        requested_ids,
        skipped_existing_ids,
        snapshots,
        plan: Some(plan),
    })
}

pub fn remove_plan(args: RemoveWorkItemArgs) -> Result<WorkItemUpdatePlanReport> {
    let work_item_ids = args
        .work_item_ids
        .as_deref()
        .filter(|ids| !ids.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("Work items à retirer manquants. Fournir au moins un identifiant.")
        })?
        .to_owned();
    let root = resolve_root(args.root.as_deref());
    let workspace = resolve_workspace(
        &root,
        args.workspace.as_deref(),
        args.project.as_deref(),
        args.work_item.as_deref(),
        args.positional_work_item.as_deref(),
        args.r#continue,
    )?;
    let requested_ids = parse_workspace_work_item_ids(&work_item_ids);
    let (_manifest, plan) = plan_remove_work_items(&root, &workspace, &work_item_ids)?;

    Ok(WorkItemUpdatePlanReport {
        action: WorkItemUpdateAction::Remove,
        workspace,
        requested_ids,
        skipped_existing_ids: Vec::new(),
        snapshots: Vec::new(),
        plan: Some(plan),
    })
}

pub fn execute_update(
    report: &WorkItemUpdatePlanReport,
) -> Result<Option<WorkItemUpdateExecutionReport>> {
    let Some(plan) = &report.plan else {
        return Ok(None);
    };
    let manifest = read_manifest_path(&format!("{}/task.json", plan.workspace))?;
    let (updated, new_workspace) = execute_work_item_update(&manifest, plan)?;
    write_workspace_agent_configs(&new_workspace, &updated)?;
    Ok(Some(WorkItemUpdateExecutionReport {
        action: report.action,
        plan: plan.clone(),
        manifest: updated,
        new_workspace,
    }))
}

pub fn work_item_choice_label(item: &WorkspaceWorkItem) -> String {
    format!(
        "#{}{}{}{}",
        item.id,
        item.kind
            .as_ref()
            .map(|kind| format!(" [{kind}]"))
            .unwrap_or_default(),
        item.state
            .as_ref()
            .map(|state| format!(" ({state})"))
            .unwrap_or_default(),
        item.title
            .as_ref()
            .map(|title| format!(" {title}"))
            .unwrap_or_default()
    )
}

pub fn work_item_id_from_choice(label: &str) -> String {
    label
        .trim_start_matches('#')
        .split_whitespace()
        .next()
        .unwrap_or(label)
        .to_string()
}

pub fn work_item_fetch_line(item_count: usize) -> String {
    match item_count {
        0 => "Chargement ADO: aucun work item à résoudre.".into(),
        1 => "Chargement ADO: résolution de 1 work item...".into(),
        count => format!("Chargement ADO: résolution de {count} work items..."),
    }
}

fn resolve_workspace_from_args(root: &str, args: &WorkItemChoicesArgs) -> Result<String> {
    Ok(resolve_workspace(
        root,
        args.workspace.as_deref(),
        args.project.as_deref(),
        args.work_item.as_deref(),
        args.positional_work_item.as_deref(),
        args.r#continue,
    )?)
}

fn removable_work_item_choices(manifest: &WorkspaceManifest) -> Vec<WorkspaceWorkItem> {
    manifest.parent_work_items()
}

fn ensure_all_snapshots_resolved(
    requested: &[String],
    snapshots: &[WorkItemSnapshot],
) -> Result<()> {
    if snapshots.len() == requested.len() {
        return Ok(());
    }
    let found = snapshots
        .iter()
        .map(|snapshot| snapshot.id.clone())
        .collect::<Vec<_>>();
    let unresolved = requested
        .iter()
        .filter(|id| {
            !found
                .iter()
                .any(|found_id| found_id.eq_ignore_ascii_case(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    Err(anyhow::anyhow!(
        "Work items ADO introuvables ou inaccessibles: {}",
        unresolved.join(", ")
    ))
}

fn ensure_no_final_snapshots(snapshots: &[WorkItemSnapshot]) -> Result<()> {
    let final_items = snapshots
        .iter()
        .filter(|snapshot| {
            dw_workspace::is_final_state(snapshot.kind.as_deref(), snapshot.state.as_deref())
        })
        .collect::<Vec<_>>();
    if final_items.is_empty() {
        return Ok(());
    }
    let labels = final_items
        .iter()
        .map(|item| {
            format!(
                "#{} ({})",
                item.id,
                item.state.as_deref().unwrap_or("état inconnu")
            )
        })
        .collect::<Vec<_>>();
    Err(anyhow::anyhow!(
        "Impossible d'ajouter des work items en état final: {}",
        labels.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removable_work_item_choices_include_context() {
        let manifest = workspace_manifest_with_items(vec![
            WorkspaceWorkItem {
                id: "1".into(),
                kind: Some("User Story".into()),
                title: Some("Parent".into()),
                state: Some("Active".into()),
            },
            WorkspaceWorkItem {
                id: "2".into(),
                kind: Some("Bug".into()),
                title: Some("Secondaire".into()),
                state: Some("New".into()),
            },
        ]);

        let choices = removable_work_item_choices(&manifest);

        assert_eq!(
            work_item_choice_label(&choices[0]),
            "#1 [User Story] (Active) Parent"
        );
        assert_eq!(
            work_item_choice_label(&choices[1]),
            "#2 [Bug] (New) Secondaire"
        );
        assert_eq!(
            work_item_id_from_choice(&work_item_choice_label(&choices[1])),
            "2"
        );
    }

    #[test]
    fn work_item_fetch_line_handles_counts() {
        assert_eq!(
            work_item_fetch_line(0),
            "Chargement ADO: aucun work item à résoudre."
        );
        assert_eq!(
            work_item_fetch_line(1),
            "Chargement ADO: résolution de 1 work item..."
        );
        assert_eq!(
            work_item_fetch_line(3),
            "Chargement ADO: résolution de 3 work items..."
        );
    }

    #[test]
    fn add_work_item_choice_uses_same_context_format() {
        let item = WorkspaceWorkItem {
            id: "3".into(),
            kind: Some("Task".into()),
            title: Some("À ajouter".into()),
            state: Some("Active".into()),
        };

        assert_eq!(
            work_item_choice_label(&item),
            "#3 [Task] (Active) À ajouter"
        );
    }

    #[test]
    fn final_snapshots_are_rejected() {
        let result = ensure_no_final_snapshots(&[WorkItemSnapshot {
            id: "9".into(),
            kind: Some("Task".into()),
            state: Some("Clôturé".into()),
            title: Some("Done".into()),
            url: None,
        }]);

        assert!(result.is_err());
    }

    fn workspace_manifest_with_items(items: Vec<WorkspaceWorkItem>) -> WorkspaceManifest {
        WorkspaceManifest {
            schema: 1,
            project: "ha".into(),
            work_item_id: "1".into(),
            task_id: None,
            kind: "feature".into(),
            slug: "parent".into(),
            branch_name: "feature/1-parent".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            repositories: Vec::new(),
            status: "active".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Parent".into()),
            work_item_state: Some("Active".into()),
            child_task_ids: None,
            child_tasks: None,
            work_items: Some(items),
        }
    }
}
