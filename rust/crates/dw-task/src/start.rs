use crate::write_workspace_agent_configs;
use anyhow::Result;
use dw_ado::{env_pat, get_work_item_snapshots};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_workspace::{
    TaskStartPlan, TaskStartRequest, execute_task_start, execute_task_start_with_work_items,
    plan_task_start,
};

use self::interactive::{interactive_repositories, interactive_start_selection};
use crate::render::print_styled_lines;

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
}
