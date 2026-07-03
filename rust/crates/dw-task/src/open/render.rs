pub(super) fn task_status_lines(root: &str, items: &[String]) -> Vec<String> {
    let mut lines = vec![
        "Workspaces task".into(),
        format!("Root      : {root}"),
        format!("Détectés  : {}", items.len()),
    ];

    if items.is_empty() {
        lines.push("Aucun workspace task trouvé.".into());
        return lines;
    }

    lines.push("Chemins".into());
    lines.extend(items.iter().map(|item| format!("- {item}")));
    lines
}

pub(super) fn task_list_lines(items: &[dw_workspace::TaskListItem]) -> Vec<String> {
    let mut lines = vec![
        format!("Workspaces task: {}", items.len()),
        "Projet  Créé        Type   Work items".into(),
    ];

    for item in items {
        lines.push(format!(
            "{:<7} {}  {:<6} {}",
            item.project,
            created_date(&item.created_at),
            item.kind,
            item.display_work_items
        ));
        lines.push(format!("  Branche: {}", item.branch_name));
        if !item.repositories.is_empty() {
            lines.push(format!("  Repos: {}", item.repositories.join(", ")));
        }
        lines.push(format!("  Chemin: {}", item.path));
    }

    lines
}

pub(super) fn current_workspace_lines(item: &dw_workspace::TaskCurrentItem) -> Vec<String> {
    let mut lines = vec![
        "Workspace courant".into(),
        format!("Workspace : {}", item.workspace),
        format!("Projet    : {}", item.project),
        format!("Branche   : {}", item.branch),
        format!(
            "Work items: {}",
            format_current_work_items(&item.work_items)
        ),
    ];

    if !item.child_tasks.is_empty() || !item.child_task_ids.is_empty() {
        lines.push(format!("Tâches enfants: {}", format_child_tasks(item)));
    }

    lines.push(format!("Repos     : {}", item.repositories.join(", ")));
    lines
}

fn created_date(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}

fn format_current_work_items(items: &[dw_workspace::WorkspaceWorkItem]) -> String {
    items
        .iter()
        .map(|item| {
            let title = item.title.clone().unwrap_or_else(|| "(sans titre)".into());
            let metadata = [item.kind.as_deref(), item.state.as_deref()]
                .into_iter()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>();
            if metadata.is_empty() {
                format!("#{} {}", item.id, title)
            } else {
                format!("#{} {} [{}]", item.id, title, metadata.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_child_tasks(item: &dw_workspace::TaskCurrentItem) -> String {
    if !item.child_tasks.is_empty() {
        return item
            .child_tasks
            .iter()
            .map(|task| {
                let title = task.title.clone().unwrap_or_else(|| "(sans titre)".into());
                format!("#{} {} ({})", task.id, title, task.repository)
            })
            .collect::<Vec<_>>()
            .join(", ");
    }

    item.child_task_ids
        .iter()
        .map(|(repository, id)| format!("#{id} ({repository})"))
        .collect::<Vec<_>>()
        .join(", ")
}
