use anyhow::Result;
use dw_ado::{AzureDevOpsOptions, default_api_version};
use dw_config::{project_choices, resolve_project};
use inquire::Select;
use std::io::IsTerminal;

pub fn resolve_ado_options(
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
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

pub fn resolve_cli_ado_options(
    root: &str,
    organization: Option<String>,
    project: Option<String>,
) -> Result<AzureDevOpsOptions> {
    match (organization, project) {
        (Some(organization), Some(project)) => Ok(AzureDevOpsOptions {
            organization,
            project,
            api_version: default_api_version(),
        }),
        (None, Some(project)) => {
            let projects = dw_config::load_projects_config(root);
            let workflow = dw_config::load_workflow_config(root);
            resolve_ado_options(&projects, &workflow, &project)
        }
        _ => Err(anyhow::anyhow!(
            "ado ai-context requiert --project configure ou --organization avec --project."
        )),
    }
}

pub fn resolve_project_key_or_prompt(
    project: Option<String>,
    projects: &dw_config::ProjectsConfig,
    command_name: &str,
) -> Result<String> {
    if let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        return Ok(project);
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "{command_name} requiert --project configure en mode non-interactif."
        ));
    }

    let choices = project_choices(projects);
    if choices.is_empty() {
        return Err(anyhow::anyhow!(
            "Aucun projet configure dans projects.json. Executer dw init ou completer config/projects.json."
        ));
    }

    let selected = Select::new("Projet Azure DevOps", choices).prompt()?;
    Ok(selected.key)
}

#[cfg(test)]
mod tests {
    #[test]
    fn project_choices_are_provided_by_dw_config() {
        let projects = dw_config::ProjectsConfig::default();

        assert!(dw_config::project_choices(&projects).is_empty());
    }
}
