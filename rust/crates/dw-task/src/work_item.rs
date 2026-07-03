use crate::{load_auth_options, resolve_ado_options, write_workspace_agent_configs};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::get_work_item_snapshots_authenticated;
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_ui::multiselect_optional;
use dw_workspace::{
    display_work_items, execute_work_item_update,
    parse_work_item_ids as parse_workspace_work_item_ids, plan_add_work_item_snapshots,
    plan_add_work_items, plan_remove_work_items, read_manifest_path, resolve_workspace,
};
use std::path::Path;

use crate::render::{print_styled, print_styled_lines};

#[derive(Debug, Clone)]
pub struct AddWorkItemArgs {
    pub work_item_ids: String,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub skip_ado: bool,
    pub type_name: Option<String>,
    pub title: Option<String>,
    pub state: Option<String>,
    pub execute: bool,
    pub json: bool,
}

#[derive(Debug, Clone)]
pub struct RemoveWorkItemArgs {
    pub work_item_ids: Option<String>,
    pub workspace: Option<String>,
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub r#continue: bool,
    pub positional_work_item: Option<String>,
    pub execute: bool,
    pub json: bool,
}

pub fn add(args: AddWorkItemArgs) -> Result<()> {
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
            print_styled("Tous les work items demandés sont déjà présents dans le workspace.");
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
                        item.state.as_deref().unwrap_or("état inconnu")
                    )
                })
                .collect::<Vec<_>>();
            return Err(anyhow::anyhow!(
                "Impossible d'ajouter des work items en état final: {}",
                labels.join(", ")
            ));
        }
        plan_add_work_item_snapshots(&root, &workspace, &snapshots)?
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_styled_lines(&work_item_update_plan_lines("ajout", &plan));
        if !skip_ado {
            print_styled("Work items ADO résolus:");
            print_styled(&display_work_items(&plan.work_items, true));
        }
    }
    if execute {
        let (updated, new_workspace) = execute_work_item_update(&manifest, &plan)?;
        write_workspace_agent_configs(&new_workspace, &updated)?;
        if !json {
            print_styled(&format!("Workspace mis à jour: {new_workspace}"));
        }
    }
    Ok(())
}

pub fn remove(args: RemoveWorkItemArgs) -> Result<()> {
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
    let current_manifest = read_manifest_path(
        &Path::new(&workspace)
            .join("task.json")
            .display()
            .to_string(),
    )?;
    let work_item_ids = resolve_remove_work_item_ids(work_item_ids, &current_manifest, json)?;
    let (manifest, plan) = plan_remove_work_items(&root, &workspace, &work_item_ids)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_styled_lines(&work_item_update_plan_lines("retrait", &plan));
    }
    if execute {
        let (updated, new_workspace) = execute_work_item_update(&manifest, &plan)?;
        write_workspace_agent_configs(&new_workspace, &updated)?;
        if !json {
            print_styled(&format!("Workspace mis à jour: {new_workspace}"));
        }
    }
    Ok(())
}

fn resolve_remove_work_item_ids(
    explicit: Option<String>,
    manifest: &dw_workspace::WorkspaceManifest,
    json: bool,
) -> Result<String> {
    if let Some(ids) = explicit.filter(|ids| !ids.trim().is_empty()) {
        return Ok(ids);
    }
    if json {
        return Err(anyhow::anyhow!(
            "Work items à retirer manquants. Fournir `dw task remove-work-item <ids>`."
        ));
    }

    let choices = removable_work_item_choices(manifest);
    let Some(selected) = multiselect_optional("Work items à retirer", choices)? else {
        return Err(anyhow::anyhow!(
            "Work items à retirer manquants. Fournir `dw task remove-work-item <ids>`."
        ));
    };
    if selected.is_empty() {
        return Err(anyhow::anyhow!("Aucun work item sélectionné."));
    }

    Ok(selected
        .iter()
        .map(|label| work_item_id_from_choice(label))
        .collect::<Vec<_>>()
        .join(","))
}

fn removable_work_item_choices(manifest: &dw_workspace::WorkspaceManifest) -> Vec<String> {
    manifest
        .parent_work_items()
        .iter()
        .map(display_remove_work_item_choice)
        .collect()
}

fn display_remove_work_item_choice(item: &dw_workspace::WorkspaceWorkItem) -> String {
    format!(
        "#{}{}{}{}",
        item.id,
        item.kind
            .as_ref()
            .map(|kind| format!(" [{kind}]"))
            .unwrap_or_default(),
        item.state
            .as_ref()
            .map(|state| format!(" ({state})"))
            .unwrap_or_default(),
        item.title
            .as_ref()
            .map(|title| format!(" {title}"))
            .unwrap_or_default()
    )
}

fn work_item_id_from_choice(label: &str) -> String {
    label
        .trim_start_matches('#')
        .split_whitespace()
        .next()
        .unwrap_or(label)
        .to_string()
}

fn work_item_update_plan_lines(
    action: &str,
    plan: &dw_workspace::TaskWorkItemUpdatePlan,
) -> Vec<String> {
    vec![
        "Work items workspace".into(),
        "Mode      : prévisualisation".into(),
        format!("Action    : {action}"),
        format!("Branche   : {} -> {}", plan.old_branch, plan.new_branch),
        format!("Workspace : {} -> {}", plan.workspace, plan.new_workspace),
        format!(
            "Éléments  : {}",
            plan.work_items
                .iter()
                .map(|item| format!("#{}", item.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "À faire   : dw task add-work-item/remove-work-item --execute".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_update_plan_lines_render_branch_workspace_and_ids() {
        let plan = dw_workspace::TaskWorkItemUpdatePlan {
            workspace: "/tmp/old".into(),
            new_workspace: "/tmp/new".into(),
            old_branch: "feat/1-old".into(),
            new_branch: "feat/1-2-new".into(),
            work_items: vec![
                dw_workspace::WorkspaceWorkItem {
                    id: "1".into(),
                    kind: None,
                    title: None,
                    state: None,
                },
                dw_workspace::WorkspaceWorkItem {
                    id: "2".into(),
                    kind: None,
                    title: None,
                    state: None,
                },
            ],
        };

        let lines = work_item_update_plan_lines("ajout", &plan);

        assert_eq!(lines[0], "Work items workspace");
        assert_eq!(lines[1], "Mode      : prévisualisation");
        assert_eq!(lines[2], "Action    : ajout");
        assert!(lines.contains(&"Branche   : feat/1-old -> feat/1-2-new".into()));
        assert!(lines.contains(&"Éléments  : #1, #2".into()));
        assert!(
            lines.contains(&"À faire   : dw task add-work-item/remove-work-item --execute".into())
        );
    }

    #[test]
    fn removable_work_item_choices_include_context() {
        let manifest = dw_workspace::WorkspaceManifest {
            schema: 1,
            project: "ha".into(),
            work_item_id: "1".into(),
            task_id: None,
            kind: "feature".into(),
            slug: "parent".into(),
            branch_name: "feature/1-parent".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            repositories: Vec::new(),
            status: "active".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Parent".into()),
            work_item_state: Some("Active".into()),
            child_task_ids: None,
            child_tasks: None,
            work_items: Some(vec![
                dw_workspace::WorkspaceWorkItem {
                    id: "1".into(),
                    kind: Some("User Story".into()),
                    title: Some("Parent".into()),
                    state: Some("Active".into()),
                },
                dw_workspace::WorkspaceWorkItem {
                    id: "2".into(),
                    kind: Some("Bug".into()),
                    title: Some("Secondaire".into()),
                    state: Some("New".into()),
                },
            ]),
        };

        let choices = removable_work_item_choices(&manifest);

        assert_eq!(choices[0], "#1 [User Story] (Active) Parent");
        assert_eq!(choices[1], "#2 [Bug] (New) Secondaire");
        assert_eq!(work_item_id_from_choice(&choices[1]), "2");
    }
}
