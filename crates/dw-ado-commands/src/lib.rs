pub mod auth;
pub mod commands;

use anyhow::Result;
use dw_ado::auth::AdoAuthOptions;
use dw_config::WorkflowConfig;
use dw_core::ProjectKey;

pub use commands::project::resolve_ado_options;

pub fn load_auth_options(root: Option<&str>) -> Result<Option<AdoAuthOptions>> {
    let root = dw_config::resolve_root(root);
    let workflow = dw_config::load_workflow_config(&root);
    workflow
        .auth
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

pub fn is_final_state(work_item_type: Option<&str>, state: Option<&str>) -> bool {
    dw_ado::is_final_state(work_item_type, state)
}

pub fn resolve_options(
    projects: &dw_config::ProjectsConfig,
    workflow: &WorkflowConfig,
    project_key: &ProjectKey,
) -> Result<dw_ado::AzureDevOpsOptions> {
    commands::project::resolve_ado_options(projects, workflow, project_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_state_matches_workspace_rules() {
        assert!(is_final_state(Some("User Story"), Some("Validé")));
        assert!(!is_final_state(Some("Bug"), Some("Validé")));
        assert!(is_final_state(Some("Bug"), Some("Clôturé")));
    }
}
