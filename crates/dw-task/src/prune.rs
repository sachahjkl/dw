use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::get_work_item_snapshots_authenticated;
use dw_config::{load_projects_config, load_workflow_config, resolve_root};
use dw_git::{worktree_prune, worktree_remove};
use dw_workspace::{
    WorkspaceSummary, display_work_items, execute_task_sync, execute_task_teardown,
    filter_workspaces, find_workspaces, plan_task_prune, plan_task_teardown,
};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct PruneArgs {
    pub root: Option<String>,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub mode: dw_core::ExecutionMode,
    pub yes: bool,
    pub no_sync: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrunePlanReport {
    pub root: String,
    pub project: Option<String>,
    pub work_item: Option<String>,
    pub sync: Vec<PruneSyncReport>,
    pub candidates: Vec<WorkspaceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PruneSyncReport {
    pub workspace: String,
    pub status: PruneSyncStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PruneSyncStatus {
    Skipped,
    Synced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PruneExecutionReport {
    pub root: String,
    pub deleted: Vec<String>,
}

pub async fn plan(args: PruneArgs) -> Result<PrunePlanReport> {
    let PruneArgs {
        root,
        project,
        work_item,
        no_sync,
        mode: _,
        yes: _,
    } = args;

    let root = resolve_root(root.as_deref());
    let sync = if no_sync {
        Vec::new()
    } else {
        let workspaces = filter_workspaces(
            find_workspaces(&root),
            project.as_deref(),
            work_item.as_deref(),
        );
        sync_workspaces(&root, &workspaces).await
    };

    let candidates = plan_task_prune(&root, project.as_deref(), work_item.as_deref());
    Ok(PrunePlanReport {
        root,
        project,
        work_item,
        sync,
        candidates,
    })
}

pub fn execute(
    root: &str,
    selected_candidates: Vec<WorkspaceSummary>,
) -> Result<PruneExecutionReport> {
    let projects = load_projects_config(root);
    let mut deleted = Vec::new();
    for candidate in selected_candidates {
        let (_manifest, steps) = plan_task_teardown(root, &projects, &candidate.path)?;
        execute_task_teardown(&candidate.path, &steps, |git_dir, args| match args {
            ["worktree", "remove", "--force", target] => {
                worktree_remove(git_dir, target).map_err(|error| error.to_string())
            }
            ["worktree", "prune"] => worktree_prune(git_dir).map_err(|error| error.to_string()),
            _ => Err(format!("commande git non supportée: {}", args.join(" "))),
        })?;
        deleted.push(candidate.path);
    }
    Ok(PruneExecutionReport {
        root: root.into(),
        deleted,
    })
}

async fn sync_workspaces(root: &str, workspaces: &[WorkspaceSummary]) -> Vec<PruneSyncReport> {
    let projects = load_projects_config(root);
    let workflow = load_workflow_config(root);
    let auth_options = match load_auth_options(Some(root)) {
        Ok(options) => options,
        Err(error) => {
            return workspaces
                .iter()
                .map(|workspace| PruneSyncReport {
                    workspace: workspace.path.clone(),
                    status: PruneSyncStatus::Skipped,
                    message: format!("auth indisponible: {error}"),
                })
                .collect();
        }
    };

    let mut reports = Vec::new();
    for workspace in workspaces {
        let result: Result<String> = async {
            let mut options =
                resolve_ado_options(&projects, &workflow, &workspace.manifest.project)?;
            if options.project.trim().is_empty() {
                options.project = workspace.manifest.project.clone();
            }
            let token = require_token(auth_options.clone()).await?;
            let ids = workspace
                .manifest
                .parent_work_items()
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>();
            let snapshots = get_work_item_snapshots_authenticated(&options, &ids, &token)?;
            let updated = execute_task_sync(&workspace.path, &snapshots)?;
            Ok(display_work_items(&updated.parent_work_items(), true))
        }
        .await;

        match result {
            Ok(items) => reports.push(PruneSyncReport {
                workspace: workspace.path.clone(),
                status: PruneSyncStatus::Synced,
                message: items,
            }),
            Err(error) => reports.push(PruneSyncReport {
                workspace: workspace.path.clone(),
                status: PruneSyncStatus::Skipped,
                message: error.to_string(),
            }),
        }
    }
    reports
}

pub fn prune_candidate_label(candidate: &WorkspaceSummary) -> String {
    format!(
        "{} / {}",
        candidate.manifest.project,
        display_work_items(&candidate.manifest.parent_work_items(), true)
    )
}

pub fn prune_candidate_choice(candidate: &WorkspaceSummary) -> String {
    format!(
        "{} - {} - {}",
        prune_candidate_label(candidate),
        candidate.manifest.repositories.join(", "),
        candidate.path
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dw_workspace::WorkspaceManifest;

    #[tokio::test]
    async fn prune_no_sync_dry_run_does_not_require_auth() {
        let root = unique_temp_root();
        let workspace = root.join("projects/ha/workspaces/feat-1-done");
        std::fs::create_dir_all(&workspace).expect("workspace should be created");
        std::fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"1","taskId":null,"project":"ha","type":"feat","slug":"done","branchName":"feat/1-done","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"1","type":"User Story","title":"Done","state":"Valide"}]}"#,
        )
        .expect("manifest should be written");

        let report = plan(PruneArgs {
            root: Some(root.display().to_string()),
            project: Some("ha".into()),
            work_item: None,
            mode: dw_core::ExecutionMode::Preview,
            yes: false,
            no_sync: true,
        })
        .await
        .expect("offline dry-run should not require auth");

        assert_eq!(report.candidates.len(), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn prune_candidate_label_includes_project_and_items() {
        let candidate = candidate_fixture();
        assert_eq!(prune_candidate_label(&candidate), "ha / #1 Done [Valide]");
    }

    #[test]
    fn prune_candidate_choice_includes_context_and_path() {
        let candidate = candidate_fixture();
        assert_eq!(
            prune_candidate_choice(&candidate),
            "ha / #1 Done [Valide] - front, back - /tmp/dw/projects/ha/workspaces/feat-1-done"
        );
    }

    fn candidate_fixture() -> WorkspaceSummary {
        WorkspaceSummary {
            path: "/tmp/dw/projects/ha/workspaces/feat-1-done".into(),
            manifest: WorkspaceManifest {
                schema: 1,
                work_item_id: "1".into(),
                task_id: None,
                project: "ha".into(),
                kind: "feat".into(),
                slug: "done".into(),
                branch_name: "feat/1-done".into(),
                created_at: "2026-07-02T10:00:00Z".into(),
                repositories: vec!["front".into(), "back".into()],
                status: "created".into(),
                work_item_type: Some("User Story".into()),
                work_item_title: Some("Done".into()),
                work_item_state: Some("Valide".into()),
                child_task_ids: None,
                child_tasks: None,
                work_items: None,
            },
        }
    }

    fn unique_temp_root() -> std::path::PathBuf {
        let suffix = format!(
            "dw-prune-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(suffix)
    }
}
