use crate::{load_auth_options, resolve_ado_options, start::resolve_ado_repositories};
use anyhow::Result;
use dw_ado::{auth::require_token, get_work_item_ids_from_pull_requests, run_blocking_ado};
use dw_agent::{AgentOpenRequest, build_open_launch_plan};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::ExternalLaunchPlan;
use dw_workspace::{
    TaskCurrentItem, TaskListItem, read_manifest_path, resolve_open_target, resolve_workspace,
    task_current, task_list,
};

#[derive(Debug, Clone)]
pub struct OpenWorkspaceArgs {
    pub workspace: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub positional_work_item: Option<String>,
    pub pull_request: Option<String>,
    pub r#continue: bool,
    pub repo: Option<String>,
    pub agent: Option<String>,
    pub root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskStatusReport {
    pub root: String,
    pub items: Vec<TaskListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskListReport {
    pub root: String,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub items: Vec<TaskListItem>,
}

pub fn status_report(root: Option<String>) -> TaskStatusReport {
    let root = resolve_root(root.as_deref());
    let items = task_list(&root, None, None);
    TaskStatusReport { root, items }
}

pub fn list_report(
    root: Option<String>,
    project: Option<String>,
    work_item: Option<String>,
) -> TaskListReport {
    let root = resolve_root(root.as_deref());
    let items = task_list(&root, project.as_deref(), work_item.as_deref());
    TaskListReport {
        root,
        project,
        work_item,
        items,
    }
}

pub fn current_report() -> Result<TaskCurrentItem> {
    let current = std::env::current_dir()?.display().to_string();
    Ok(task_current(&current)?)
}

pub fn resolve_open_launch(args: OpenWorkspaceArgs) -> Result<ExternalLaunchPlan> {
    let OpenWorkspaceArgs {
        workspace,
        project,
        work_item,
        positional_work_item,
        pull_request,
        r#continue,
        repo,
        agent,
        root,
    } = args;
    if pull_request.is_some() {
        return Err(anyhow::anyhow!(
            "opening by PR requires the async resolver; use resolve_open_launch_async"
        ));
    }
    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace(
        &root,
        workspace.as_deref(),
        project.as_deref(),
        work_item.as_deref(),
        positional_work_item.as_deref(),
        r#continue,
    )?;
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let project_config = resolve_project(&projects, &manifest.project);
    let target = resolve_open_target(
        &workspace,
        &manifest,
        project_config.as_ref(),
        repo.as_deref(),
    )?;
    let selected_agent = agent
        .or_else(|| {
            project_config
                .as_ref()
                .and_then(|project| project.agent.as_ref().map(|agent| agent.default.clone()))
        })
        .or_else(|| workflow.agent.as_ref().map(|agent| agent.default.clone()));
    Ok(build_open_launch_plan(
        selected_agent.as_deref(),
        &AgentOpenRequest {
            root,
            workspace: target,
            r#continue,
        },
    )?)
}

pub async fn resolve_open_launch_async(mut args: OpenWorkspaceArgs) -> Result<ExternalLaunchPlan> {
    if let Some(pull_request) = args.pull_request.take() {
        let root = resolve_root(args.root.as_deref());
        let project = args.project.clone().ok_or_else(|| {
            anyhow::anyhow!("task open --pr requires --project to resolve Azure DevOps settings.")
        })?;
        let projects = load_projects_config(&root);
        let workflow = load_workflow_config(&root);
        let project_config = resolve_project(&projects, &project);
        let repositories = resolve_ado_repositories(project_config.as_ref(), args.repo.as_deref());
        if repositories.is_empty() {
            return Err(anyhow::anyhow!(
                "task open --pr requires --repo, or a project with configured Azure DevOps repositories."
            ));
        }
        let options = resolve_ado_options(&projects, &workflow, &project)?;
        let token = require_token(load_auth_options(Some(&root))?).await?;
        let work_item_ids = run_blocking_ado({
            let options = options.clone();
            let repositories = repositories.clone();
            let pull_request = pull_request.clone();
            let token = token.clone();
            move || {
                get_work_item_ids_from_pull_requests(&options, &repositories, &pull_request, &token)
            }
        })
        .await?;
        if work_item_ids.is_empty() {
            return Err(anyhow::anyhow!(
                "No work item linked to PR #{} in tested repositories: {}.",
                pull_request,
                repositories.join(", ")
            ));
        }
        args.root = Some(root);
        args.project = Some(project);
        args.work_item = Some(work_item_ids.join(","));
        args.positional_work_item = None;
    }

    resolve_open_launch(args)
}

pub fn run_external_launch(launch: &ExternalLaunchPlan) -> Result<()> {
    let status = dw_process::status(
        &launch.program,
        &launch.arguments,
        launch.working_directory.as_deref(),
        launch
            .environment
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    )?;
    if !status.success() {
        return Err(anyhow::anyhow!("agent exited with status {status}"));
    }
    Ok(())
}
