use anyhow::Result;
use dw_core::WorkspacePath;

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
                id: item.id,
                kind: item.kind,
                title: item.title,
            })
            .collect(),
        project: manifest.project.clone(),
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
