use crate::{read_manifest_path, task_list};

pub fn repository_values(
    root: &str,
    project: Option<&str>,
    workspace: Option<&str>,
) -> Vec<String> {
    let projects = dw_config::load_projects_config(root);
    let mut values = project
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
        values = manifest.repositories;
    }
    values.sort();
    values.dedup();
    values
}

pub fn workspace_values(root: &str, project: Option<&str>, work_item: Option<&str>) -> Vec<String> {
    task_list(root, project, work_item)
        .into_iter()
        .map(|item| item.path)
        .collect()
}

pub fn work_item_values(root: &str, project: Option<&str>) -> Vec<String> {
    let mut values = task_list(root, project, None)
        .into_iter()
        .flat_map(|item| {
            item.display_work_items
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

pub fn workspace_resolution_options(extra: &[&'static str]) -> Vec<&'static str> {
    let mut options = vec![
        "--workspace",
        "--root",
        "--project",
        "--work-item",
        "--continue",
    ];
    options.extend_from_slice(extra);
    options
}
