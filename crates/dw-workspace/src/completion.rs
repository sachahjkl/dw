use crate::{read_manifest_path, task_list};

pub fn repository_values(
    root: &str,
    project: Option<&str>,
    workspace: Option<&str>,
) -> Vec<String> {
    let projects = dw_config::load_projects_config(root);
    let values = project
        .and_then(|project| dw_config::resolve_project(&projects, project))
        .map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| {
            projects
                .projects
                .keys()
                .filter_map(|project| dw_config::resolve_project(&projects, project))
                .flat_map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
                .collect()
        });
    if let Some(workspace) = workspace
        && let Ok(manifest) = read_manifest_path(&format!("{workspace}/task.json"))
    {
        return dedup_preserving_order(manifest.repositories);
    }
    dedup_preserving_order(values)
}

pub fn workspace_values(root: &str, project: Option<&str>, work_item: Option<&str>) -> Vec<String> {
    task_list(root, project, work_item)
        .into_iter()
        .map(|item| item.path)
        .collect()
}

pub fn work_item_values(root: &str, project: Option<&str>) -> Vec<String> {
    let values = task_list(root, project, None)
        .into_iter()
        .flat_map(|item| {
            item.display_work_items
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    dedup_preserving_order(values)
}

fn dedup_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut seen = Vec::<String>::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&value))
        {
            continue;
        }
        seen.push(value.clone());
        deduped.push(value);
    }
    deduped
}
