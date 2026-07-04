use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{WorkItemSnapshot, query_work_item_snapshots};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::ActionEvent;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct WorkItemArgs {
    pub id: String,
    pub root: Option<String>,
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkItemReport {
    pub root: String,
    pub project: String,
    #[serde(rename = "requestedIds")]
    pub requested_ids: Vec<i32>,
    pub items: Vec<WorkItemSnapshot>,
    pub events: Vec<String>,
}

pub async fn report(args: WorkItemArgs) -> Result<WorkItemReport> {
    report_with_events(args, |_| {}).await
}

pub async fn report_with_events(
    args: WorkItemArgs,
    mut emit: impl FnMut(ActionEvent),
) -> Result<WorkItemReport> {
    let WorkItemArgs { id, root, project } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado work-item requiert un projet configuré."))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        format!("Connexion Azure DevOps pour le projet {project_key}..."),
    );
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let ids = parse_work_item_ids(&id)?;
    push_event(&mut events, &mut emit, work_item_fetch_line(ids.len()));
    let items = query_work_item_snapshots(&options, &ids, &token).await?;
    Ok(WorkItemReport {
        root,
        project: project_key,
        requested_ids: ids,
        items,
        events,
    })
}

fn push_event(events: &mut Vec<String>, emit: &mut impl FnMut(ActionEvent), message: String) {
    emit(ActionEvent::info(message.clone()));
    events.push(message);
}

pub(super) fn parse_work_item_ids(raw: &str) -> Result<Vec<i32>> {
    let mut ids = Vec::new();
    for part in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let id = part
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Work item invalide: {part}"))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }
    Ok(ids)
}

pub(crate) fn parse_work_item_ids_as_strings(raw: &str) -> Result<Vec<String>> {
    Ok(parse_work_item_ids(raw)?
        .into_iter()
        .map(|id| id.to_string())
        .collect())
}

pub fn work_item_fetch_line(count: usize) -> String {
    match count {
        0 => "Chargement ADO: aucun work item à résoudre.".into(),
        1 => "Chargement ADO: résolution de 1 work item...".into(),
        count => format!("Chargement ADO: résolution de {count} work items..."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
