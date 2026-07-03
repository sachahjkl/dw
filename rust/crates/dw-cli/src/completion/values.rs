use dw_config::{load_databases_config, load_projects_config};

pub(super) fn project_values(root: &str) -> Vec<String> {
    let mut values = load_projects_config(root)
        .projects
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    values.sort();
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
