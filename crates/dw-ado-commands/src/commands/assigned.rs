use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    WorkItemGroup, WorkItemSnapshot, group_work_items_by_parent, is_final_state,
    query_assigned_work_items,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_core::{ActionEvent, PromptChoice, PromptSpec};
use serde::Serialize;

pub const MANUAL_WORK_ITEM_PROMPT_VALUE: &str = "__manual_work_item_id__";
pub const MANUAL_WORK_ITEM_PROMPT_LABEL: &str = "Entrer un ID manuel...";

#[derive(Debug, Clone)]
pub struct AssignedArgs {
    pub root: Option<String>,
    pub project: Option<String>,
    pub top: i32,
    pub all: bool,
    pub group_by_parent: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssignedReport {
    pub root: String,
    pub project: String,
    pub top: i32,
    #[serde(rename = "includeFinalStates")]
    pub include_final_states: bool,
    #[serde(rename = "groupByParent")]
    pub group_by_parent: bool,
    pub items: Vec<WorkItemSnapshot>,
    pub groups: Vec<WorkItemGroup>,
    pub events: Vec<String>,
}

pub async fn report(args: AssignedArgs) -> Result<AssignedReport> {
    report_with_events(args, |_| {}).await
}

pub async fn report_with_events(
    args: AssignedArgs,
    mut emit: impl FnMut(ActionEvent),
) -> Result<AssignedReport> {
    let AssignedArgs {
        root,
        project,
        top,
        all,
        group_by_parent,
    } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado assigned requiert un projet configuré."))?;
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
    push_event(
        &mut events,
        &mut emit,
        "Chargement des work items assignés...",
    );
    let items = query_assigned_work_items(&options, top.try_into().unwrap_or(20), &token).await?;
    let items = items
        .into_iter()
        .filter(|item| all || !is_final_state(item.kind.as_deref(), item.state.as_deref()))
        .collect::<Vec<_>>();
    let groups = if group_by_parent && !items.is_empty() {
        push_event(&mut events, &mut emit, "Groupement par parent ADO...");
        group_work_items_by_parent(&options, &items, &token)?
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
    events: &mut Vec<String>,
    emit: &mut impl FnMut(ActionEvent),
    message: impl Into<String>,
) {
    let message = message.into();
    emit(ActionEvent::info(message.clone()));
    events.push(message);
}

pub fn empty_assigned_message(include_final_states: bool) -> &'static str {
    if include_final_states {
        "Aucun work item assigné."
    } else {
        "Aucun work item assigné hors états finaux."
    }
}

pub fn suggested_start_ids(parent: &WorkItemSnapshot, children: &[WorkItemSnapshot]) -> String {
    let mut ids = vec![parent.id.clone()];
    for child in children {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(&child.id)) {
            ids.push(child.id.clone());
        }
    }
    ids.join(",")
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
        .map(|item| PromptChoice::new(item.id.clone(), work_item_choice_label(item)))
        .collect::<Vec<_>>();
    choices.push(PromptChoice::new(
        MANUAL_WORK_ITEM_PROMPT_VALUE,
        MANUAL_WORK_ITEM_PROMPT_LABEL,
    ));

    PromptSpec::select("assigned-work-item", "Work item Azure DevOps", choices)
        .with_help("Choisir un work item assigné hors états finaux")
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

        assert_eq!(suggested_start_ids(&parent, &children), "42,43");
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

        assert_eq!(spec.id, "assigned-work-item");
        assert_eq!(spec.choices[0].value, "42");
        assert_eq!(spec.choices[0].label, "#42 [User Story] (Actif) Parent");
        assert_eq!(spec.choices[1].value, MANUAL_WORK_ITEM_PROMPT_VALUE);
        assert_eq!(spec.choices[1].label, MANUAL_WORK_ITEM_PROMPT_LABEL);
    }

    #[test]
    fn push_event_streams_and_keeps_final_report_event() {
        let mut final_events = Vec::new();
        let mut streamed = Vec::new();

        push_event(
            &mut final_events,
            &mut |event| streamed.push(event),
            "Chargement",
        );

        assert_eq!(final_events, ["Chargement"]);
        assert_eq!(streamed[0], ActionEvent::info("Chargement"));
    }
}
