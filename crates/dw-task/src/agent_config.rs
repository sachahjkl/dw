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

pub fn refresh_workspace_agent_configs(root: &str) -> Result<()> {
    for workspace in dw_workspace::find_workspaces(root) {
        write_workspace_agent_configs(workspace.path.as_str(), &workspace.manifest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_core::{BranchName, ProjectKey, TaskSlug, Timestamp, WorkItemId, WorkItemTypeName};

    #[test]
    fn refresh_rewrites_existing_workspace_agent_contexts() {
        let root = tempfile::tempdir().expect("root");
        let workspace = root.path().join("projects/ha/workspaces/feat-42-demo");
        std::fs::create_dir_all(&workspace).expect("workspace");
        let manifest = dw_workspace::WorkspaceManifest {
            schema: 1,
            work_item_id: WorkItemId::from("42"),
            task_id: None,
            project: ProjectKey::from("ha"),
            kind: WorkItemTypeName::from("feat"),
            slug: TaskSlug::from("demo"),
            branch_name: BranchName::from("feat/42-demo"),
            created_at: Timestamp::from("2026-07-17T00:00:00Z"),
            repositories: Vec::new(),
            status: dw_workspace::WorkspaceManifestStatus::Created,
            work_item_type: Some(WorkItemTypeName::from("User Story")),
            work_item_title: None,
            work_item_state: None,
            child_task_ids: None,
            child_tasks: None,
            work_items: None,
        };
        std::fs::write(
            workspace.join("task.json"),
            serde_json::to_string(&manifest).expect("manifest"),
        )
        .expect("task.json");
        std::fs::write(workspace.join("AGENTS.md"), "stale context").expect("stale context");

        refresh_workspace_agent_configs(root.path().to_str().expect("root path"))
            .expect("refresh contexts");

        let agents = std::fs::read_to_string(workspace.join("AGENTS.md")).expect("AGENTS.md");
        assert!(agents.contains("action work current"));
        assert!(agents.contains("action ADO item show"));
    }
}
