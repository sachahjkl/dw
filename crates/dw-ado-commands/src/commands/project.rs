use anyhow::Result;
use dw_ado::{AzureDevOpsOptions, default_api_version};
use dw_config::resolve_project;
use dw_core::ProjectKey;

pub fn resolve_ado_options(
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project_key: &ProjectKey,
) -> Result<AzureDevOpsOptions> {
    let workflow_options = workflow
        .azure_dev_ops
        .clone()
        .and_then(|value| serde_json::from_value::<AzureDevOpsOptions>(value).ok());
    let project_options =
        resolve_project(projects, project_key.as_str()).and_then(|project| project.azure_dev_ops);

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
            "Missing azureDevOps configuration for {}.",
            project_key
        )),
    }
}

pub fn resolve_cli_ado_options(
    root: &str,
    organization: Option<String>,
    project: Option<ProjectKey>,
) -> Result<AzureDevOpsOptions> {
    match (organization, project) {
        (Some(organization), Some(project)) => Ok(AzureDevOpsOptions {
            organization,
            project: project.to_string(),
            api_version: default_api_version(),
        }),
        (None, Some(project)) => {
            let projects = dw_config::load_projects_config(root);
            let workflow = dw_config::load_workflow_config(root);
            resolve_ado_options(&projects, &workflow, &project)
        }
        _ => Err(anyhow::anyhow!(
            "ado ai-context requires a configured project, or an organization with an explicit project."
        )),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn project_choices_are_provided_by_dw_config() {
        let projects = dw_config::ProjectsConfig::default();

        assert!(dw_config::project_choices(&projects).is_empty());
    }
}
