use anyhow::Result;
use dw_agent::{AgentOpenRequest, build_open_launch};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_workspace::{
    read_manifest_path, resolve_open_target, resolve_workspace, task_current, task_list,
};
use std::io::IsTerminal;

use crate::render::{print_styled, print_styled_lines};
use dw_ui::select_optional;

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
    let items = task_list(&root, None, None);
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
        handle_interactive_list_action(&root, &items)?;
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
    let selected = select_optional("Workspace", labels)?
        .ok_or_else(|| anyhow::anyhow!("Sélection workspace annulée."))?;
    options
        .into_iter()
        .find(|(label, _)| *label == selected)
        .map(|(_, path)| path)
        .ok_or_else(|| anyhow::anyhow!("Sélection workspace invalide"))
}

fn handle_interactive_list_action(root: &str, items: &[dw_workspace::TaskListItem]) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        return Ok(());
    }

    let Some(workspace) = select_optional("Workspace", workspace_action_choices(items))? else {
        return Ok(());
    };
    let Some(item) = items
        .iter()
        .find(|item| workspace.ends_with(&item.path))
        .or_else(|| items.iter().find(|item| workspace.contains(&item.path)))
    else {
        return Err(anyhow::anyhow!("Sélection workspace invalide"));
    };

    let Some(action) = select_optional(
        "Action",
        vec![
            "Ne rien faire".into(),
            "Ouvrir avec l'agent configuré".into(),
            "Lancer preflight".into(),
            "Valider les handoffs".into(),
            "Prévisualiser teardown".into(),
        ],
    )?
    else {
        return Ok(());
    };

    match action.as_str() {
        "Ouvrir avec l'agent configuré" => open_workspace(OpenWorkspaceArgs {
            workspace: Some(item.path.clone()),
            project: None,
            work_item: None,
            positional_work_item: None,
            r#continue: false,
            repo: None,
            agent: None,
            json: false,
            root: Some(root.to_string()),
        }),
        "Lancer preflight" => crate::validate::preflight(crate::validate::PreflightArgs {
            workspace: Some(item.path.clone()),
            root: Some(root.to_string()),
            project: None,
            work_item: None,
            r#continue: false,
            ai_context_file: Vec::new(),
            json: false,
            positional_work_item: None,
        }),
        "Valider les handoffs" => {
            crate::validate::handoff_validate(crate::validate::HandoffValidateArgs {
                workspace: Some(item.path.clone()),
                root: Some(root.to_string()),
                project: None,
                work_item: None,
                r#continue: false,
                json: false,
                positional_work_item: None,
            })
        }
        "Prévisualiser teardown" => crate::repo::teardown(crate::repo::TeardownArgs {
            workspace: Some(item.path.clone()),
            root: Some(root.to_string()),
            project: None,
            work_item: None,
            r#continue: false,
            positional_work_item: None,
            execute: false,
            yes: false,
            json: false,
        }),
        _ => Ok(()),
    }
}

fn workspace_action_choices(items: &[dw_workspace::TaskListItem]) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            format!(
                "{} / {} / {}",
                item.project, item.display_work_items, item.path
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::render::{current_workspace_lines, task_list_lines, task_status_lines};
    use super::workspace_action_choices;

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
        assert_eq!(lines.last().map(String::as_str), Some("  Chemin: /tmp/ws"));
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
        assert_eq!(empty[0], "Workspaces task");
        assert!(empty.contains(&"Root      : /tmp/root".into()));
        assert!(empty.contains(&"Détectés  : 0".into()));
        assert!(empty.contains(&"Aucun workspace task trouvé.".into()));

        let filled = task_status_lines(
            "/tmp/root",
            &[dw_workspace::TaskListItem {
                path: "/tmp/root/projects/ha/tasks/ws".into(),
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
                repositories: vec!["front".into(), "back".into()],
            }],
        );
        assert!(filled.contains(&"Détectés  : 1".into()));
        assert!(filled.contains(&"- ha feat #42 Titre [Actif]".into()));
        assert!(filled.contains(&"  Repositories: front, back".into()));
        assert!(filled.contains(&"  Chemin      : /tmp/root/projects/ha/tasks/ws".into()));
    }

    #[test]
    fn workspace_action_choices_include_project_work_items_and_path() {
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

        assert_eq!(
            workspace_action_choices(&items),
            vec!["ha / #42 Titre [Actif] / /tmp/ws"]
        );
    }
}
