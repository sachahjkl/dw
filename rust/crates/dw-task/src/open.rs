use anyhow::Result;
use dw_agent::{AgentOpenRequest, build_open_launch};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_workspace::{
    read_manifest_path, resolve_open_target, resolve_workspace, task_current, task_list,
    task_status,
};
use inquire::Select;
use std::io::IsTerminal;

use crate::render::{print_styled, print_styled_lines};

mod render;

pub struct OpenWorkspaceArgs {
    pub workspace: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub positional_work_item: Option<String>,
    pub r#continue: bool,
    pub repo: Option<String>,
    pub agent: Option<String>,
    pub json: bool,
    pub root: Option<String>,
}

pub fn status(root: Option<String>) {
    let root = resolve_root(root.as_deref());
    let items = task_status(&root);
    print_styled_lines(&render::task_status_lines(&root, &items));
}

pub fn list(
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
        print_styled("Aucun workspace task trouvé.");
    } else {
        print_styled_lines(&render::task_list_lines(&items));
    }
    Ok(())
}

pub fn current(json: bool) -> Result<()> {
    let current = std::env::current_dir()?.display().to_string();
    let item = task_current(&current)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&item)?);
    } else {
        print_styled_lines(&render::current_workspace_lines(&item));
    }
    Ok(())
}

pub fn open_workspace(args: OpenWorkspaceArgs) -> Result<()> {
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
        return Err(anyhow::anyhow!("Aucun workspace task trouvé."));
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
        .ok_or_else(|| anyhow::anyhow!("Sélection workspace invalide"))
}

#[cfg(test)]
mod tests {
    use super::render::{current_workspace_lines, task_list_lines, task_status_lines};

    #[test]
    fn task_list_lines_render_table_and_paths() {
        let items = vec![dw_workspace::TaskListItem {
            path: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            display_work_items: "#42 Titre [Actif]".into(),
            task_id: None,
            kind: "feat".into(),
            slug: "titre".into(),
            branch_name: "feat/42-titre".into(),
            created_at: "2026-07-02T10:00:00Z".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Titre".into()),
            work_item_state: Some("Actif".into()),
            repositories: vec!["front".into()],
        }];

        let lines = task_list_lines(&items);

        assert_eq!(lines[0], "Workspaces task: 1");
        assert_eq!(lines[1], "Projet  Créé        Type   Work items");
        assert!(lines[2].contains("ha      2026-07-02  feat   #42 Titre [Actif]"));
        assert!(lines.contains(&"  Branche: feat/42-titre".into()));
        assert!(lines.contains(&"  Repos: front".into()));
        assert_eq!(lines.last().map(String::as_str), Some("  Path: /tmp/ws"));
    }

    #[test]
    fn current_workspace_lines_render_work_items() {
        let item = dw_workspace::TaskCurrentItem {
            workspace: "/tmp/ws".into(),
            project: "ha".into(),
            primary_work_item_id: "42".into(),
            work_items: vec![dw_workspace::WorkspaceWorkItem {
                id: "42".into(),
                kind: Some("Bug".into()),
                title: Some("Corriger".into()),
                state: Some("Actif".into()),
            }],
            task_id: None,
            child_task_ids: Default::default(),
            child_tasks: vec![dw_workspace::WorkspaceChildTask {
                id: "43".into(),
                repository: "front".into(),
                title: Some("Corriger front".into()),
            }],
            branch: "fix/42-corriger".into(),
            repositories: vec!["front".into(), "back".into()],
        };

        let lines = current_workspace_lines(&item);

        assert_eq!(lines[0], "Workspace courant");
        assert!(lines.contains(&"Workspace : /tmp/ws".into()));
        assert!(lines.contains(&"Projet    : ha".into()));
        assert!(lines.contains(&"Work items: #42 Corriger [Bug, Actif]".into()));
        assert!(lines.contains(&"Tâches enfants: #43 Corriger front (front)".into()));
        assert!(lines.contains(&"Repos     : front, back".into()));
    }

    #[test]
    fn task_status_lines_render_empty_and_non_empty_states() {
        let empty = task_status_lines("/tmp/root", &[]);
        assert_eq!(empty[0], "Task workspaces");
        assert!(empty.contains(&"Root      : /tmp/root".into()));
        assert!(empty.contains(&"Détectés  : 0".into()));
        assert!(empty.contains(&"Aucun workspace task trouvé.".into()));

        let filled = task_status_lines("/tmp/root", &["/tmp/root/projects/ha/tasks/ws".into()]);
        assert!(filled.contains(&"Détectés  : 1".into()));
        assert!(filled.contains(&"- /tmp/root/projects/ha/tasks/ws".into()));
    }
}
