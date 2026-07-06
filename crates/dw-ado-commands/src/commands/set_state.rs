use crate::commands::work_item::parse_work_item_ids_as_strings;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{auth::require_token, run_blocking_ado, update_work_item_state_authenticated};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{AdoActionEvent, WorkItemId};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct SetStateArgs {
    pub id: String,
    pub root: Option<String>,
    pub project: Option<String>,
    pub state: String,
    pub history: Option<String>,
    pub yes: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetStatePlanReport {
    pub root: String,
    pub project: String,
    pub ids: Vec<String>,
    pub state: String,
    pub history: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetStateExecutionReport {
    pub plan: SetStatePlanReport,
    pub events: Vec<AdoActionEvent>,
    pub updated: Vec<SetStateUpdate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SetStateUpdate {
    pub id: String,
    pub state: String,
}

pub fn plan(args: SetStateArgs) -> Result<SetStatePlanReport> {
    let SetStateArgs {
        id,
        root,
        project,
        state,
        history,
        yes: _,
    } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado set-state requiert un projet configuré."))?;
    let state = state.trim().to_string();
    if state.is_empty() {
        return Err(anyhow::anyhow!("ado set-state requiert un état non vide."));
    }
    let ids = parse_work_item_ids_as_strings(&id)?;

    Ok(SetStatePlanReport {
        root,
        project: project_key,
        ids,
        state,
        history: history.unwrap_or_else(|| "ado set-state".into()),
    })
}

pub async fn execute(plan: SetStatePlanReport) -> Result<SetStateExecutionReport> {
    execute_with_events(plan, |_| {}).await
}

pub async fn execute_with_events(
    plan: SetStatePlanReport,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<SetStateExecutionReport> {
    let projects = load_projects_config(&plan.root);
    let workflow = load_workflow_config(&plan.root);
    let options = resolve_ado_options(&projects, &workflow, &plan.project)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::Authenticating {
            project: Some(plan.project.clone()),
        },
    );
    let token = require_token(load_auth_options(Some(&plan.root))?).await?;
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::UpdatingWorkItemState {
            ids: plan.ids.iter().cloned().map(WorkItemId::from).collect(),
            state: plan.state.clone(),
        },
    );
    let mut updated = Vec::new();
    for id in &plan.ids {
        let options = options.clone();
        let state = plan.state.clone();
        let history = plan.history.clone();
        let token = token.clone();
        let id_for_update = id.clone();
        run_blocking_ado(move || {
            update_work_item_state_authenticated(&options, &id_for_update, &state, &history, &token)
        })
        .await?;
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::UpdatedWorkItemState {
                id: WorkItemId::from(id.clone()),
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

pub fn set_state_confirmation_prompt(ids: &[String], project: &str, state: &str) -> String {
    format!(
        "Mettre {} work item(s) du projet {project} en état `{state}` ?\n{}",
        ids.len(),
        ids.iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn set_state_progress_line(count: usize, state: &str) -> String {
    match count {
        0 => format!("Mise à jour ADO: aucun work item à passer en `{state}`."),
        1 => format!("Mise à jour ADO: passage de 1 work item en `{state}`..."),
        count => format!("Mise à jour ADO: passage de {count} work items en `{state}`..."),
    }
}

pub fn set_state_done_line(count: usize, state: &str) -> String {
    match count {
        0 => format!("Aucun work item passé en `{state}`."),
        1 => format!("1 work item passé en `{state}`."),
        count => format!("{count} work items passés en `{state}`."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_state_messages_include_count_state_and_ids() {
        assert_eq!(
            set_state_progress_line(1, "En développement"),
            "Mise à jour ADO: passage de 1 work item en `En développement`..."
        );
        assert_eq!(
            set_state_done_line(2, "PR en attente"),
            "2 work items passés en `PR en attente`."
        );
        assert_eq!(
            set_state_confirmation_prompt(&["42".into(), "43".into()], "ha", "Actif"),
            "Mettre 2 work item(s) du projet ha en état `Actif` ?\n#42, #43"
        );
    }
}
