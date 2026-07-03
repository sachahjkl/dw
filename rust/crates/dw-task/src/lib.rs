mod agent_config;
pub mod command;
pub mod completion;
pub mod finish;
mod interactive;
pub mod lifecycle;
pub mod open;
pub mod prune;
mod render;
pub mod repo;
pub mod start;
pub mod validate;
pub mod work_item;

use anyhow::Result;
use dw_ado::{AzureDevOpsOptions, auth::AdoAuthOptions};
use dw_config::{WorkflowConfig, load_workflow_config, resolve_project, resolve_root};

pub use agent_config::write_workspace_agent_configs;

pub fn load_auth_options(root: Option<&str>) -> Result<Option<AdoAuthOptions>> {
    let root = resolve_root(root);
    let workflow = load_workflow_config(&root);
    workflow
        .auth
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

pub fn resolve_ado_options(
    projects: &dw_config::ProjectsConfig,
    workflow: &WorkflowConfig,
    project_key: &str,
) -> Result<AzureDevOpsOptions> {
    let workflow_options = workflow
        .azure_dev_ops
        .clone()
        .and_then(|value| serde_json::from_value::<AzureDevOpsOptions>(value).ok());
    let project_options =
        resolve_project(projects, project_key).and_then(|project| project.azure_dev_ops);

    match (workflow_options, project_options) {
        (Some(workflow), Some(project)) => Ok(AzureDevOpsOptions {
            organization: if project.organization.trim().is_empty() {
                workflow.organization
            } else {
                project.organization
            },
            project: if project.project.trim().is_empty() {
                workflow.project
            } else {
                project.project
            },
            api_version: if project.api_version.trim().is_empty() {
                workflow.api_version
            } else {
                project.api_version
            },
        }),
        (Some(options), None) | (None, Some(options)) => Ok(options),
        (None, None) => Err(anyhow::anyhow!(
            "Configuration azureDevOps manquante pour {}.",
            project_key
        )),
    }
}
