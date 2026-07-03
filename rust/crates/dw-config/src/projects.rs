use crate::json::read_json;
use crate::types::{ProjectConfig, ProjectsConfig, RepositoryConfig};
use serde_json::Map;
use std::path::Path;

pub fn load_projects_config(root: &str) -> ProjectsConfig {
    let path = Path::new(root).join("config").join("projects.json");
    read_json::<ProjectsConfig>(&path).unwrap_or_default()
}

pub fn resolve_project(config: &ProjectsConfig, project: &str) -> Option<ProjectConfig> {
    resolve_project_inner(config, project, &mut Vec::new())
}

pub fn repository_config(project: &ProjectConfig, repository: &str) -> Option<RepositoryConfig> {
    let value = project.repositories.get(repository)?;
    serde_json::from_value(value.clone()).ok()
}

fn resolve_project_inner(
    config: &ProjectsConfig,
    project: &str,
    visited: &mut Vec<String>,
) -> Option<ProjectConfig> {
    if visited
        .iter()
        .any(|item| item.eq_ignore_ascii_case(project))
    {
        return None;
    }
    visited.push(project.to_string());

    let value = config.projects.get(project)?;
    let project_config: ProjectConfig = serde_json::from_value(value.clone()).ok()?;
    let mut repositories = Map::new();

    for included in project_config.included_projects.clone().unwrap_or_default() {
        let included_project = resolve_project_inner(config, &included, visited)?;
        for (key, value) in included_project.repositories {
            repositories.insert(key, value);
        }
    }

    for (key, value) in &project_config.repositories {
        repositories.insert(key.clone(), value.clone());
    }

    Some(ProjectConfig {
        display_name: project_config.display_name,
        repositories,
        included_projects: project_config.included_projects,
        agent: project_config.agent,
        azure_dev_ops: project_config.azure_dev_ops,
    })
}
