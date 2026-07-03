use crate::{load_auth_options, resolve_ado_options, write_workspace_agent_configs};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{env_pat, get_work_item_snapshots, query_assigned_work_items};
use dw_config::{
    load_projects_config, load_workflow_config, project_choices, resolve_project, resolve_root,
};
use dw_ui::TerminalTheme;
use dw_workspace::{
    TaskStartPlan, TaskStartRequest, execute_task_start, execute_task_start_with_work_items,
    plan_task_start,
};
use inquire::{MultiSelect, Select, Text};
use std::io::IsTerminal;

#[derive(Debug, Clone)]
pub struct StartArgs {
    pub work_item_id: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub type_name: Option<String>,
    pub only: Option<String>,
    pub slug: Option<String>,
    pub skip_ado: bool,
    pub json: bool,
    pub execute: bool,
}

pub fn handle(args: StartArgs) -> Result<()> {
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
    let selection =
        interactive_start_selection(work_item_id, project, &root, &projects, &workflow, skip_ado)?;
    let project = selection.project;
    let work_item_id = selection.work_item_id;
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
        write_workspace_agent_configs(&plan.workspace, &manifest)?;
        print_styled_lines(&created_workspace_lines(&plan));
    } else {
        print_styled_lines(&planned_workspace_lines(&plan));
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct StartSelection {
    project: Option<String>,
    work_item_id: String,
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

fn interactive_start_selection(
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
            "work-item-id requis en mode non interactif"
        ));
    }

    if !skip_ado && project.is_none() {
        match interactive_assigned_project_selection(root, projects, workflow) {
            Ok(Some(selection)) => return Ok(selection),
            Ok(None) => {}
            Err(error) => {
                print_styled(&format!("Selection ADO indisponible: {error}"));
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
                print_styled(&format!("Selection ADO indisponible: {error}"));
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
            label: format!("{} ({} assignes)", project.label, items.len()),
            project_key: project.key,
            items,
        });
    }

    if choices.is_empty() {
        print_styled("Aucun work item assigne hors etats finaux dans les projets configures.");
        return Ok(None);
    }

    let project = Select::new("Projet avec work items assignes", choices).prompt()?;
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
            "Aucun work item assigne hors etats finaux pour {project}."
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
    let selected = MultiSelect::new("Work items assignes", work_item_choices(items)).prompt()?;
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

fn planned_workspace_lines(plan: &TaskStartPlan) -> Vec<String> {
    vec![
        "Plan task start".into(),
        format!("Project: {}", plan.project),
        format!("Work items: {}", plan.work_item_ids.join(", ")),
        format!("Slug: {}", plan.slug),
        format!("Branche cible: {}", plan.branch_name),
        format!("Workspace cible: {}", plan.workspace),
        format!("Repos: {}", plan.repositories.join(", ")),
        "Relancer avec --execute pour creer le workspace.".into(),
    ]
}

fn created_workspace_lines(plan: &TaskStartPlan) -> Vec<String> {
    vec![
        format!("Workspace cree: {}", plan.workspace),
        format!("Branche cible: {}", plan.branch_name),
        format!("Repos: {}", plan.repositories.join(", ")),
        "Prochaine etape conseillee: ouvrir le workspace ou lancer l'agent.".into(),
    ]
}

fn print_styled_lines(lines: &[String]) {
    for line in lines {
        print_styled(line);
    }
}

fn print_styled(line: &str) {
    println!("{}", TerminalTheme::stdout_auto().style_line(line, false));
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
            label: "ha - Hommage Agence (2 assignes)".into(),
            items: vec![],
        };

        assert_eq!(choice.to_string(), "ha - Hommage Agence (2 assignes)");
    }

    #[test]
    fn planned_workspace_lines_include_execute_hint() {
        let plan = TaskStartPlan {
            project: "ha".into(),
            work_item_ids: vec!["42".into()],
            primary_work_item_id: "42".into(),
            task_id: None,
            kind: "feat".into(),
            slug: "titre".into(),
            branch_name: "feat/42-titre".into(),
            subject_name: "42-titre".into(),
            workspace: "/tmp/dw/ha/42-titre".into(),
            repositories: vec!["front".into(), "back".into()],
            repository_folders: Default::default(),
        };

        let lines = planned_workspace_lines(&plan);

        assert_eq!(lines[0], "Plan task start");
        assert!(lines.contains(&"Relancer avec --execute pour creer le workspace.".into()));
    }

    #[test]
    fn created_workspace_lines_include_next_step() {
        let plan = TaskStartPlan {
            project: "ha".into(),
            work_item_ids: vec!["42".into()],
            primary_work_item_id: "42".into(),
            task_id: None,
            kind: "feat".into(),
            slug: "titre".into(),
            branch_name: "feat/42-titre".into(),
            subject_name: "42-titre".into(),
            workspace: "/tmp/dw/ha/42-titre".into(),
            repositories: vec!["front".into()],
            repository_folders: Default::default(),
        };

        let lines = created_workspace_lines(&plan);

        assert_eq!(lines[0], "Workspace cree: /tmp/dw/ha/42-titre");
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Prochaine etape conseillee"))
        );
    }
}
