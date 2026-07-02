use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UserSettings {
    pub root: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowConfig {
    #[serde(default, rename = "azureDevOps")]
    pub azure_dev_ops: Option<Value>,
    #[serde(default)]
    pub auth: Option<Value>,
    #[serde(default)]
    pub updates: Option<Value>,
    #[serde(default, rename = "branchPrefixes")]
    pub branch_prefixes: Map<String, Value>,
    #[serde(default)]
    pub agent: Option<AgentOptions>,
    #[serde(default, rename = "taskStart")]
    pub task_start: Option<Value>,
    #[serde(default, rename = "taskFinish")]
    pub task_finish: Option<Value>,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            azure_dev_ops: None,
            auth: None,
            updates: None,
            branch_prefixes: Map::new(),
            agent: None,
            task_start: None,
            task_finish: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectsConfig {
    #[serde(default)]
    pub projects: Map<String, Value>,
}

impl Default for ProjectsConfig {
    fn default() -> Self {
        Self {
            projects: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AgentOptions {
    #[serde(default)]
    pub default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfig {
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub repositories: Map<String, Value>,
    #[serde(rename = "includedProjects")]
    pub included_projects: Option<Vec<String>>,
    #[serde(default)]
    pub agent: Option<AgentOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RepositoryConfig {
    pub url: String,
    #[serde(rename = "defaultBranch")]
    pub default_branch: String,
    #[serde(rename = "pullRequestTargetBranch")]
    pub pull_request_target_branch: Option<String>,
    #[serde(rename = "azureDevOpsRepository")]
    pub azure_dev_ops_repository: Option<String>,
    #[serde(rename = "anchorName")]
    pub anchor_name: Option<String>,
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatabasesConfig {
    #[serde(default)]
    pub defaults: Option<Value>,
    #[serde(default)]
    pub globals: Map<String, Value>,
    #[serde(default)]
    pub projects: Map<String, Value>,
}

impl Default for DatabasesConfig {
    fn default() -> Self {
        Self {
            defaults: None,
            globals: Map::new(),
            projects: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigShow {
    pub root: String,
    pub color: String,
    #[serde(rename = "settingsPath")]
    pub settings_path: String,
    #[serde(rename = "workflowPath")]
    pub workflow_path: String,
    #[serde(rename = "projectsPath")]
    pub projects_path: String,
    #[serde(rename = "databasesPath")]
    pub databases_path: String,
    #[serde(rename = "workflowExists")]
    pub workflow_exists: bool,
    #[serde(rename = "projectsExists")]
    pub projects_exists: bool,
    #[serde(rename = "databasesExists")]
    pub databases_exists: bool,
}

pub fn default_root() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    format!("{home}/dev/dw")
}

pub fn user_config_directory() -> String {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.trim().is_empty()
    {
        return format!("{xdg}/DevWorkflow");
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    format!("{home}/.config/DevWorkflow")
}

pub fn user_settings_path() -> String {
    format!("{}/settings.json", user_config_directory())
}

pub fn load_user_settings() -> UserSettings {
    let path = user_settings_path();
    read_json::<UserSettings>(&path).unwrap_or_default()
}

pub fn resolve_root(explicit_root: Option<&str>) -> String {
    if let Some(root) = explicit_root.filter(|value| !value.trim().is_empty()) {
        return normalize_path(root);
    }

    let settings = load_user_settings();
    if let Some(root) = settings.root.filter(|value| !value.trim().is_empty()) {
        return normalize_path(&root);
    }

    normalize_path(&default_root())
}

pub fn load_workflow_config(root: &str) -> WorkflowConfig {
    let path = Path::new(root).join("config").join("workflow.json");
    read_json::<WorkflowConfig>(&path).unwrap_or_default()
}

pub fn load_projects_config(root: &str) -> ProjectsConfig {
    let path = Path::new(root).join("config").join("projects.json");
    read_json::<ProjectsConfig>(&path).unwrap_or_default()
}

pub fn resolve_project(config: &ProjectsConfig, project: &str) -> Option<ProjectConfig> {
    resolve_project_inner(config, project, &mut Vec::new())
}

pub fn load_databases_config(root: &str) -> DatabasesConfig {
    let path = Path::new(root).join("config").join("databases.json");
    read_json::<DatabasesConfig>(&path).unwrap_or_default()
}

pub fn config_show(explicit_root: Option<&str>) -> ConfigShow {
    let root = resolve_root(explicit_root);
    let settings = load_user_settings();
    let workflow_path = Path::new(&root).join("config").join("workflow.json");
    let projects_path = Path::new(&root).join("config").join("projects.json");
    let databases_path = Path::new(&root).join("config").join("databases.json");

    ConfigShow {
        root,
        color: settings.color.unwrap_or_else(|| "auto".into()),
        settings_path: user_settings_path(),
        workflow_path: workflow_path.display().to_string(),
        projects_path: projects_path.display().to_string(),
        databases_path: databases_path.display().to_string(),
        workflow_exists: workflow_path.exists(),
        projects_exists: projects_path.exists(),
        databases_exists: databases_path.exists(),
    }
}

fn read_json<T>(path: impl AsRef<Path>) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = path.as_ref();
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<T>(&text).ok()
}

fn normalize_path(value: &str) -> String {
    let expanded = expand_home(value);
    let path = PathBuf::from(expanded);
    path.display().to_string()
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
    })
}

pub fn repository_config(project: &ProjectConfig, repository: &str) -> Option<RepositoryConfig> {
    let value = project.repositories.get(repository)?;
    serde_json::from_value(value.clone()).ok()
}

fn expand_home(value: &str) -> String {
    if let Some(stripped) = value.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
        return format!("{home}/{stripped}");
    }

    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_uses_dev_dw_suffix() {
        let value = default_root();
        assert!(value.ends_with("/dev/dw"));
    }

    #[test]
    fn config_show_reports_expected_paths() {
        let report = config_show(Some("/tmp/demo-root"));
        assert_eq!(report.root, "/tmp/demo-root");
        assert!(
            report
                .workflow_path
                .ends_with("/tmp/demo-root/config/workflow.json")
        );
        assert!(
            report
                .projects_path
                .ends_with("/tmp/demo-root/config/projects.json")
        );
        assert!(
            report
                .databases_path
                .ends_with("/tmp/demo-root/config/databases.json")
        );
    }

    #[test]
    fn resolve_project_merges_included_projects() {
        let config: ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "base": {
      "displayName": "BASE",
      "repositories": {
        "front": { "url": "", "defaultBranch": "develop" }
      }
    },
    "ha": {
      "displayName": "HA",
      "includedProjects": ["base"],
      "repositories": {
        "back": { "url": "", "defaultBranch": "main" }
      },
      "agent": { "default": "claude" }
    }
  }
}"#,
        )
        .expect("projects config should parse");

        let project = resolve_project(&config, "ha").expect("project should resolve");
        assert!(project.repositories.contains_key("front"));
        assert!(project.repositories.contains_key("back"));
        assert_eq!(project.agent.expect("agent should exist").default, "claude");
    }

    #[test]
    fn repository_config_reads_folder_override() {
        let project: ProjectConfig = serde_json::from_str(
            r#"{
  "displayName": "HA",
  "repositories": {
    "front": { "url": "", "defaultBranch": "develop", "folder": "custom-front" }
  }
}"#,
        )
        .expect("project should parse");

        let repository = repository_config(&project, "front").expect("repository should resolve");
        assert_eq!(repository.folder.as_deref(), Some("custom-front"));
    }
}
