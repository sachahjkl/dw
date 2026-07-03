use dw_config::{load_databases_config, load_projects_config, resolve_project};
use dw_workspace::{read_manifest_path, task_list};

pub(super) fn project_values(root: &str) -> Vec<String> {
    let mut values = load_projects_config(root)
        .projects
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    values.sort();
    values
}

pub(super) fn repository_values(
    root: &str,
    project: Option<&str>,
    words: &[String],
) -> Vec<String> {
    let projects = load_projects_config(root);
    let mut values = project
        .and_then(|project| resolve_project(&projects, project))
        .map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| {
            projects
                .projects
                .keys()
                .filter_map(|project| resolve_project(&projects, project))
                .flat_map(|project| project.repositories.keys().cloned().collect::<Vec<_>>())
                .collect()
        });
    if let Some(workspace) = option_value(words, "--workspace")
        && let Ok(manifest) = read_manifest_path(&format!("{workspace}/task.json"))
    {
        values = manifest.repositories;
    }
    values.sort();
    values.dedup();
    values
}

pub(super) fn workspace_values(
    root: &str,
    project: Option<&str>,
    work_item: Option<&str>,
) -> Vec<String> {
    task_list(root, project, work_item)
        .into_iter()
        .map(|item| item.path)
        .collect()
}

pub(super) fn work_item_values(root: &str, project: Option<&str>) -> Vec<String> {
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

pub(super) fn database_values(root: &str, project: Option<&str>) -> Vec<String> {
    let config = load_databases_config(root);
    let mut values = config.globals.keys().cloned().collect::<Vec<_>>();
    if let Some(project) = project.and_then(|project| config.projects.get(project))
        && let Some(map) = project.as_object()
    {
        values.extend(map.keys().cloned());
    }
    values.sort();
    values.dedup();
    values
}

pub(super) fn env_values(root: &str, project: Option<&str>) -> Vec<String> {
    database_values(root, project)
}

fn option_value(words: &[String], option: &str) -> Option<String> {
    words.windows(2).find_map(|pair| {
        if pair[0] == option && !pair[1].starts_with('-') {
            Some(pair[1].clone())
        } else {
            None
        }
    })
}
