use anyhow::Result;
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
        r#continue,
        repo,
        agent,
        root,
    } = args;
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
