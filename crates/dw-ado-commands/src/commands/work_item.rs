use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{WorkItemSnapshot, query_work_item_snapshots};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{AdoActionEvent, ProjectKey, WorkItemId};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct WorkItemArgs {
    pub ids: Vec<WorkItemId>,
    pub root: Option<String>,
    pub project: Option<ProjectKey>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkItemReport {
    pub root: String,
    pub project: ProjectKey,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<WorkItemId>,
    pub items: Vec<WorkItemSnapshot>,
    pub events: Vec<AdoActionEvent>,
}

pub async fn report(args: WorkItemArgs) -> Result<WorkItemReport> {
    report_with_events(args, |_| {}).await
}

pub async fn report_with_events(
    args: WorkItemArgs,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<WorkItemReport> {
    let WorkItemArgs { ids, root, project } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado work-item requiert un projet configuré."))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, project_key.as_str())?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::Authenticating {
            project: Some(project_key.clone()),
        },
    );
    let token = require_token(load_auth_options(Some(&root))?).await?;
    if ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }
    let ado_ids = work_item_ids_as_i32(&ids)?;
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::LoadingWorkItems { ids: ids.clone() },
    );
    let items = query_work_item_snapshots(&options, &ado_ids, &token).await?;
    Ok(WorkItemReport {
        root,
        project: project_key,
        requested_ids: ids,
        items,
        events,
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

pub(crate) fn ado_work_item_id_values(ids: &[WorkItemId]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

fn work_item_ids_as_i32(ids: &[WorkItemId]) -> Result<Vec<i32>> {
    let mut parsed_ids = Vec::new();
    for id in ids {
        let parsed = id
            .as_str()
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Work item invalide: {id}"))?;
        if !parsed_ids.contains(&parsed) {
            parsed_ids.push(parsed);
        }
    }
    if parsed_ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }
    Ok(parsed_ids)
}
