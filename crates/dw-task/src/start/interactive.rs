use crate::render::print_styled;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::query_assigned_work_items;
use dw_config::{project_choices, resolve_project};
use inquire::{MultiSelect, Select, Text};
use std::io::IsTerminal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StartSelection {
    pub(super) project: Option<String>,
    pub(super) work_item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkItemChoice {
    id: String,
    label: String,
}

impl std::fmt::Display for WorkItemChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssignedProjectChoice {
    project_key: String,
    label: String,
    items: Vec<dw_ado::WorkItemSnapshot>,
}

impl std::fmt::Display for AssignedProjectChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub(super) fn interactive_start_selection(
    work_item_id: Option<String>,
    project: Option<String>,
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    skip_ado: bool,
) -> Result<StartSelection> {
    if work_item_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Ok(StartSelection {
            project,
            work_item_id: work_item_id.unwrap_or_default(),
        });
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "work-item-id requis en mode non interactif. Fournir `dw task start <id>` ou relancer dans un terminal interactif pour choisir depuis Azure DevOps."
        ));
    }

    if !skip_ado && project.is_none() {
        match interactive_assigned_project_selection(root, projects, workflow) {
            Ok(Some(selection)) => return Ok(selection),
            Ok(None) => {}
            Err(error) => {
                print_styled(&format!("Sélection ADO indisponible: {error}"));
                print_styled("Saisie manuelle du work item.");
            }
        }
    }

    let project = interactive_project(project, projects);
    let work_item_id =
        interactive_work_item(root, projects, workflow, project.as_deref(), skip_ado)?;
    Ok(StartSelection {
        project,
        work_item_id,
    })
}

pub(super) fn interactive_repositories(
    only: Option<String>,
    projects: &dw_config::ProjectsConfig,
    project: Option<&str>,
) -> Option<String> {
    if only.is_some() || !std::io::stdin().is_terminal() {
        return only;
    }

    let project = project?;
    let project_config = resolve_project(projects, project)?;
    let options = project_config
        .repositories
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if options.len() <= 1 {
        return None;
    }

    let selected = MultiSelect::new("Repositories", options).prompt().ok()?;
    if selected.is_empty() {
        None
    } else {
        Some(selected.join(","))
    }
}

fn interactive_project(
    project: Option<String>,
    projects: &dw_config::ProjectsConfig,
) -> Option<String> {
    if project.is_some() || !std::io::stdin().is_terminal() {
        return project;
    }

    let choices = project_choices(projects);
    if choices.is_empty() {
        return None;
    }

    Select::new("Projet", choices)
        .prompt()
        .ok()
        .map(|choice| choice.key)
}

fn interactive_work_item(
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project: Option<&str>,
    skip_ado: bool,
) -> Result<String> {
    if !skip_ado && let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        match interactive_assigned_work_item_selection(root, projects, workflow, project) {
            Ok(Some(selection)) => return Ok(selection),
            Ok(None) => {}
            Err(error) => {
                print_styled(&format!("Sélection ADO indisponible: {error}"));
                print_styled("Saisie manuelle du work item.");
            }
        }
    }

    Ok(Text::new("Work item ID").prompt()?)
}

fn interactive_assigned_project_selection(
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
) -> Result<Option<StartSelection>> {
    let token = require_token(load_auth_options(Some(root))?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let mut choices = Vec::new();

    for project in project_choices(projects) {
        let Ok(options) = resolve_ado_options(projects, workflow, &project.key) else {
            continue;
        };
        let items = runtime.block_on(query_assigned_work_items(&options, 50, &token))?;
        let items = active_work_items(items);
        if items.is_empty() {
            continue;
        }
        choices.push(AssignedProjectChoice {
            label: format!("{} ({} assignés)", project.label, items.len()),
            project_key: project.key,
            items,
        });
    }

    if choices.is_empty() {
        print_styled("Aucun work item assigné hors états finaux dans les projets configurés.");
        return Ok(None);
    }

    let project = Select::new("Projet avec work items assignés", choices).prompt()?;
    let selected = select_work_items(&project.items)?;
    if selected.is_empty() {
        return Ok(None);
    }

    Ok(Some(StartSelection {
        project: Some(project.project_key),
        work_item_id: selected.join(","),
    }))
}

fn interactive_assigned_work_item_selection(
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project: &str,
) -> Result<Option<String>> {
    let options = resolve_ado_options(projects, workflow, project)?;
    let token = require_token(load_auth_options(Some(root))?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let items = runtime.block_on(query_assigned_work_items(&options, 50, &token))?;
    let items = active_work_items(items);
    if items.is_empty() {
        print_styled(&format!(
            "Aucun work item assigné hors états finaux pour {project}."
        ));
        return Ok(None);
    }

    let selected = select_work_items(&items)?;
    if selected.is_empty() {
        return Ok(None);
    }

    Ok(Some(selected.join(",")))
}

fn active_work_items(items: Vec<dw_ado::WorkItemSnapshot>) -> Vec<dw_ado::WorkItemSnapshot> {
    items
        .into_iter()
        .filter(|item| !dw_workspace::is_final_state(item.kind.as_deref(), item.state.as_deref()))
        .collect()
}

fn select_work_items(items: &[dw_ado::WorkItemSnapshot]) -> Result<Vec<String>> {
    let selected = MultiSelect::new("Work items assignés", work_item_choices(items)).prompt()?;
    Ok(selected.into_iter().map(|choice| choice.id).collect())
}

fn work_item_choices(items: &[dw_ado::WorkItemSnapshot]) -> Vec<WorkItemChoice> {
    items
        .iter()
        .map(|item| WorkItemChoice {
            id: item.id.clone(),
            label: format_work_item_choice(item),
        })
        .collect()
}

fn format_work_item_choice(item: &dw_ado::WorkItemSnapshot) -> String {
    let mut label = format!("#{}", item.id);
    if let Some(kind) = item
        .kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(&format!(" [{kind}]"));
    }
    if let Some(state) = item
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(&format!(" {state}"));
    }
    if let Some(title) = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(&format!(" - {title}"));
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_choices_include_id_type_state_and_title() {
        let items = vec![dw_ado::WorkItemSnapshot {
            id: "53115".into(),
            kind: Some("Bug".into()),
            state: Some("En développement".into()),
            title: Some("Corriger le calcul".into()),
            url: None,
        }];

        let choices = work_item_choices(&items);

        assert_eq!(
            choices,
            vec![WorkItemChoice {
                id: "53115".into(),
                label: "#53115 [Bug] En développement - Corriger le calcul".into()
            }]
        );
    }

    #[test]
    fn active_work_items_excludes_final_states() {
        let items = vec![
            dw_ado::WorkItemSnapshot {
                id: "1".into(),
                kind: Some("Task".into()),
                state: Some("Valide".into()),
                title: Some("Termine".into()),
                url: None,
            },
            dw_ado::WorkItemSnapshot {
                id: "2".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Actif".into()),
                url: None,
            },
        ];

        let active = active_work_items(items);

        assert_eq!(
            active
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["2"]
        );
    }

    #[test]
    fn assigned_project_choice_displays_label_and_count() {
        let choice = AssignedProjectChoice {
            project_key: "ha".into(),
            label: "ha - Hommage Agence (2 assignés)".into(),
            items: vec![],
        };

        assert_eq!(choice.to_string(), "ha - Hommage Agence (2 assignés)");
    }
}
