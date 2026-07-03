use crate::ado::{resolve_ado_options, resolve_project_key_or_prompt};
use crate::simple_handlers::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{AzureDevOpsOptions, group_work_items_by_parent, query_assigned_work_items};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};

#[derive(Debug, Clone)]
pub(crate) struct AssignedArgs {
    pub(crate) root: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) top: i32,
    pub(crate) all: bool,
    pub(crate) group_by_parent: bool,
    pub(crate) json: bool,
}

pub(crate) fn handle(args: AssignedArgs) -> Result<()> {
    let AssignedArgs {
        root,
        project,
        top,
        all,
        group_by_parent,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let projects = load_projects_config(&root);
    let project_key = resolve_project_key_or_prompt(project, &projects, "ado assigned")?;
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let token = require_token(load_auth_options(Some(&root))?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let items = runtime.block_on(query_assigned_work_items(
        &options,
        top.try_into().unwrap_or(20),
        &token,
    ))?;
    let items = items
        .into_iter()
        .filter(|item| {
            all || !dw_workspace::is_final_state(item.kind.as_deref(), item.state.as_deref())
        })
        .collect::<Vec<_>>();
    if group_by_parent {
        print_assigned_items_grouped(&options, &items, &token, &project_key, all, json)?;
    } else {
        print_assigned_items(&items, &project_key, all, json)?;
    }
    Ok(())
}

fn print_assigned_items(
    items: &[dw_ado::WorkItemSnapshot],
    project: &str,
    include_final_states: bool,
    json: bool,
) -> Result<()> {
    if items.is_empty() {
        println!(
            "{}",
            if include_final_states {
                "Aucun work item assigne."
            } else {
                "Aucun work item assigne hors etats finaux."
            }
        );
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
        return Ok(());
    }

    for item in items {
        println!(
            "#{} [{}] {} - {}",
            item.id,
            item.kind.as_deref().unwrap_or("inconnu"),
            item.state.as_deref().unwrap_or("inconnu"),
            item.title.as_deref().unwrap_or("inconnu")
        );
        println!("  Start: dw task start {} --project {}", item.id, project);
    }
    Ok(())
}

fn print_assigned_items_grouped(
    options: &AzureDevOpsOptions,
    items: &[dw_ado::WorkItemSnapshot],
    token: &dw_ado::auth::AdoToken,
    project: &str,
    include_final_states: bool,
    json: bool,
) -> Result<()> {
    if items.is_empty() {
        println!(
            "{}",
            if include_final_states {
                "Aucun work item assigne."
            } else {
                "Aucun work item assigne hors etats finaux."
            }
        );
        return Ok(());
    }

    let groups = group_work_items_by_parent(options, items, token)?;
    if json {
        let payload = groups
            .iter()
            .map(|group| {
                serde_json::json!({
                    "parent": group.parent,
                    "items": group.items,
                    "suggestedStartCommand": format!(
                        "dw task start {} --project {}",
                        suggested_start_ids(&group.parent, &group.items),
                        project
                    )
                })
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string(&payload)?);
        return Ok(());
    }

    for group in groups {
        println!(
            "#{} [{}] {} - {}",
            group.parent.id,
            group.parent.kind.as_deref().unwrap_or("(inconnu)"),
            group.parent.state.as_deref().unwrap_or("(inconnu)"),
            group.parent.title.as_deref().unwrap_or("(sans titre)")
        );
        if !group.items.is_empty() {
            println!(
                "  Start: dw task start {} --project {}",
                suggested_start_ids(&group.parent, &group.items),
                project
            );
        }
        for item in group.items {
            println!(
                "  - #{} [{}] {} - {}",
                item.id,
                item.kind.as_deref().unwrap_or("(inconnu)"),
                item.state.as_deref().unwrap_or("(inconnu)"),
                item.title.as_deref().unwrap_or("(sans titre)")
            );
        }
        println!();
    }
    Ok(())
}

fn suggested_start_ids(
    parent: &dw_ado::WorkItemSnapshot,
    children: &[dw_ado::WorkItemSnapshot],
) -> String {
    let mut ids = vec![parent.id.clone()];
    for child in children {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(&child.id)) {
            ids.push(child.id.clone());
        }
    }
    ids.join(",")
}
