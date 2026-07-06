use crate::commands::work_item::ado_work_item_id_values;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{auth::require_token, run_blocking_ado, update_work_item_state_authenticated};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{
    AdoActionEvent, DevWorkflowRoot, ProjectKey, WorkItemHistoryComment, WorkItemId, WorkItemState,
};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct SetStateArgs {
    pub ids: Vec<WorkItemId>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub state: WorkItemState,
    pub history: Option<WorkItemHistoryComment>,
    pub yes: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetStatePlanReport {
    pub root: DevWorkflowRoot,
    pub project: ProjectKey,
    pub ids: Vec<WorkItemId>,
    pub state: WorkItemState,
    pub history: WorkItemHistoryComment,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetStateExecutionReport {
    pub plan: SetStatePlanReport,
    pub events: Vec<AdoActionEvent>,
    pub updated: Vec<SetStateUpdate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetStateUpdate {
    pub id: WorkItemId,
    pub state: WorkItemState,
}

pub fn plan(args: SetStateArgs) -> Result<SetStatePlanReport> {
    let SetStateArgs {
        ids,
        root,
        project,
        state,
        history,
        yes: _,
    } = args;
    let root = DevWorkflowRoot::from(resolve_root(root.as_ref().map(DevWorkflowRoot::as_str)));
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado set-state requiert un projet configuré."))?;
    if ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }

    Ok(SetStatePlanReport {
        root,
        project: project_key,
        ids,
        state,
        history: history.unwrap_or_default(),
    })
}

pub async fn execute(plan: SetStatePlanReport) -> Result<SetStateExecutionReport> {
    execute_with_events(plan, |_| {}).await
}

pub async fn execute_with_events(
    plan: SetStatePlanReport,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<SetStateExecutionReport> {
    let projects = load_projects_config(plan.root.as_str());
    let workflow = load_workflow_config(plan.root.as_str());
    let options = resolve_ado_options(&projects, &workflow, plan.project.as_str())?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::Authenticating {
            project: Some(plan.project.clone()),
        },
    );
    let token = require_token(load_auth_options(Some(plan.root.as_str()))?).await?;
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::UpdatingWorkItemState {
            ids: plan.ids.clone(),
            state: plan.state.clone(),
        },
    );
    let mut updated = Vec::new();
    let ado_ids = ado_work_item_id_values(&plan.ids);
    for (id, ado_id) in plan.ids.iter().zip(ado_ids.iter()) {
        let options = options.clone();
        let state = plan.state.clone();
        let history = plan.history.clone();
        let token = token.clone();
        let id_for_update = ado_id.clone();
        run_blocking_ado(move || {
            update_work_item_state_authenticated(
                &options,
                &id_for_update,
                state.as_str(),
                history.as_str(),
                &token,
            )
        })
        .await?;
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::UpdatedWorkItemState {
                id: id.clone(),
                state: plan.state.clone(),
            },
        );
        updated.push(SetStateUpdate {
            id: id.clone(),
            state: plan.state.clone(),
        });
    }

    Ok(SetStateExecutionReport {
        plan,
        events,
        updated,
    })
}

fn push_event(
    events: &mut Vec<AdoActionEvent>,
    emit: &mut impl FnMut(AdoActionEvent),
    event: AdoActionEvent,
) {
    emit(event.clone());
    events.push(event);
}
