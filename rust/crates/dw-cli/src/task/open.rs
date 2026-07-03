use anyhow::Result;
use dw_agent::{AgentOpenRequest, build_open_launch};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_workspace::{
    read_manifest_path, resolve_open_target, resolve_workspace, task_current, task_list,
    task_status,
};
use inquire::Select;
use std::io::IsTerminal;

pub(crate) struct OpenWorkspaceArgs {
    pub(crate) workspace: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) work_item: Option<String>,
    pub(crate) positional_work_item: Option<String>,
    pub(crate) r#continue: bool,
    pub(crate) repo: Option<String>,
    pub(crate) agent: Option<String>,
    pub(crate) json: bool,
    pub(crate) root: Option<String>,
}

pub(crate) fn status(root: Option<String>) {
    let root = resolve_root(root.as_deref());
    let items = task_status(&root);
    println!("Root: {}", root);
    println!("Workspaces detectes:");
    if items.is_empty() {
        println!("  Aucun workspace task trouve.");
    } else {
        for item in items {
            println!("  {}", item);
        }
    }
}

pub(crate) fn list(
    root: Option<String>,
    project: Option<String>,
    work_item: Option<String>,
    json: bool,
) -> Result<()> {
    let root = resolve_root(root.as_deref());
    let items = task_list(&root, project.as_deref(), work_item.as_deref());
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else if items.is_empty() {
        println!("Aucun workspace task trouve.");
    } else {
        println!("Project  WorkItems  Created     Branch");
        for item in items {
            println!(
                "{:<8} {:<8} {}  {}",
                item.project,
                item.display_work_items,
                created_date(&item.created_at),
                item.branch_name
            );
            println!("  {}", item.path);
        }
    }
    Ok(())
}

pub(crate) fn current(json: bool) -> Result<()> {
    let current = std::env::current_dir()?.display().to_string();
    let item = task_current(&current)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&item)?);
    } else {
        println!("Workspace: {}", item.workspace);
        println!("Project: {}", item.project);
        println!(
            "Work items: {}",
            format_current_work_items(&item.work_items)
        );
        println!("Branch: {}", item.branch);
        println!("Repos: {}", item.repositories.join(", "));
    }
    Ok(())
}

pub(crate) fn open_workspace(args: OpenWorkspaceArgs) -> Result<()> {
    let OpenWorkspaceArgs {
        workspace,
        project,
        work_item,
        positional_work_item,
        r#continue,
        repo,
        agent,
        json,
        root,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = if workspace.is_none() && !r#continue && std::io::stdin().is_terminal() {
        interactive_workspace_selection(
            &root,
            project.as_deref(),
            work_item.as_deref().or(positional_work_item.as_deref()),
        )?
    } else {
        resolve_workspace(
            &root,
            workspace.as_deref(),
            project.as_deref(),
            work_item.as_deref(),
            positional_work_item.as_deref(),
            r#continue,
        )?
    };
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
    let launch = build_open_launch(
        selected_agent.as_deref(),
        &AgentOpenRequest {
            root,
            workspace: target,
            r#continue,
        },
    )?;

    if json {
        println!("{}", serde_json::to_string_pretty(&launch)?);
        return Ok(());
    }

    let mut command = std::process::Command::new(&launch.file_name);
    command
        .args(&launch.arguments)
        .current_dir(&launch.working_directory);
    for (key, value) in &launch.environment {
        command.env(key, value);
    }
    let status = command.status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("agent exited with status {status}"));
    }
    Ok(())
}

fn interactive_workspace_selection(
    root: &str,
    project: Option<&str>,
    work_item: Option<&str>,
) -> Result<String> {
    let items = task_list(root, project, work_item);
    if items.is_empty() {
        return Err(anyhow::anyhow!("Aucun workspace task trouve."));
    }
    if items.len() == 1 {
        return Ok(items[0].path.clone());
    }

    let options = items
        .into_iter()
        .map(|item| {
            (
                format!(
                    "{} / {} / {} / {}",
                    item.project, item.display_work_items, item.kind, item.path
                ),
                item.path,
            )
        })
        .collect::<Vec<_>>();
    let labels = options
        .iter()
        .map(|(label, _)| label.clone())
        .collect::<Vec<_>>();
    let selected = Select::new("Workspace", labels).prompt()?;
    options
        .into_iter()
        .find(|(label, _)| *label == selected)
        .map(|(_, path)| path)
        .ok_or_else(|| anyhow::anyhow!("Workspace selection invalide"))
}

fn created_date(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}

fn format_current_work_items(items: &[dw_workspace::WorkspaceWorkItem]) -> String {
    items
        .iter()
        .map(|item| {
            let title = item.title.clone().unwrap_or_else(|| "(sans titre)".into());
            format!("#{} {}", item.id, title)
        })
        .collect::<Vec<_>>()
        .join(", ")
}
