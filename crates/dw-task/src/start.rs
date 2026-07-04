use crate::write_workspace_agent_configs;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::{Context, Result};
use dw_ado::update_work_item_state_authenticated;
use dw_ado::{
    AzureDevOpsOptions, auth::AdoToken, auth::require_token, get_work_item_ids_from_pull_requests,
};
use dw_config::{
    load_projects_config, load_workflow_config, repository_config, resolve_project, resolve_root,
};
use dw_workspace::{
    TaskStartOptions, TaskStartPlan, TaskStartRequest, WorkspaceChildTask, WorkspaceManifest,
    WorkspaceWorkItem, execute_task_start, execute_task_start_with_work_items_and_child_tasks,
    plan_task_start, start_plan_with_child_tasks, start_state, task_start_options,
};
use serde::{Deserialize, Serialize};

pub mod ado;

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
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct StartPrArgs {
    pub pull_request_id: String,
    pub root: Option<String>,
    pub project: String,
    pub repo: Option<String>,
    pub type_name: Option<String>,
    pub slug: Option<String>,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartPlanReport {
    pub root: String,
    pub plan: TaskStartPlan,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
    #[serde(rename = "childTasks")]
    pub child_tasks: Vec<WorkspaceChildTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartExecutionReport {
    pub plan: TaskStartPlan,
    pub manifest: WorkspaceManifest,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
    #[serde(rename = "childTasks")]
    pub child_tasks: Vec<WorkspaceChildTask>,
    #[serde(rename = "stateUpdates")]
    pub state_updates: Vec<StartStateUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartPrPlanReport {
    #[serde(rename = "pullRequestId")]
    pub pull_request_id: String,
    pub repositories: Vec<String>,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<String>,
    pub start: StartPlanReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartStateUpdate {
    pub id: String,
    pub label: String,
    #[serde(rename = "targetState")]
    pub target_state: String,
    pub changed: bool,
}

pub async fn start_plan(args: StartArgs) -> Result<StartPlanReport> {
    let root = resolve_root(args.root.as_deref());
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let selected_work_item_id = args
        .work_item_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("work-item-id requis pour construire un plan de démarrage task.")
        })?
        .to_owned();
    let project = args.project.as_deref();
    let ado_context = if args.skip_ado {
        None
    } else if args.with_active_children || args.mode.executes() {
        let project_key = project.unwrap_or("default");
        let mut ado_options = resolve_ado_options(&projects, &workflow, project_key)?;
        if ado_options.project.trim().is_empty() {
            ado_options.project = project_key.to_string();
        }
        let token = require_token(load_auth_options(Some(&root))?).await?;
        Some((ado_options, token))
    } else {
        None
    };
    let ado_work_items = if let Some((ado_options, token)) = ado_context.as_ref() {
        ado::load_start_work_items(
            ado_options,
            &selected_work_item_id,
            args.with_active_children,
            token,
        )?
    } else {
        Vec::new()
    };
    let planned_work_item_id = if args.with_active_children && !ado_work_items.is_empty() {
        ado_work_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        selected_work_item_id
    };
    let plan = plan_task_start(TaskStartRequest {
        root: &root,
        projects: &projects,
        work_item_id: &planned_work_item_id,
        project,
        task_id: args.task.as_deref(),
        type_name: args.type_name.as_deref(),
        only: args.only.as_deref(),
        slug: args.slug.as_deref(),
    })?;

    Ok(StartPlanReport {
        root,
        plan,
        work_items: ado_work_items,
        child_tasks: Vec::new(),
    })
}

pub async fn execute_start(
    mut report: StartPlanReport,
    args: &StartArgs,
) -> Result<StartExecutionReport> {
    let workflow = load_workflow_config(&report.root);
    let manifest = if args.skip_ado {
        execute_task_start(&report.plan, None, None, None)?
    } else {
        let projects = load_projects_config(&report.root);
        let mut work_items = report.work_items.clone();
        if work_items.is_empty() {
            work_items = report
                .plan
                .work_item_ids
                .iter()
                .map(|id| WorkspaceWorkItem {
                    id: id.clone(),
                    kind: None,
                    title: None,
                    state: None,
                })
                .collect();
        }
        let project_key = if report.plan.project.trim().is_empty() {
            "default"
        } else {
            &report.plan.project
        };
        let mut ado_options = resolve_ado_options(&projects, &workflow, project_key)?;
        if ado_options.project.trim().is_empty() {
            ado_options.project = report.plan.project.clone();
        }
        let token = require_token(load_auth_options(Some(&report.root))?).await?;
        let start_options = task_start_options(&workflow);
        let child_tasks = if args.create_child_tasks || start_options.create_child_tasks {
            ado::create_start_child_tasks(
                &ado_options,
                &token,
                work_items.first(),
                &report.plan.repositories,
            )?
        } else {
            Vec::new()
        };
        if !child_tasks.is_empty() {
            report.plan = start_plan_with_child_tasks(report.plan, &child_tasks);
        }
        let state_updates = update_start_states(&ado_options, &token, &work_items, &start_options)?;
        let manifest = execute_task_start_with_work_items_and_child_tasks(
            &report.plan,
            work_items.clone(),
            child_tasks.clone(),
        )?;
        write_workspace_agent_configs(&report.plan.workspace, &manifest)?;
        return Ok(StartExecutionReport {
            plan: report.plan,
            manifest,
            work_items,
            child_tasks,
            state_updates,
        });
    };
    write_workspace_agent_configs(&report.plan.workspace, &manifest)?;
    Ok(StartExecutionReport {
        plan: report.plan,
        manifest,
        work_items: Vec::new(),
        child_tasks: Vec::new(),
        state_updates: Vec::new(),
    })
}

pub async fn start_pr_plan(args: StartPrArgs) -> Result<StartPrPlanReport> {
    let root = resolve_root(args.root.as_deref());
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let project_config = resolve_project(&projects, &args.project);
    let repositories = resolve_ado_repositories(project_config.as_ref(), args.repo.as_deref());
    if repositories.is_empty() {
        return Err(anyhow::anyhow!(
            "task start-pr requiert un repository explicite, ou un projet avec des azureDevOpsRepository configurés."
        ));
    }
    let options = resolve_ado_options(&projects, &workflow, &args.project)?;
    let token = require_token(load_auth_options(Some(&root))?).await?;
    let work_item_options = options.clone();
    let work_item_repositories = repositories.clone();
    let pull_request_id = args.pull_request_id.clone();
    let work_item_token = token.clone();
    let work_item_ids = tokio::task::spawn_blocking(move || {
        get_work_item_ids_from_pull_requests(
            &work_item_options,
            &work_item_repositories,
            &pull_request_id,
            &work_item_token,
        )
    })
    .await
    .context("résolution des work items liés à la PR interrompue")??;
    if work_item_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "Aucun work item lié à la PR #{} dans les repositories testés: {}.",
            args.pull_request_id,
            repositories.join(", ")
        ));
    }

    let start = start_plan(StartArgs {
        work_item_id: Some(work_item_ids.join(",")),
        root: Some(root.clone()),
        project: Some(args.project.clone()),
        task: None,
        type_name: args.type_name.clone(),
        only: args.repo.clone(),
        slug: args.slug.clone(),
        skip_ado: false,
        with_active_children: false,
        create_child_tasks: false,
        mode: args.mode,
    })
    .await?;

    Ok(StartPrPlanReport {
        pull_request_id: args.pull_request_id,
        repositories,
        work_item_ids,
        start,
    })
}

pub async fn execute_start_pr(
    report: StartPrPlanReport,
    args: &StartPrArgs,
) -> Result<StartExecutionReport> {
    execute_start(
        report.start,
        &StartArgs {
            work_item_id: Some(report.work_item_ids.join(",")),
            root: args.root.clone(),
            project: Some(args.project.clone()),
            task: None,
            type_name: args.type_name.clone(),
            only: args.repo.clone(),
            slug: args.slug.clone(),
            skip_ado: false,
            with_active_children: false,
            create_child_tasks: false,
            mode: args.mode,
        },
    )
    .await
}

pub fn start_pr_fetch_line(pull_request_id: &str, repositories: &[String]) -> String {
    match repositories.len() {
        0 => format!("Résolution des work items liés à la PR #{pull_request_id}..."),
        1 => format!(
            "Résolution des work items liés à la PR #{pull_request_id} dans {}...",
            repositories[0]
        ),
        count => format!(
            "Résolution des work items liés à la PR #{pull_request_id} dans {count} repositories..."
        ),
    }
}

pub fn start_pr_resolved_line(work_item_ids: &[String]) -> String {
    match work_item_ids.len() {
        0 => "Aucun work item lié à la PR.".into(),
        1 => format!("PR liée au work item #{}.", work_item_ids[0]),
        count => format!(
            "PR liée à {count} work items: {}.",
            work_item_ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub fn resolve_ado_repositories(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: Option<&str>,
) -> Vec<String> {
    if let Some(repository) = repository.filter(|value| !value.trim().is_empty()) {
        return repository
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|repo| resolve_ado_repository(project_config, repo))
            .fold(Vec::new(), |mut repos, repo| {
                if !repos
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&repo))
                {
                    repos.push(repo);
                }
                repos
            });
    }

    project_config
        .map(|project| {
            project
                .repositories
                .keys()
                .filter_map(|key| repository_config(project, key))
                .filter_map(|repo| repo.azure_dev_ops_repository)
                .filter(|repo| !repo.trim().is_empty())
                .fold(Vec::new(), |mut repos, repo| {
                    if !repos
                        .iter()
                        .any(|existing: &String| existing.eq_ignore_ascii_case(&repo))
                    {
                        repos.push(repo);
                    }
                    repos
                })
        })
        .unwrap_or_default()
}

fn resolve_ado_repository(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: &str,
) -> String {
    project_config
        .and_then(|project| repository_config(project, repository))
        .and_then(|repo| repo.azure_dev_ops_repository)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| repository.to_string())
}

fn update_start_states(
    options: &AzureDevOpsOptions,
    token: &AdoToken,
    work_items: &[WorkspaceWorkItem],
    start_options: &TaskStartOptions,
) -> Result<Vec<StartStateUpdate>> {
    if !start_options.update_work_item_state {
        return Ok(Vec::new());
    }

    let mut updates = Vec::new();
    for item in work_items {
        let Some(state) = start_state(item.kind.as_deref(), start_options) else {
            continue;
        };
        let label = display_workspace_work_item(item);
        let changed = !item
            .state
            .as_deref()
            .is_some_and(|current| current.eq_ignore_ascii_case(&state));
        if changed {
            update_work_item_state_authenticated(options, &item.id, &state, "task start", token)?;
        }
        updates.push(StartStateUpdate {
            id: item.id.clone(),
            label,
            target_state: state,
            changed,
        });
    }
    Ok(updates)
}

pub fn display_workspace_work_item(item: &WorkspaceWorkItem) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_pr_progress_lines_include_repository_and_work_items() {
        assert_eq!(
            start_pr_fetch_line("42", &[]),
            "Résolution des work items liés à la PR #42..."
        );
        assert_eq!(
            start_pr_fetch_line("42", &["front".into()]),
            "Résolution des work items liés à la PR #42 dans front..."
        );
        assert_eq!(
            start_pr_fetch_line("42", &["front".into(), "back".into()]),
            "Résolution des work items liés à la PR #42 dans 2 repositories..."
        );
        assert_eq!(start_pr_resolved_line(&[]), "Aucun work item lié à la PR.");
        assert_eq!(
            start_pr_resolved_line(&["123".into()]),
            "PR liée au work item #123."
        );
        assert_eq!(
            start_pr_resolved_line(&["123".into(), "456".into()]),
            "PR liée à 2 work items: #123, #456."
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
