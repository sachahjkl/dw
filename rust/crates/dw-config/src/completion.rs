use crate::{load_databases_config, load_projects_config};

pub fn subcommands() -> &'static [&'static str] {
    &["show", "set-root", "set-color", "doctor"]
}

pub fn options_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "show" | "doctor" => vec!["--root", "--json"],
        "set-root" | "set-color" => Vec::new(),
        _ => Vec::new(),
    }
}

pub fn option_requires_value(option: &str) -> bool {
    matches!(option, "--root" | "--project" | "--env")
}

pub fn option_allowed(_option: &str, _selected: &[&str]) -> bool {
    true
}

pub fn project_values(root: &str) -> Vec<String> {
    load_projects_config(root)
        .projects
        .keys()
        .cloned()
        .collect::<Vec<_>>()
}

pub fn database_values(root: &str, project: Option<&str>) -> Vec<String> {
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

pub fn env_values(root: &str, project: Option<&str>) -> Vec<String> {
    database_values(root, project)
}
