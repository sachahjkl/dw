use crate::{load_auth_options, resolve_ado_options, start::resolve_ado_repositories};
use anyhow::Result;
use dw_ado::{auth::require_token, get_work_item_ids_from_pull_requests, run_blocking_ado};
use dw_agent::{AgentOpenRequest, build_open_launch_plan};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::{
    Agent, DevWorkflowRoot, ExternalLaunchPlan, ProjectKey, PullRequestId, WorkItemId,
    WorkspacePath, WorkspaceRepositoryName,
};
use dw_workspace::{
    TaskCurrentItem, TaskListItem, read_manifest_path, resolve_open_target,
    resolve_workspace_by_work_item_ids, task_current, task_list,
};

#[derive(Debug, Clone)]
pub struct OpenWorkspaceArgs {
    pub workspace: Option<WorkspacePath>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub pull_request: Option<PullRequestId>,
    pub r#continue: bool,
    pub repo: Option<WorkspaceRepositoryName>,
    pub agent: Option<Agent>,
    pub root: Option<DevWorkflowRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskStatusReport {
    pub root: DevWorkflowRoot,
    pub items: Vec<TaskListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskListReport {
    pub root: DevWorkflowRoot,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub items: Vec<TaskListItem>,
}

pub type TaskCurrentReport = TaskCurrentItem;

pub fn status_report(root: Option<DevWorkflowRoot>) -> TaskStatusReport {
    let root = resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
    let items = task_list(&root, None, None);
    TaskStatusReport {
        root: DevWorkflowRoot::from(root),
        items,
    }
}

pub fn list_report(
    root: Option<DevWorkflowRoot>,
    project: Option<ProjectKey>,
    work_item_ids: Vec<WorkItemId>,
) -> TaskListReport {
    let root = resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
    let items = task_list(
        &root,
        project.as_ref().map(ProjectKey::as_str),
        work_item_ids.first().map(WorkItemId::as_str),
    );
    TaskListReport {
        root: DevWorkflowRoot::from(root),
        project,
        work_item_ids,
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
        work_item_ids,
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
    let root = resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_by_work_item_ids(
        &root,
        workspace.as_ref().map(WorkspacePath::as_str),
        project.as_ref().map(ProjectKey::as_str),
        &work_item_ids,
        r#continue,
    )?;
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let project_config = resolve_project(&projects, manifest.project.as_str());
    let target = resolve_open_target(
        &workspace,
        &manifest,
        project_config.as_ref(),
        repo.as_ref().map(WorkspaceRepositoryName::as_str),
    )?;
    let selected_agent = resolve_selected_agent(agent, project_config.as_ref(), &workflow)?;
    Ok(build_open_launch_plan(
        selected_agent,
        &AgentOpenRequest {
            root: DevWorkflowRoot::from(root),
            workspace: WorkspacePath::from(target),
            r#continue,
        },
    ))
}

pub async fn resolve_open_launch_async(mut args: OpenWorkspaceArgs) -> Result<ExternalLaunchPlan> {
    if let Some(pull_request) = args.pull_request.take() {
        let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
        let project = args.project.clone().ok_or_else(|| {
            anyhow::anyhow!("task open --pr requires --project to resolve Azure DevOps settings.")
        })?;
        let projects = load_projects_config(&root);
        let workflow = load_workflow_config(&root);
        let project_config = resolve_project(&projects, project.as_str());
        let repositories = args.repo.clone().into_iter().collect::<Vec<_>>();
        let repositories = resolve_ado_repositories(project_config.as_ref(), &repositories);
        if repositories.is_empty() {
            return Err(anyhow::anyhow!(
                "task open --pr requires --repo, or a project with configured Azure DevOps repositories."
            ));
        }
        let options = resolve_ado_options(&projects, &workflow, project.as_str())?;
        let token = require_token(load_auth_options(Some(&root))?).await?;
        let work_item_ids = run_blocking_ado({
            let options = options.clone();
            let repositories = repositories.clone();
            let pull_request = pull_request.clone();
            let token = token.clone();
            move || {
                get_work_item_ids_from_pull_requests(
                    &options,
                    &repositories,
                    &[pull_request],
                    &token,
                )
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
        let resolved_work_item_ids = work_item_ids
            .into_iter()
            .map(dw_core::WorkItemId::from)
            .collect::<Vec<_>>();
        args.workspace = Some(resolve_workspace_by_work_item_ids(
            &root,
            args.workspace.as_ref().map(WorkspacePath::as_str),
            Some(project.as_str()),
            &resolved_work_item_ids,
            args.r#continue,
        )?);
        args.root = Some(DevWorkflowRoot::from(root));
        args.project = Some(project);
        args.work_item_ids = Vec::new();
    }

    resolve_open_launch(args)
}

fn resolve_selected_agent(
    explicit: Option<Agent>,
    project_config: Option<&dw_config::ProjectConfig>,
    workflow: &dw_config::WorkflowConfig,
) -> Result<Option<Agent>> {
    if explicit.is_some() {
        return Ok(explicit);
    }
    if let Some(agent) = project_config.and_then(|project| project.agent.as_ref()) {
        return parse_config_agent(&agent.default).map(Some);
    }
    if let Some(agent) = workflow.agent.as_ref() {
        return parse_config_agent(&agent.default).map(Some);
    }
    Ok(None)
}

fn parse_config_agent(value: &str) -> Result<Agent> {
    value
        .parse::<Agent>()
        .map_err(|error| anyhow::anyhow!(error))
}
