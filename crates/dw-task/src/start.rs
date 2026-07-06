use crate::write_workspace_agent_configs;
use crate::{load_auth_options, resolve_ado_options};
use anyhow::{Context, Result};
use dw_ado::{
    AzureDevOpsOptions, auth::AdoToken, auth::require_token, get_work_item_ids_from_pull_requests,
    run_blocking_ado, update_work_item_state_authenticated,
};
use dw_config::{
    load_projects_config, load_workflow_config, repository_config, resolve_project, resolve_root,
};
use dw_core::{
    AdoRepositoryName, DevWorkflowRoot, ProjectKey, PullRequestId, TaskId, TaskSlug, WorkItemId,
    WorkItemState, WorkItemTypeName, WorkspaceRepositoryName,
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
    pub work_item_ids: Vec<WorkItemId>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub task: Option<TaskId>,
    pub type_name: Option<WorkItemTypeName>,
    pub repositories: Vec<WorkspaceRepositoryName>,
    pub slug: Option<TaskSlug>,
    pub skip_ado: bool,
    pub with_active_children: bool,
    pub create_child_tasks: bool,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct StartPrArgs {
    pub pull_request_id: PullRequestId,
    pub root: Option<DevWorkflowRoot>,
    pub project: ProjectKey,
    pub repositories: Vec<WorkspaceRepositoryName>,
    pub type_name: Option<WorkItemTypeName>,
    pub slug: Option<TaskSlug>,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartPlanReport {
    pub root: DevWorkflowRoot,
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
    pub pull_request_id: PullRequestId,
    pub repositories: Vec<AdoRepositoryName>,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<WorkItemId>,
    pub start: StartPlanReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartStateUpdate {
    pub id: WorkItemId,
    pub label: String,
    #[serde(rename = "targetState")]
    pub target_state: WorkItemState,
    pub changed: bool,
}

pub async fn start_plan(args: StartArgs) -> Result<StartPlanReport> {
    let root = DevWorkflowRoot::from(resolve_root(
        args.root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let projects = load_projects_config(root.as_str());
    let workflow = load_workflow_config(root.as_str());
    if args.work_item_ids.is_empty() {
        anyhow::bail!("work-item-id requis pour construire un plan de démarrage task.");
    }
    let project = args.project.as_ref();
    let ado_context = if args.skip_ado {
        None
    } else if args.with_active_children || args.mode.executes() {
        let project_key = project.map(|project| project.as_str()).unwrap_or("default");
        let mut ado_options = resolve_ado_options(&projects, &workflow, project_key)?;
        if ado_options.project.trim().is_empty() {
            ado_options.project = project_key.to_string();
        }
        let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
        Some((ado_options, token))
    } else {
        None
    };
    let ado_work_items = if let Some((ado_options, token)) = ado_context.as_ref() {
        let ado_options = ado_options.clone();
        let token = token.clone();
        let selected_work_item_ids = args.work_item_ids.clone();
        let with_active_children = args.with_active_children;
        run_blocking_ado(move || {
            ado::load_start_work_items(
                &ado_options,
                &selected_work_item_ids,
                with_active_children,
                &token,
            )
            .map_err(|error| dw_ado::AdoError::Request(error.to_string()))
        })
        .await?
    } else {
        Vec::new()
    };
    let planned_work_item_ids = if args.with_active_children && !ado_work_items.is_empty() {
        ado_work_items
            .iter()
            .map(|item| WorkItemId::from(item.id.clone()))
            .collect::<Vec<_>>()
    } else {
        args.work_item_ids.clone()
    };
    let plan = plan_task_start(TaskStartRequest {
        root: root.as_str(),
        projects: &projects,
        work_item_ids: &planned_work_item_ids,
        project,
        task_id: args.task.as_ref().map(TaskId::as_str),
        type_name: args.type_name.as_ref().map(WorkItemTypeName::as_str),
        repositories: &args.repositories,
        slug: args.slug.as_ref().map(TaskSlug::as_str),
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
    let workflow = load_workflow_config(report.root.as_str());
    let manifest = if args.skip_ado {
        execute_task_start(&report.plan, None, None, None)?
    } else {
        let projects = load_projects_config(report.root.as_str());
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
        let token = require_token(load_auth_options(Some(report.root.as_str()))?).await?;
        let start_options = task_start_options(&workflow);
        let child_tasks = if args.create_child_tasks || start_options.create_child_tasks {
            let ado_options = ado_options.clone();
            let token = token.clone();
            let parent = work_items.first().cloned();
            let repositories = report.plan.repositories.clone();
            run_blocking_ado(move || {
                ado::create_start_child_tasks(&ado_options, &token, parent.as_ref(), &repositories)
                    .map_err(|error| dw_ado::AdoError::Request(error.to_string()))
            })
            .await?
        } else {
            Vec::new()
        };
        if !child_tasks.is_empty() {
            report.plan = start_plan_with_child_tasks(report.plan, &child_tasks);
        }
        let state_updates =
            update_start_states(&ado_options, &token, &work_items, &start_options).await?;
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
    let root = DevWorkflowRoot::from(resolve_root(
        args.root.as_ref().map(DevWorkflowRoot::as_str),
    ));
    let projects = load_projects_config(root.as_str());
    let workflow = load_workflow_config(root.as_str());
    let project_config = resolve_project(&projects, args.project.as_str());
    let ado_repositories = resolve_ado_repositories(project_config.as_ref(), &args.repositories);
    if ado_repositories.is_empty() {
        return Err(anyhow::anyhow!(
            "task start-pr requires an explicit repository, or a project with configured azureDevOpsRepository entries."
        ));
    }
    let options = resolve_ado_options(&projects, &workflow, args.project.as_str())?;
    let token = require_token(load_auth_options(Some(root.as_str()))?).await?;
    let work_item_options = options.clone();
    let work_item_repositories = ado_repositories.clone();
    let pull_request_id = args.pull_request_id.clone();
    let work_item_token = token.clone();
    let work_item_ids = tokio::task::spawn_blocking(move || {
        get_work_item_ids_from_pull_requests(
            &work_item_options,
            &work_item_repositories,
            &[pull_request_id],
            &work_item_token,
        )
    })
    .await
    .context("resolving work items linked to the PR was interrupted")??;
    if work_item_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "No work item linked to PR #{} in tested repositories: {}.",
            args.pull_request_id,
            ado_repositories.join(", ")
        ));
    }
    let workspace_repositories =
        resolve_workspace_repositories(project_config.as_ref(), &args.repositories);

    let start = start_plan(StartArgs {
        work_item_ids: work_item_ids
            .iter()
            .cloned()
            .map(WorkItemId::from)
            .collect(),
        root: Some(root.clone()),
        project: Some(args.project.clone()),
        task: None,
        type_name: args.type_name.clone(),
        repositories: workspace_repositories
            .into_iter()
            .map(WorkspaceRepositoryName::from)
            .collect(),
        slug: args.slug.clone(),
        skip_ado: false,
        with_active_children: false,
        create_child_tasks: false,
        mode: args.mode,
    })
    .await?;

    Ok(StartPrPlanReport {
        pull_request_id: args.pull_request_id,
        repositories: ado_repositories
            .into_iter()
            .map(AdoRepositoryName::from)
            .collect(),
        work_item_ids: work_item_ids.into_iter().map(WorkItemId::from).collect(),
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
            work_item_ids: report.work_item_ids.clone(),
            root: args.root.clone(),
            project: Some(args.project.clone()),
            task: None,
            type_name: args.type_name.clone(),
            repositories: args.repositories.clone(),
            slug: args.slug.clone(),
            skip_ado: false,
            with_active_children: false,
            create_child_tasks: false,
            mode: args.mode,
        },
    )
    .await
}

pub fn resolve_ado_repositories(
    project_config: Option<&dw_config::ProjectConfig>,
    repositories: &[WorkspaceRepositoryName],
) -> Vec<String> {
    if !repositories.is_empty() {
        return repositories
            .iter()
            .map(|repo| resolve_ado_repository(project_config, repo.as_str()))
            .fold(Vec::new(), push_case_insensitive_unique);
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

fn resolve_workspace_repositories(
    project_config: Option<&dw_config::ProjectConfig>,
    repositories: &[WorkspaceRepositoryName],
) -> Vec<String> {
    if !repositories.is_empty() {
        return repositories
            .iter()
            .map(|repo| resolve_workspace_repository(project_config, repo.as_str()))
            .fold(Vec::new(), push_case_insensitive_unique);
    }

    project_config
        .map(|project| project.repositories.keys().cloned().collect())
        .unwrap_or_default()
}

fn push_case_insensitive_unique(mut values: Vec<String>, value: String) -> Vec<String> {
    if !values
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&value))
    {
        values.push(value);
    }
    values
}

fn resolve_workspace_repository(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: &str,
) -> String {
    let Some(project) = project_config else {
        return repository.to_string();
    };
    if project.repositories.contains_key(repository) {
        return repository.to_string();
    }
    project
        .repositories
        .keys()
        .find_map(|key| {
            repository_config(project, key)?
                .azure_dev_ops_repository
                .as_deref()
                .is_some_and(|ado| ado.eq_ignore_ascii_case(repository))
                .then(|| key.clone())
        })
        .unwrap_or_else(|| repository.to_string())
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

async fn update_start_states(
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
            let options_for_update = options.clone();
            let token_for_update = token.clone();
            let id_for_update = item.id.clone();
            let state_for_update = state.clone();
            run_blocking_ado(move || {
                update_work_item_state_authenticated(
                    &options_for_update,
                    &id_for_update,
                    &state_for_update,
                    "task start",
                    &token_for_update,
                )
            })
            .await?;
        }
        updates.push(StartStateUpdate {
            id: WorkItemId::from(item.id.clone()),
            label,
            target_state: WorkItemState::from(state),
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
    fn pr_repository_resolution_keeps_ado_and_workspace_names_separate() {
        let project: dw_config::ProjectConfig = serde_json::from_str(
            r#"{
  "displayName": "HA",
  "repositories": {
    "front": {
      "url": "git@example.invalid/front.git",
      "defaultBranch": "develop",
      "azureDevOpsRepository": "gesco-front"
    },
    "back": {
      "url": "git@example.invalid/back.git",
      "defaultBranch": "main",
      "azureDevOpsRepository": "gesco-back"
    }
  }
}"#,
        )
        .expect("project config");

        let front = [WorkspaceRepositoryName::from("front")];
        let gesco_front = [WorkspaceRepositoryName::from("gesco-front")];
        let all_repositories: [WorkspaceRepositoryName; 0] = [];

        assert_eq!(
            resolve_ado_repositories(Some(&project), &front),
            ["gesco-front"]
        );
        assert_eq!(
            resolve_workspace_repositories(Some(&project), &gesco_front),
            ["front"]
        );
        assert_eq!(
            resolve_workspace_repositories(Some(&project), &front),
            ["front"]
        );
        assert_eq!(
            resolve_workspace_repositories(Some(&project), &all_repositories),
            ["front", "back"]
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
