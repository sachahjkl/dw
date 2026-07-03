use crate::ado::resolve_ado_options;
use crate::simple_handlers::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{env_pat, get_work_item_snapshots, query_assigned_work_items};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_workspace::{
    TaskStartRequest, execute_task_start, execute_task_start_with_work_items, plan_task_start,
};
use inquire::{MultiSelect, Select, Text};
use std::io::IsTerminal;

#[derive(Debug, Clone)]
pub(crate) struct StartArgs {
    pub(crate) work_item_id: Option<String>,
    pub(crate) root: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) task: Option<String>,
    pub(crate) type_name: Option<String>,
    pub(crate) only: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) skip_ado: bool,
    pub(crate) json: bool,
    pub(crate) execute: bool,
}

pub(crate) fn handle(args: StartArgs) -> Result<()> {
    let StartArgs {
        work_item_id,
        root,
        project,
        task,
        type_name,
        only,
        slug,
        skip_ado,
        json,
        execute,
    } = args;
    let root = resolve_root(root.as_deref());
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let project = interactive_project(project, &projects);
    let work_item_id = interactive_work_item(
        work_item_id,
        &root,
        &projects,
        &workflow,
        project.as_deref(),
        skip_ado,
    )?;
    let only = interactive_repositories(only, &projects, project.as_deref());
    let plan = plan_task_start(TaskStartRequest {
        root: &root,
        projects: &projects,
        work_item_id: &work_item_id,
        project: project.as_deref(),
        task_id: task.as_deref(),
        type_name: type_name.as_deref(),
        only: only.as_deref(),
        slug: slug.as_deref(),
    })?;

    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else if execute {
        let manifest = if skip_ado {
            execute_task_start(&plan, None, None, None)?
        } else {
            let mut ado_options = resolve_project(&projects, &plan.project)
                .and_then(|project| project.azure_dev_ops)
                .ok_or_else(|| anyhow::anyhow!("Configuration azureDevOps manquante dans projects.json pour {}. Ajouter --skip-ado pour un start offline.", plan.project))?;
            if ado_options.project.trim().is_empty() {
                ado_options.project = plan.project.clone();
            }
            let token = env_pat()?;
            let snapshots = get_work_item_snapshots(&ado_options, &plan.work_item_ids, &token)?;
            let work_items = if snapshots.is_empty() {
                plan.work_item_ids
                    .iter()
                    .map(|id| dw_workspace::WorkspaceWorkItem {
                        id: id.clone(),
                        kind: None,
                        title: None,
                        state: None,
                    })
                    .collect()
            } else {
                snapshots
                    .into_iter()
                    .map(|snapshot| dw_workspace::WorkspaceWorkItem {
                        id: snapshot.id,
                        kind: snapshot.kind,
                        title: snapshot.title,
                        state: snapshot.state,
                    })
                    .collect()
            };
            execute_task_start_with_work_items(&plan, work_items)?
        };
        super::write_agent_configs(&plan.workspace, &manifest)?;
        println!("Workspace cree: {}", plan.workspace);
        println!("Branche cible: {}", plan.branch_name);
        println!("Repos: {}", plan.repositories.join(", "));
    } else {
        println!("Project: {}", plan.project);
        println!("Work items: {}", plan.work_item_ids.join(", "));
        println!("Slug: {}", plan.slug);
        println!("Branche cible: {}", plan.branch_name);
        println!("Workspace cible: {}", plan.workspace);
        println!("Repos: {}", plan.repositories.join(", "));
    }

    Ok(())
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

fn interactive_project(
    project: Option<String>,
    projects: &dw_config::ProjectsConfig,
) -> Option<String> {
    if project.is_some() || !std::io::stdin().is_terminal() {
        return project;
    }

    let options = projects.projects.keys().cloned().collect::<Vec<_>>();
    if options.is_empty() {
        return None;
    }

    Select::new("Projet", options).prompt().ok()
}

fn interactive_work_item(
    work_item_id: Option<String>,
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project: Option<&str>,
    skip_ado: bool,
) -> Result<String> {
    if let Some(work_item_id) = work_item_id.filter(|value| !value.trim().is_empty()) {
        return Ok(work_item_id);
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "work-item-id requis en mode non interactif"
        ));
    }

    if !skip_ado && let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        match interactive_assigned_work_item_selection(root, projects, workflow, project) {
            Ok(Some(selection)) => return Ok(selection),
            Ok(None) => {}
            Err(error) => {
                println!("Selection ADO indisponible: {error}");
                println!("Saisie manuelle du work item.");
            }
        }
    }

    Ok(Text::new("Work item ID").prompt()?)
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
    let items = items
        .into_iter()
        .filter(|item| !dw_workspace::is_final_state(item.kind.as_deref(), item.state.as_deref()))
        .collect::<Vec<_>>();
    let choices = work_item_choices(&items);
    if choices.is_empty() {
        println!("Aucun work item assigne hors etats finaux pour {project}.");
        return Ok(None);
    }

    let selected = MultiSelect::new("Work items assignes", choices).prompt()?;
    if selected.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        selected
            .into_iter()
            .map(|choice| choice.id)
            .collect::<Vec<_>>()
            .join(","),
    ))
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

fn interactive_repositories(
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

    let selected = MultiSelect::new("Repos", options).prompt().ok()?;
    if selected.is_empty() {
        None
    } else {
        Some(selected.join(","))
    }
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
}
