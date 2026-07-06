use anyhow::Result;
use dw_core::{ProjectKey, WorkItemId, WorkItemTitle, WorkItemTypeName, WorkspacePath};

pub fn write_workspace_agent_configs(
    workspace: &str,
    manifest: &dw_workspace::WorkspaceManifest,
) -> Result<()> {
    let config_files = dw_agent::workspace_config_files(&dw_agent::AgentWorkspaceConfigRequest {
        workspace: WorkspacePath::from(workspace),
        work_items: manifest
            .parent_work_items()
            .into_iter()
            .map(|item| dw_agent::WorkspaceWorkItemRef {
                id: WorkItemId::from(item.id),
                kind: item.kind.map(WorkItemTypeName::from),
                title: item.title.map(WorkItemTitle::from),
            })
            .collect(),
        project: ProjectKey::from(manifest.project.clone()),
    });
    for file in config_files {
        let path = std::path::Path::new(workspace).join(file.relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.content)?;
    }
    Ok(())
}
