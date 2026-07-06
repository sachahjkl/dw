use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UserSettings {
    pub root: Option<String>,
    pub color: Option<dw_core::ConfigColorMode>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectChoice {
    pub key: String,
    pub label: String,
}

impl std::fmt::Display for ProjectChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
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
    #[serde(default, rename = "azureDevOps")]
    pub azure_dev_ops: Option<dw_ado::AzureDevOpsOptions>,
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
    #[serde(rename = "gitCredentialSecret")]
    pub git_credential_secret: Option<dw_core::SecretKey>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigShow {
    pub root: String,
    pub color: dw_core::ConfigColorMode,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDoctorReport {
    pub root: String,
    pub checks: Vec<ConfigDoctorCheck>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDoctorCheck {
    pub path: String,
    pub passed: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RootStatus {
    pub root: String,
    #[serde(rename = "initialized")]
    pub initialized: bool,
    #[serde(rename = "missingPaths")]
    pub missing_paths: Vec<String>,
}
