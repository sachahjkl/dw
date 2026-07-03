use crate::resolve_ado_options;
use anyhow::Result;
use dw_ado::{create_child_task as ado_create_child_task, env_pat, get_work_item_snapshots};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_workspace::{
    execute_add_child_task, execute_task_rename, execute_task_sync, plan_task_rename,
    read_manifest_path, requires_child_tasks, resolve_workspace,
};

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
    let token = env_pat()?;
    let ids = manifest
        .parent_work_items()
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let snapshots = get_work_item_snapshots(&options, &ids, &token)?;
    let updated = execute_task_sync(&workspace, &snapshots)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&updated)?);
    } else {
        println!("Workspace synchronise: {workspace}");
        for item in updated.parent_work_items() {
            println!(
                "ADO item {}: {} / {} / {}",
                item.id,
                item.kind.unwrap_or_else(|| "?".into()),
                item.state.unwrap_or_else(|| "?".into()),
                item.title.unwrap_or_else(|| "(sans titre)".into())
            );
        }
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
        println!("Rename dry-run:");
        println!("- slug: {} -> {}", plan.old_slug, plan.new_slug);
        println!("- branch: {} -> {}", plan.old_branch, plan.new_branch);
        println!("- workspace: {} -> {}", plan.workspace, plan.new_workspace);
        if execute {
            let _updated = execute_task_rename(&manifest, &plan)?;
            println!("Workspace renomme: {}", plan.new_workspace);
        } else {
            println!("Relancer avec --execute pour appliquer.");
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
            "Cette commande est reservee aux User Story et Anomalie."
        ));
    }
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let mut options = resolve_ado_options(&projects, &workflow, &manifest.project)?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = env_pat()?;
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
        println!(
            "Sous-tache enregistree dans le workspace: {} -> #{} {}",
            repo, result.id, result.title
        );
    }
    Ok(())
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
