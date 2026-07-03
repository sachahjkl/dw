mod open;
mod repo;
mod start;
mod validate;
mod work_item;

use crate::ado::resolve_ado_options;
use crate::cli::TaskCommand;
use anyhow::Result;
use dw_ado::{create_child_task, env_pat, get_work_item_snapshots};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_workspace::{
    execute_add_child_task, execute_task_rename, execute_task_sync, plan_task_rename,
    read_manifest_path, requires_child_tasks, resolve_workspace,
};

pub(crate) use open::{OpenWorkspaceArgs, open_workspace};

pub(crate) fn handle_task(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Status { root } => open::status(root),
        TaskCommand::List {
            root,
            project,
            work_item,
            json,
        } => open::list(root, project, work_item, json)?,
        TaskCommand::Current { json } => open::current(json)?,
        TaskCommand::Open {
            workspace,
            project,
            work_item,
            positional_work_item,
            r#continue,
            repo,
            agent,
            json,
            root,
        } => open::open_workspace(open::OpenWorkspaceArgs {
            workspace,
            project,
            work_item,
            positional_work_item,
            r#continue,
            repo,
            agent,
            json,
            root,
        })?,
        TaskCommand::Start {
            work_item_id,
            root,
            project,
            task,
            type_name,
            only,
            slug,
            skip_ado,
            json,
            execute,
        } => start::handle(start::StartArgs {
            work_item_id,
            root,
            project,
            task,
            type_name,
            only,
            slug,
            skip_ado,
            json,
            execute,
        })?,
        TaskCommand::Preflight {
            workspace,
            ai_context_file,
            json,
        } => validate::preflight(workspace, ai_context_file, json)?,
        TaskCommand::Sync {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        } => {
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
            let mut options = resolve_project(&projects, &manifest.project)
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
        }
        TaskCommand::Rename {
            slug,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            json,
            execute,
            positional_work_item,
        } => {
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
        }
        TaskCommand::RepoLatest {
            workspace,
            r#continue,
            only,
            root,
            json,
        } => repo::repo_latest(repo::RepoLatestArgs {
            workspace,
            r#continue,
            only,
            root,
            json,
        })?,
        TaskCommand::Commit {
            workspace,
            r#continue,
            root,
            execute,
            message,
            json,
        } => repo::commit(repo::CommitArgs {
            workspace,
            r#continue,
            root,
            execute,
            message,
            json,
        })?,
        TaskCommand::AddWorkItem {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            skip_ado,
            type_name,
            title,
            state,
            execute,
            json,
        } => work_item::add(work_item::AddWorkItemArgs {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            skip_ado,
            type_name,
            title,
            state,
            execute,
            json,
        })?,
        TaskCommand::RemoveWorkItem {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            json,
        } => work_item::remove(work_item::RemoveWorkItemArgs {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            json,
        })?,
        TaskCommand::AddRepo {
            repo,
            workspace,
            root,
            execute,
            json,
        } => repo::add_repo(repo::AddRepoArgs {
            repo,
            workspace,
            root,
            execute,
            json,
        })?,
        TaskCommand::CreateChildTask {
            repo,
            title,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        } => {
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
            let result = create_child_task(
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
        }
        TaskCommand::Finish {
            workspace,
            r#continue,
            root,
            execute,
            message,
            create_pr,
            ready,
            skip_verify,
            skip_ado,
            json,
        } => dw_task::finish::handle(dw_task::finish::FinishArgs {
            workspace,
            r#continue,
            root,
            execute,
            message,
            create_pr,
            ready,
            skip_verify,
            skip_ado,
            json,
        })?,
        TaskCommand::HandoffValidate { workspace, json } => {
            validate::handoff_validate(workspace, json)?
        }
        TaskCommand::Teardown {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            yes,
            json,
        } => repo::teardown(repo::TeardownArgs {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            yes,
            json,
        })?,
        TaskCommand::Prune {
            root,
            project,
            work_item,
            execute,
            yes,
            no_sync,
            json,
        } => dw_task::prune::handle(dw_task::prune::PruneArgs {
            root,
            project,
            work_item,
            execute,
            yes,
            no_sync,
            json,
        })?,
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

pub(super) fn write_agent_configs(
    workspace: &str,
    manifest: &dw_workspace::WorkspaceManifest,
) -> Result<()> {
    let config_files = dw_agent::workspace_config_files(&dw_agent::AgentWorkspaceConfigRequest {
        workspace: workspace.into(),
        work_items: manifest
            .parent_work_items()
            .into_iter()
            .map(|item| dw_agent::WorkspaceWorkItemRef {
                id: item.id,
                kind: item.kind,
                title: item.title,
            })
            .collect(),
        project: manifest.project.clone(),
    });
    for file in config_files {
        let path = std::path::Path::new(workspace).join(file.relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.content)?;
    }
    Ok(())
}
