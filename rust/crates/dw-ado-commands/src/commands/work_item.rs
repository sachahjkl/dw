use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{WorkItemSnapshot, query_work_item_snapshots};
use dw_config::{load_projects_config, load_workflow_config, resolve_root};

#[derive(Debug, Clone)]
pub struct WorkItemArgs {
    pub id: String,
    pub root: Option<String>,
    pub project: Option<String>,
    pub json: bool,
}

pub fn handle(args: WorkItemArgs) -> Result<()> {
    let WorkItemArgs {
        id,
        root,
        project,
        json,
    } = args;
    let root = resolve_root(root.as_deref());
    let project_key =
        project.ok_or_else(|| anyhow::anyhow!("ado work-item requiert --project configure."))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let options = resolve_ado_options(&projects, &workflow, &project_key)?;
    let token = require_token(load_auth_options(Some(&root))?)?;
    let ids = parse_work_item_ids(&id)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let items = runtime.block_on(query_work_item_snapshots(&options, &ids, &token))?;
    print_work_item_snapshots(&items, &project_key, json)?;
    Ok(())
}

pub(super) fn parse_work_item_ids(raw: &str) -> Result<Vec<i32>> {
    let mut ids = Vec::new();
    for part in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let id = part
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Work item invalide: {part}"))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }
    Ok(ids)
}

pub(crate) fn parse_work_item_ids_as_strings(raw: &str) -> Result<Vec<String>> {
    Ok(parse_work_item_ids(raw)?
        .into_iter()
        .map(|id| id.to_string())
        .collect())
}

fn print_work_item_snapshots(items: &[WorkItemSnapshot], project: &str, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
        return Ok(());
    }

    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            println!();
            println!("---");
        }
        println!("#{}", item.id);
        println!("Type: {}", item.kind.as_deref().unwrap_or("inconnu"));
        println!("Etat: {}", item.state.as_deref().unwrap_or("inconnu"));
        println!("Titre: {}", item.title.as_deref().unwrap_or("inconnu"));
        println!();
        println!(
            "Contexte complet: dw ado context {} --project {}",
            item.id, project
        );
    }
    Ok(())
}
