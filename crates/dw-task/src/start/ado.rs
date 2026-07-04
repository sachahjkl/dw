use anyhow::Result;
use dw_ado::{
    AzureDevOpsOptions, RELATION_HIERARCHY_FORWARD, WorkItemSnapshot, auth::AdoToken,
    create_child_task_authenticated, get_related_work_item_ids,
    get_work_item_snapshots_authenticated, is_final_state,
};
use dw_workspace::{WorkspaceChildTask, WorkspaceWorkItem};

pub fn load_start_work_items(
    options: &AzureDevOpsOptions,
    selected_work_item_id: &str,
    with_active_children: bool,
    token: &AdoToken,
) -> Result<Vec<dw_workspace::WorkspaceWorkItem>> {
    let selected_ids = parse_selected_work_item_ids(selected_work_item_id);
    let snapshots = get_work_item_snapshots_authenticated(options, &selected_ids, token)?;
    if snapshots.is_empty() {
        return Ok(selected_ids
            .into_iter()
            .map(|id| dw_workspace::WorkspaceWorkItem {
                id,
                kind: None,
                title: None,
                state: None,
            })
            .collect());
    }

    let child_snapshots = if with_active_children {
        let child_ids = active_child_ids(options, &snapshots, token)?;
        get_work_item_snapshots_authenticated(options, &child_ids, token)?
    } else {
        Vec::new()
    };

    Ok(merge_start_snapshots(snapshots, child_snapshots))
}

pub fn create_start_child_tasks(
    options: &AzureDevOpsOptions,
    token: &AdoToken,
    parent: Option<&WorkspaceWorkItem>,
    repositories: &[String],
) -> Result<Vec<WorkspaceChildTask>> {
    let Some(parent) = parent else {
        return Ok(Vec::new());
    };
    let parent_snapshot = WorkItemSnapshot {
        id: parent.id.clone(),
        kind: parent.kind.clone(),
        state: parent.state.clone(),
        title: parent.title.clone(),
        url: None,
    };
    let mut created = Vec::new();
    for repository in repositories {
        let title = child_task_title(
            repository,
            parent.title.as_deref().unwrap_or(parent.id.as_str()),
        );
        let result = create_child_task_authenticated(
            options,
            &parent_snapshot,
            repository,
            &title,
            "task start",
            token,
        )?;
        created.push(WorkspaceChildTask {
            repository: repository.clone(),
            id: result.id,
            title: Some(result.title),
        });
    }
    Ok(created)
}

fn active_child_ids(
    options: &AzureDevOpsOptions,
    snapshots: &[WorkItemSnapshot],
    token: &AdoToken,
) -> Result<Vec<String>> {
    let mut child_ids = Vec::new();
    for snapshot in snapshots {
        for child_id in
            get_related_work_item_ids(options, &snapshot.id, RELATION_HIERARCHY_FORWARD, token)?
        {
            if snapshots
                .iter()
                .all(|existing| !existing.id.eq_ignore_ascii_case(&child_id))
                && child_ids
                    .iter()
                    .all(|existing: &String| !existing.eq_ignore_ascii_case(&child_id))
            {
                child_ids.push(child_id);
            }
        }
    }
    Ok(child_ids)
}

pub fn merge_start_snapshots(
    mut snapshots: Vec<WorkItemSnapshot>,
    child_snapshots: Vec<WorkItemSnapshot>,
) -> Vec<dw_workspace::WorkspaceWorkItem> {
    for child in child_snapshots {
        if is_final_state(child.kind.as_deref(), child.state.as_deref()) {
            continue;
        }
        if snapshots
            .iter()
            .any(|existing| existing.id.eq_ignore_ascii_case(&child.id))
        {
            continue;
        }
        snapshots.push(child);
    }
    snapshots
        .into_iter()
        .map(|snapshot| dw_workspace::WorkspaceWorkItem {
            id: snapshot.id,
            kind: snapshot.kind,
            title: snapshot.title,
            state: snapshot.state,
        })
        .collect()
}

pub fn parse_selected_work_item_ids(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn child_task_title(repository: &str, title: &str) -> String {
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
    fn merge_start_snapshots_keeps_active_children_only() {
        let work_items = merge_start_snapshots(
            vec![WorkItemSnapshot {
                id: "42".into(),
                kind: Some("User Story".into()),
                title: Some("Parent".into()),
                state: Some("Actif".into()),
                url: None,
            }],
            vec![
                WorkItemSnapshot {
                    id: "43".into(),
                    kind: Some("Task".into()),
                    title: Some("Enfant actif".into()),
                    state: Some("Actif".into()),
                    url: None,
                },
                WorkItemSnapshot {
                    id: "44".into(),
                    kind: Some("Task".into()),
                    title: Some("Enfant terminé".into()),
                    state: Some("Clôturé".into()),
                    url: None,
                },
            ],
        );

        assert_eq!(
            work_items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["42", "43"]
        );
    }

    #[test]
    fn merge_start_snapshots_deduplicates_children() {
        let work_items = merge_start_snapshots(
            vec![WorkItemSnapshot {
                id: "42".into(),
                kind: Some("Task".into()),
                title: None,
                state: Some("Actif".into()),
                url: None,
            }],
            vec![WorkItemSnapshot {
                id: "42".into(),
                kind: Some("Task".into()),
                title: None,
                state: Some("Actif".into()),
                url: None,
            }],
        );

        assert_eq!(work_items.len(), 1);
    }

    #[test]
    fn parse_selected_work_item_ids_trims_commas() {
        assert_eq!(
            parse_selected_work_item_ids("42, 43,,44"),
            vec!["42", "43", "44"]
        );
    }

    #[test]
    fn child_task_title_uses_domain_prefix() {
        assert_eq!(
            child_task_title("front", "Créer l'écran"),
            "[FRONT] Créer l'écran"
        );
        assert_eq!(child_task_title("db", "Migrer"), "[SQL] Migrer");
        assert_eq!(child_task_title("ops", "Déployer"), "[OPS] Déployer");
    }
}
