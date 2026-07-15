use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    WorkItemGroup, WorkItemSnapshot, group_work_items_by_parent, is_final_state,
    query_assigned_work_items, run_blocking_ado,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{AdoActionEvent, DevWorkflowRoot, ProjectKey, PromptChoice, PromptSpec, WorkItemId};
use serde::Serialize;

pub const MANUAL_WORK_ITEM_PROMPT_VALUE: &str = "__manual_work_item_id__";
pub const MANUAL_WORK_ITEM_PROMPT_LABEL: &str = "Enter a manual ID...";

#[derive(Debug, Clone)]
pub struct AssignedArgs {
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub top: i32,
    pub all: bool,
    pub group_by_parent: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssignedReport {
    pub root: DevWorkflowRoot,
    pub project: ProjectKey,
    pub top: i32,
    #[serde(rename = "includeFinalStates")]
    pub include_final_states: bool,
    #[serde(rename = "groupByParent")]
    pub group_by_parent: bool,
    pub items: Vec<WorkItemSnapshot>,
    pub groups: Vec<WorkItemGroup>,
    pub events: Vec<AdoActionEvent>,
}

pub async fn report(args: AssignedArgs) -> Result<AssignedReport> {
    report_with_events(args, |_| {}).await
}

pub async fn report_with_events(
    args: AssignedArgs,
    mut emit: impl FnMut(AdoActionEvent),
) -> Result<AssignedReport> {
    let AssignedArgs {
        root,
        project,
        top,
        all,
        group_by_parent,
    } = args;
    let root = DevWorkflowRoot::from(resolve_root(root.as_ref().map(DevWorkflowRoot::as_str)));
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado assigned requires a configured project."))?;
    let projects = load_projects_config(root.as_str());
    let workflow = load_workflow_config(root.as_str());
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let mut events = Vec::new();
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::Authenticating {
            project: Some(project_key.clone()),
        },
    );
    let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
    push_event(
        &mut events,
        &mut emit,
        AdoActionEvent::LoadingAssignedWorkItems {
            project: project_key.clone(),
            top,
        },
    );
    let items = query_assigned_work_items(&options, top.try_into().unwrap_or(20), &token).await?;
    let items = items
        .into_iter()
        .filter(|item| all || !is_final_state(item.kind.as_deref(), item.state.as_deref()))
        .collect::<Vec<_>>();
    let groups = if group_by_parent && !items.is_empty() {
        push_event(
            &mut events,
            &mut emit,
            AdoActionEvent::GroupingAssignedWorkItems {
                project: project_key.clone(),
            },
        );
        let options = options.clone();
        let items_for_grouping = items.clone();
        let token = token.clone();
        run_blocking_ado(move || group_work_items_by_parent(&options, &items_for_grouping, &token))
            .await?
    } else {
        Vec::new()
    };
    Ok(AssignedReport {
        root,
        project: project_key,
        top,
        include_final_states: all,
        group_by_parent,
        items,
        groups,
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

pub fn empty_assigned_message(include_final_states: bool) -> &'static str {
    if include_final_states {
        "No assigned work items."
    } else {
        "No assigned work items outside final states."
    }
}

pub fn suggested_start_ids(
    parent: &WorkItemSnapshot,
    children: &[WorkItemSnapshot],
) -> Vec<WorkItemId> {
    let mut ids = vec![parent.id.clone()];
    for child in children {
        if !ids.iter().any(|id| id == &child.id) {
            ids.push(child.id.clone());
        }
    }
    ids
}

pub fn work_item_choice_label(item: &WorkItemSnapshot) -> String {
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

pub fn assigned_work_item_prompt_spec(items: &[WorkItemSnapshot]) -> PromptSpec {
    let mut choices = items
        .iter()
        .map(|item| PromptChoice::new(item.id.as_str(), work_item_choice_label(item)))
        .collect::<Vec<_>>();
    choices.push(PromptChoice::new(
        MANUAL_WORK_ITEM_PROMPT_VALUE,
        MANUAL_WORK_ITEM_PROMPT_LABEL,
    ));

    PromptSpec::select("assigned-work-item", "Work item Azure DevOps", choices)
        .with_help("Choose an assigned work item outside final states")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_assigned_items_deduplicate_suggested_start_ids() {
        let parent = WorkItemSnapshot {
            id: "42".into(),
            kind: Some("User Story".into()),
            state: Some("Actif".into()),
            title: Some("Parent".into()),
            url: None,
        };
        let children = vec![
            WorkItemSnapshot {
                id: "42".into(),
                kind: Some("Task".into()),
                state: Some("Actif".into()),
                title: Some("Doublon".into()),
                url: None,
            },
            WorkItemSnapshot {
                id: "43".into(),
                kind: Some("Task".into()),
                state: Some("Actif".into()),
                title: Some("Enfant".into()),
                url: None,
            },
        ];

        assert_eq!(
            suggested_start_ids(&parent, &children),
            vec![WorkItemId::from("42"), WorkItemId::from("43")]
        );
    }

    #[test]
    fn assigned_work_item_prompt_spec_keeps_id_values_and_rich_labels() {
        let item = WorkItemSnapshot {
            id: "42".into(),
            kind: Some("User Story".into()),
            state: Some("Actif".into()),
            title: Some("Parent".into()),
            url: None,
        };

        let spec = assigned_work_item_prompt_spec(&[item]);

        assert_eq!(spec.id.as_str(), "assigned-work-item");
        assert_eq!(spec.choices[0].value.as_str(), "42");
        assert_eq!(spec.choices[0].label, "#42 [User Story] (Actif) Parent");
        assert_eq!(
            spec.choices[1].value.as_str(),
            MANUAL_WORK_ITEM_PROMPT_VALUE
        );
        assert_eq!(spec.choices[1].label, MANUAL_WORK_ITEM_PROMPT_LABEL);
    }

    #[test]
    fn push_event_streams_and_keeps_final_report_event() {
        let mut final_events = Vec::new();
        let mut streamed = Vec::new();

        push_event(
            &mut final_events,
            &mut |event| streamed.push(event),
            AdoActionEvent::LoadingAssignedWorkItems {
                project: ProjectKey::from("acme"),
                top: 20,
            },
        );

        assert_eq!(
            final_events,
            [AdoActionEvent::LoadingAssignedWorkItems {
                project: ProjectKey::from("acme"),
                top: 20,
            }]
        );
        assert_eq!(
            streamed[0],
            AdoActionEvent::LoadingAssignedWorkItems {
                project: ProjectKey::from("acme"),
                top: 20,
            }
        );
    }
}
