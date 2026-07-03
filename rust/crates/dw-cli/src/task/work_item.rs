use crate::ado::resolve_ado_options;
use crate::simple_handlers::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::get_work_item_snapshots_authenticated;
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_workspace::{
    display_work_items, execute_work_item_update,
    parse_work_item_ids as parse_workspace_work_item_ids, plan_add_work_item_snapshots,
    plan_add_work_items, plan_remove_work_items, read_manifest_path, resolve_workspace,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct AddWorkItemArgs {
    pub(crate) work_item_ids: String,
    pub(crate) workspace: Option<String>,
    pub(crate) root: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) work_item: Option<String>,
    pub(crate) r#continue: bool,
    pub(crate) positional_work_item: Option<String>,
    pub(crate) skip_ado: bool,
    pub(crate) type_name: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) state: Option<String>,
    pub(crate) execute: bool,
    pub(crate) json: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RemoveWorkItemArgs {
    pub(crate) work_item_ids: String,
    pub(crate) workspace: Option<String>,
    pub(crate) root: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) work_item: Option<String>,
    pub(crate) r#continue: bool,
    pub(crate) positional_work_item: Option<String>,
    pub(crate) execute: bool,
    pub(crate) json: bool,
}

pub(crate) fn add(args: AddWorkItemArgs) -> Result<()> {
    let AddWorkItemArgs {
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
    let current_manifest = read_manifest_path(
        &Path::new(&workspace)
            .join("task.json")
            .display()
            .to_string(),
    )?;
    let requested_ids = parse_workspace_work_item_ids(&work_item_ids);
    let missing_ids = requested_ids
        .iter()
        .filter(|id| !current_manifest.matches_work_item(id))
        .cloned()
        .collect::<Vec<_>>();
    if missing_ids.is_empty() {
        if !json {
            println!("Tous les work items demandes sont deja presents dans le workspace.");
        }
        return Ok(());
    }
    let (manifest, plan) = if skip_ado {
        plan_add_work_items(
            &root,
            &workspace,
            &work_item_ids,
            type_name.as_deref(),
            title.as_deref(),
            state.as_deref(),
        )?
    } else {
        let projects = load_projects_config(&root);
        let workflow = load_workflow_config(&root);
        let mut options = resolve_ado_options(&projects, &workflow, &current_manifest.project)?;
        if options.project.trim().is_empty() {
            options.project = current_manifest.project.clone();
        }
        let token = require_token(load_auth_options(Some(&root))?)?;
        let snapshots = get_work_item_snapshots_authenticated(&options, &missing_ids, &token)?;
        if snapshots.len() != missing_ids.len() {
            let found = snapshots
                .iter()
                .map(|snapshot| snapshot.id.clone())
                .collect::<Vec<_>>();
            let unresolved = missing_ids
                .iter()
                .filter(|id| {
                    !found
                        .iter()
                        .any(|found_id| found_id.eq_ignore_ascii_case(id))
                })
                .cloned()
                .collect::<Vec<_>>();
            return Err(anyhow::anyhow!(
                "Work items ADO introuvables ou inaccessibles: {}",
                unresolved.join(", ")
            ));
        }
        let final_items = snapshots
            .iter()
            .filter(|snapshot| {
                dw_workspace::is_final_state(snapshot.kind.as_deref(), snapshot.state.as_deref())
            })
            .collect::<Vec<_>>();
        if !final_items.is_empty() {
            let labels = final_items
                .iter()
                .map(|item| {
                    format!(
                        "#{} ({})",
                        item.id,
                        item.state.as_deref().unwrap_or("etat inconnu")
                    )
                })
                .collect::<Vec<_>>();
            return Err(anyhow::anyhow!(
                "Impossible d'ajouter des work items en etat final: {}",
                labels.join(", ")
            ));
        }
        plan_add_work_item_snapshots(&root, &workspace, &snapshots)?
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_work_item_update_plan("Add work-item", &plan);
        if !skip_ado {
            println!("Work items ADO resolus:");
            println!("{}", display_work_items(&plan.work_items, true));
        }
    }
    if execute {
        let (updated, new_workspace) = execute_work_item_update(&manifest, &plan)?;
        dw_task::write_workspace_agent_configs(&new_workspace, &updated)?;
        if !json {
            println!("Workspace mis a jour: {new_workspace}");
        }
    } else if !json {
        println!("Relancer avec --execute pour appliquer.");
    }
    Ok(())
}

pub(crate) fn remove(args: RemoveWorkItemArgs) -> Result<()> {
    let RemoveWorkItemArgs {
        work_item_ids,
        workspace,
        root,
        project,
        work_item,
        r#continue,
        positional_work_item,
        execute,
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
    let (manifest, plan) = plan_remove_work_items(&root, &workspace, &work_item_ids)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_work_item_update_plan("Remove work-item", &plan);
    }
    if execute {
        let (updated, new_workspace) = execute_work_item_update(&manifest, &plan)?;
        dw_task::write_workspace_agent_configs(&new_workspace, &updated)?;
        if !json {
            println!("Workspace mis a jour: {new_workspace}");
        }
    } else if !json {
        println!("Relancer avec --execute pour appliquer.");
    }
    Ok(())
}

fn print_work_item_update_plan(label: &str, plan: &dw_workspace::TaskWorkItemUpdatePlan) {
    println!("{label} dry-run:");
    println!("- branch: {} -> {}", plan.old_branch, plan.new_branch);
    println!("- workspace: {} -> {}", plan.workspace, plan.new_workspace);
    println!(
        "- work items: {}",
        plan.work_items
            .iter()
            .map(|item| format!("#{}", item.id))
            .collect::<Vec<_>>()
            .join(", ")
    );
}
