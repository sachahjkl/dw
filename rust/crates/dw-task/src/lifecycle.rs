use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::{
    auth::require_token, create_child_task_authenticated as ado_create_child_task,
    get_work_item_snapshots_authenticated,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_workspace::{
    execute_add_child_task, execute_task_rename, execute_task_sync, plan_task_rename,
    read_manifest_path, requires_child_tasks, resolve_workspace,
};

use crate::render::{print_styled, print_styled_lines};

#[derive(Debug, Clone)]
pub struct SyncArgs {
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct RenameArgs {
    pub slug: String,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub json: bool,
    pub execute: bool,
    pub positional_work_item: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateChildTaskArgs {
    pub repo: String,
    pub title: String,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub json: bool,
}

pub fn sync(args: SyncArgs) -> Result<()> {
    let SyncArgs {
        workspace,
        root,
        project,
        work_item,
        r#continue,
        positional_work_item,
        json,
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
    let mut options = dw_config::resolve_project(&projects, &manifest.project)
        .and_then(|project| project.azure_dev_ops)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Configuration azureDevOps manquante dans projects.json pour {}.",
                manifest.project
            )
        })?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = require_token(load_auth_options(Some(&root))?)?;
    let ids = manifest
        .parent_work_items()
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let snapshots = get_work_item_snapshots_authenticated(&options, &ids, &token)?;
    let updated = execute_task_sync(&workspace, &snapshots)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&updated)?);
    } else {
        print_styled_lines(&sync_lines(&workspace, &updated.parent_work_items()));
    }
    Ok(())
}

pub fn rename(args: RenameArgs) -> Result<()> {
    let RenameArgs {
        slug,
        workspace,
        root,
        project,
        work_item,
        r#continue,
        json,
        execute,
        positional_work_item,
    } = args;
    let root = resolve_root(root.as_deref());
    let projects = load_projects_config(&root);
    let workspace = resolve_workspace(
        &root,
        workspace.as_deref(),
        project.as_deref(),
        work_item.as_deref(),
        positional_work_item.as_deref(),
        r#continue,
    )?;
    let (manifest, plan) = plan_task_rename(&root, &projects, &workspace, &slug)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_styled_lines(&rename_plan_lines(&plan));
        if execute {
            let _updated = execute_task_rename(&manifest, &plan)?;
            print_styled(&format!("Workspace renommé: {}", plan.new_workspace));
        }
    }
    Ok(())
}

pub fn create_child_task(args: CreateChildTaskArgs) -> Result<()> {
    let CreateChildTaskArgs {
        repo,
        title,
        workspace,
        root,
        project,
        work_item,
        r#continue,
        positional_work_item,
        json,
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
    let parent = manifest.parent_work_items()[0].clone();
    if !requires_child_tasks(parent.kind.as_deref()) {
        return Err(anyhow::anyhow!(
            "Cette commande est réservée aux User Story et Anomalie."
        ));
    }
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let mut options = resolve_ado_options(&projects, &workflow, &manifest.project)?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = require_token(load_auth_options(Some(&root))?)?;
    let task_title = child_task_title(&repo, &title);
    let result = ado_create_child_task(
        &options,
        &dw_ado::WorkItemSnapshot {
            id: parent.id.clone(),
            kind: parent.kind.clone(),
            state: parent.state.clone(),
            title: parent.title.clone(),
            url: None,
        },
        &repo,
        &task_title,
        "dw task create-child-task",
        &token,
    )?;
    let updated =
        execute_add_child_task(&workspace, &repo, &result.id, Some(result.title.clone()))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&updated)?);
    } else {
        print_styled_lines(&child_task_lines(
            &workspace,
            &repo,
            &result.id,
            &result.title,
        ));
    }
    Ok(())
}

fn sync_lines(workspace: &str, items: &[dw_workspace::WorkspaceWorkItem]) -> Vec<String> {
    let mut lines = vec![
        "Synchronisation task".into(),
        format!("Workspace : {workspace}"),
        format!("Items     : {}", items.len()),
    ];
    if !items.is_empty() {
        lines.push(String::new());
        lines.push("Work items ADO".into());
    }
    for item in items {
        lines.push(work_item_line(item));
    }
    lines
}

fn rename_plan_lines(plan: &dw_workspace::TaskRenamePlan) -> Vec<String> {
    vec![
        "Renommage workspace".into(),
        "Mode      : prévisualisation".into(),
        format!("Slug      : {} -> {}", plan.old_slug, plan.new_slug),
        format!("Branche   : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        "À faire   : dw task rename <slug> --execute".into(),
    ]
}

fn child_task_lines(workspace: &str, repo: &str, id: &str, title: &str) -> Vec<String> {
    vec![
        "Sous-tâche ADO".into(),
        "Statut    : enregistrée dans le workspace".into(),
        format!("Workspace : {workspace}"),
        format!("Dépôt     : {repo}"),
        format!("Item      : #{id}"),
        format!("Titre     : {title}"),
    ]
}

fn work_item_line(item: &dw_workspace::WorkspaceWorkItem) -> String {
    format!(
        "#{} [{} / {}] {}",
        item.id,
        item.kind.as_deref().unwrap_or("type inconnu"),
        item.state.as_deref().unwrap_or("état inconnu"),
        item.title.as_deref().unwrap_or("(sans titre)")
    )
}

fn child_task_title(repository: &str, title: &str) -> String {
    let normalized = repository.to_ascii_lowercase();
    let prefix = match normalized.as_str() {
        "front" => "FRONT",
        "back" => "BACK",
        "sql" | "db" | "database" => "SQL",
        other => other,
    };
    format!("[{}] {}", prefix.to_ascii_uppercase(), title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_plan_lines_show_slug_branch_and_workspace() {
        let plan = dw_workspace::TaskRenamePlan {
            workspace: "/tmp/old".into(),
            new_workspace: "/tmp/new".into(),
            old_slug: "old".into(),
            new_slug: "new".into(),
            old_branch: "feat/1-old".into(),
            new_branch: "feat/1-new".into(),
        };

        let lines = rename_plan_lines(&plan);

        assert_eq!(lines[0], "Renommage workspace");
        assert!(lines.contains(&"Mode      : prévisualisation".into()));
        assert!(lines.contains(&"Slug      : old -> new".into()));
        assert!(lines.contains(&"Branche   : feat/1-old -> feat/1-new".into()));
        assert!(lines.contains(&"À faire   : dw task rename <slug> --execute".into()));
    }

    #[test]
    fn sync_lines_render_missing_ado_fields_as_unknown() {
        let lines = sync_lines(
            "/tmp/ws",
            &[dw_workspace::WorkspaceWorkItem {
                id: "42".into(),
                kind: None,
                state: None,
                title: None,
            }],
        );

        assert_eq!(lines[0], "Synchronisation task");
        assert_eq!(lines[1], "Workspace : /tmp/ws");
        assert_eq!(lines[2], "Items     : 1");
        assert_eq!(lines[4], "Work items ADO");
        assert_eq!(lines[5], "#42 [type inconnu / état inconnu] (sans titre)");
    }

    #[test]
    fn child_task_lines_render_workspace_repo_and_item() {
        let lines = child_task_lines("/tmp/ws", "front", "42", "[FRONT] Corriger");

        assert_eq!(lines[0], "Sous-tâche ADO");
        assert_eq!(lines[1], "Statut    : enregistrée dans le workspace");
        assert_eq!(lines[2], "Workspace : /tmp/ws");
        assert_eq!(lines[3], "Dépôt     : front");
        assert_eq!(lines[4], "Item      : #42");
        assert_eq!(lines[5], "Titre     : [FRONT] Corriger");
    }
}
