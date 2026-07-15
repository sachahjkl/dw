use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{
    WorkItemSnapshot, auth::require_token, get_work_item_snapshots_authenticated, run_blocking_ado,
    update_work_item_state_authenticated,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{
    AdoActionEvent, DevWorkflowRoot, ProjectKey, WorkItemId, WorkItemState, WorkItemTypeName,
};
use dw_workspace::{start_state, task_start_options};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct DoingArgs {
    pub ids: Vec<WorkItemId>,
    pub root: Option<DevWorkflowRoot>,
    pub project: ProjectKey,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoingPlanReport {
    pub root: DevWorkflowRoot,
    pub project: ProjectKey,
    pub updates: Vec<DoingPlanUpdate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoingPlanUpdate {
    pub id: WorkItemId,
    #[serde(rename = "type")]
    pub kind: WorkItemTypeName,
    #[serde(rename = "currentState", skip_serializing_if = "Option::is_none")]
    pub current_state: Option<WorkItemState>,
    #[serde(rename = "targetState")]
    pub target_state: WorkItemState,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoingExecutionReport {
    pub plan: DoingPlanReport,
    pub events: Vec<AdoActionEvent>,
    pub updated: Vec<DoingUpdate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DoingUpdate {
    pub id: WorkItemId,
    pub state: WorkItemState,
}

pub async fn plan(args: DoingArgs) -> Result<DoingPlanReport> {
    if args.ids.is_empty() {
        return Err(anyhow::anyhow!("At least one work item is required."));
    }
    let root = DevWorkflowRoot::from(resolve_root(
        args.root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let projects = load_projects_config(root.as_str());
    let workflow = load_workflow_config(root.as_str());
    let options = resolve_ado_options(&projects, &workflow, args.project.as_str())?;
    let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
    let ids = args.ids.clone();
    let snapshots =
        run_blocking_ado(move || get_work_item_snapshots_authenticated(&options, &ids, &token))
            .await?;
    ensure_all_items_loaded(&args.ids, &snapshots)?;
    let start_options = task_start_options(&workflow);
    let updates = snapshots
        .into_iter()
        .map(|snapshot| plan_update(snapshot, &start_options))
        .collect::<Result<Vec<_>>>()?;

    Ok(DoingPlanReport {
        root,
        project: args.project,
        updates,
    })
}

pub async fn execute(plan: DoingPlanReport) -> Result<DoingExecutionReport> {
    execute_with_events(plan, |_| {}).await
}

pub async fn execute_with_events(
    plan: DoingPlanReport,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<DoingExecutionReport> {
    let projects = load_projects_config(plan.root.as_str());
    let workflow = load_workflow_config(plan.root.as_str());
    let options = resolve_ado_options(&projects, &workflow, plan.project.as_str())?;
    let token = require_token(load_auth_options(Some(plan.root.as_str()))?).await?;
    let mut events = Vec::new();
    let mut updated = Vec::new();

    for update in plan.updates.iter().filter(|update| update.changed) {
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::UpdatingWorkItemState {
                ids: vec![update.id.clone()],
                state: update.target_state.clone(),
            },
        );
        let options = options.clone();
        let token = token.clone();
        let id = update.id.clone();
        let state = update.target_state.clone();
        run_blocking_ado(move || {
            update_work_item_state_authenticated(
                &options,
                id.as_str(),
                state.as_str(),
                "DevWorkflow: passage en cours",
                &token,
            )
        })
        .await?;
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::UpdatedWorkItemState {
                id: update.id.clone(),
                state: update.target_state.clone(),
            },
        );
        updated.push(DoingUpdate {
            id: update.id.clone(),
            state: update.target_state.clone(),
        });
    }

    Ok(DoingExecutionReport {
        plan,
        events,
        updated,
    })
}

fn plan_update(
    snapshot: WorkItemSnapshot,
    options: &dw_workspace::TaskStartOptions,
) -> Result<DoingPlanUpdate> {
    let kind = snapshot
        .kind
        .filter(|kind| !kind.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Work item #{} has no type.", snapshot.id))?;
    let target_state = start_state(Some(&kind), options).ok_or_else(|| {
        anyhow::anyhow!(
            "Work item #{} has unsupported type `{kind}` for `task doing`.",
            snapshot.id
        )
    })?;
    let current_state = snapshot.state.map(WorkItemState::from);
    let changed = !current_state
        .as_ref()
        .is_some_and(|state| state.as_str().eq_ignore_ascii_case(target_state.as_str()));
    Ok(DoingPlanUpdate {
        id: snapshot.id,
        kind: WorkItemTypeName::from(kind),
        current_state,
        target_state,
        changed,
    })
}

fn ensure_all_items_loaded(ids: &[WorkItemId], snapshots: &[WorkItemSnapshot]) -> Result<()> {
    let missing = ids
        .iter()
        .filter(|id| !snapshots.iter().any(|snapshot| snapshot.id == **id))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Work items not found: {}.",
            missing.join(", ")
        ))
    }
}

fn push_event(
    events: &mut Vec<AdoActionEvent>,
    emit: &mut impl FnMut(AdoActionEvent),
    event: AdoActionEvent,
) {
    emit(event.clone());
    events.push(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_update_maps_task_to_configured_doing_state() {
        let update = plan_update(
            WorkItemSnapshot {
                id: WorkItemId::from("116"),
                kind: Some("Task".into()),
                state: Some("A faire".into()),
                title: Some("Corriger".into()),
                url: None,
            },
            &dw_workspace::TaskStartOptions::default(),
        )
        .expect("doing update");

        assert_eq!(update.target_state.as_str(), "En développement");
        assert!(update.changed);
    }

    #[test]
    fn plan_update_rejects_unsupported_types() {
        let error = plan_update(
            WorkItemSnapshot {
                id: WorkItemId::from("1"),
                kind: Some("Epic".into()),
                state: Some("Active".into()),
                title: None,
                url: None,
            },
            &dw_workspace::TaskStartOptions::default(),
        )
        .expect_err("epic is unsupported");

        assert!(error.to_string().contains("unsupported type `Epic`"));
    }
}
