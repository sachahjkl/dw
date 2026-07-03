use crate::write_workspace_agent_configs;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::update_work_item_state_authenticated;
use dw_ado::{AzureDevOpsOptions, auth::AdoToken, auth::require_token};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_workspace::{
    TaskStartOptions, TaskStartPlan, TaskStartRequest, WorkspaceWorkItem, execute_task_start,
    execute_task_start_with_work_items_and_child_tasks, plan_task_start,
    start_plan_with_child_tasks, start_state, task_start_options,
};

use self::ado::{create_start_child_tasks, load_start_work_items};
use self::interactive::{interactive_repositories, interactive_start_selection};
use crate::render::{print_styled, print_styled_lines};

mod ado;
mod interactive;

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
    pub with_active_children: bool,
    pub create_child_tasks: bool,
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
        with_active_children,
        create_child_tasks,
        json,
        execute,
    } = args;
    let root = resolve_root(root.as_deref());
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let selection =
        interactive_start_selection(work_item_id, project, &root, &projects, &workflow, skip_ado)?;
    let project = selection.project;
    let selected_work_item_id = selection.work_item_id;
    let only = interactive_repositories(only, &projects, project.as_deref());
    let ado_context = if skip_ado {
        None
    } else if with_active_children || execute {
        let project_key = project.as_deref().unwrap_or("default");
        let mut ado_options = resolve_ado_options(&projects, &workflow, project_key)?;
        if ado_options.project.trim().is_empty() {
            ado_options.project = project_key.to_string();
        }
        let token = require_token(load_auth_options(Some(&root))?)?;
        Some((ado_options, token))
    } else {
        None
    };
    let ado_work_items = if let Some((ado_options, token)) = ado_context.as_ref() {
        Some(load_start_work_items(
            ado_options,
            &selected_work_item_id,
            with_active_children,
            token,
        )?)
    } else {
        None
    };
    let planned_work_item_id = ado_work_items
        .as_ref()
        .filter(|_| with_active_children)
        .map(|items| {
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| selected_work_item_id.clone());
    let mut plan = plan_task_start(TaskStartRequest {
        root: &root,
        projects: &projects,
        work_item_id: &planned_work_item_id,
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
            let work_items = if let Some(items) = ado_work_items {
                items
            } else {
                plan.work_item_ids
                    .iter()
                    .map(|id| dw_workspace::WorkspaceWorkItem {
                        id: id.clone(),
                        kind: None,
                        title: None,
                        state: None,
                    })
                    .collect()
            };
            if let Some((ado_options, token)) = ado_context.as_ref() {
                let start_options = task_start_options(&workflow);
                let child_tasks = if create_child_tasks || start_options.create_child_tasks {
                    create_start_child_tasks(
                        ado_options,
                        token,
                        work_items.first(),
                        &plan.repositories,
                    )?
                } else {
                    Vec::new()
                };
                if !child_tasks.is_empty() {
                    plan = start_plan_with_child_tasks(plan, &child_tasks);
                }
                update_start_states(ado_options, token, &work_items, &start_options)?;
                execute_task_start_with_work_items_and_child_tasks(&plan, work_items, child_tasks)?
            } else {
                execute_task_start_with_work_items_and_child_tasks(&plan, work_items, Vec::new())?
            }
        };
        write_workspace_agent_configs(&plan.workspace, &manifest)?;
        print_styled_lines(&created_workspace_lines(&plan));
    } else {
        print_styled_lines(&planned_workspace_lines(&plan));
    }

    Ok(())
}

fn update_start_states(
    options: &AzureDevOpsOptions,
    token: &AdoToken,
    work_items: &[WorkspaceWorkItem],
    start_options: &TaskStartOptions,
) -> Result<()> {
    if !start_options.update_work_item_state {
        return Ok(());
    }

    for item in work_items {
        let Some(state) = start_state(item.kind.as_deref(), start_options) else {
            continue;
        };
        if item
            .state
            .as_deref()
            .is_some_and(|current| current.eq_ignore_ascii_case(&state))
        {
            continue;
        }
        update_work_item_state_authenticated(options, &item.id, &state, "dw task start", token)?;
        print_styled(&format!(
            "ADO item {}: état -> {}",
            display_workspace_work_item(item),
            state
        ));
    }
    Ok(())
}

fn display_workspace_work_item(item: &WorkspaceWorkItem) -> String {
    format!(
        "#{}{}{}",
        item.id,
        item.kind
            .as_ref()
            .map(|kind| format!(" [{kind}]"))
            .unwrap_or_default(),
        item.title
            .as_ref()
            .map(|title| format!(" {title}"))
            .unwrap_or_default()
    )
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
        "Relancer avec --execute pour créer le workspace.".into(),
    ]
}

fn created_workspace_lines(plan: &TaskStartPlan) -> Vec<String> {
    vec![
        format!("Workspace créé: {}", plan.workspace),
        format!("Branche cible: {}", plan.branch_name),
        format!("Repos: {}", plan.repositories.join(", ")),
        "Prochaine étape conseillée: ouvrir le workspace ou lancer l'agent.".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(lines.contains(&"Relancer avec --execute pour créer le workspace.".into()));
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

        assert_eq!(lines[0], "Workspace créé: /tmp/dw/ha/42-titre");
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Prochaine étape conseillée"))
        );
    }

    #[test]
    fn display_workspace_work_item_includes_type_and_title() {
        let item = WorkspaceWorkItem {
            id: "42".into(),
            kind: Some("Bug".into()),
            title: Some("Corriger l'export".into()),
            state: Some("Nouveau".into()),
        };

        assert_eq!(
            display_workspace_work_item(&item),
            "#42 [Bug] Corriger l'export"
        );
    }
}
