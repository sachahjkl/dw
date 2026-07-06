use anyhow::{Result, anyhow};
use dw_config::{load_projects_config, resolve_project, resolve_root};
use dw_core::{
    BranchName, CommitMessage, DevWorkflowRoot, ProjectKey, RepositoryPath, WorkItemId,
    WorkspacePath, WorkspaceRepositoryName,
};
use dw_git::{
    RepositoryStatus, WorktreePrepareRequest, WorktreePrepareResult, commit_repository,
    prepare_worktree, repository_status, update_repository, worktree_prune, worktree_remove,
};
use dw_workspace::{
    WorkspaceError, WorkspaceManifest, WorkspaceTeardownStep, build_commit_message,
    execute_task_add_repo, execute_task_teardown, plan_task_add_repo, plan_task_commit,
    plan_task_repo_latest, plan_task_teardown, resolve_git_credential_from_keyring,
    resolve_workspace_by_work_item_ids, resolve_workspace_for_workspace_command,
};
use serde::{Deserialize, Serialize};

use crate::write_workspace_agent_configs;

#[derive(Debug, Clone)]
pub struct RepoLatestArgs {
    pub workspace: Option<WorkspacePath>,
    pub r#continue: bool,
    pub repositories: Vec<WorkspaceRepositoryName>,
    pub root: Option<DevWorkflowRoot>,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct CommitArgs {
    pub workspace: Option<WorkspacePath>,
    pub r#continue: bool,
    pub root: Option<DevWorkflowRoot>,
    pub mode: dw_core::ExecutionMode,
    pub message: Option<CommitMessage>,
}

#[derive(Debug, Clone)]
pub struct AddRepoArgs {
    pub repo: WorkspaceRepositoryName,
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub mode: dw_core::ExecutionMode,
}

#[derive(Debug, Clone)]
pub struct AddRepoChoicesArgs {
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
}

#[derive(Debug, Clone)]
pub struct TeardownArgs {
    pub workspace: Option<WorkspacePath>,
    pub root: Option<DevWorkflowRoot>,
    pub project: Option<ProjectKey>,
    pub work_item_ids: Vec<WorkItemId>,
    pub r#continue: bool,
    pub mode: dw_core::ExecutionMode,
    pub yes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoLatestPlanReport {
    pub workspace: WorkspacePath,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    pub targets: Vec<dw_workspace::TaskRepoLatestTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoLatestExecutionReport {
    pub workspace: WorkspacePath,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    pub updated: Vec<RepoLatestUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoLatestUpdate {
    pub repository: WorkspaceRepositoryName,
    pub path: RepositoryPath,
    #[serde(rename = "defaultBranch")]
    pub default_branch: BranchName,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitPlanReport {
    pub workspace: WorkspacePath,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    pub message: CommitMessage,
    pub targets: Vec<CommitTargetStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitTargetStatus {
    pub target: dw_workspace::TaskCommitTarget,
    pub status: RepositoryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitExecutionReport {
    pub workspace: WorkspacePath,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    pub message: CommitMessage,
    pub committed: Vec<WorkspaceRepositoryName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddRepoChoicesReport {
    pub workspace: WorkspacePath,
    pub choices: Vec<WorkspaceRepositoryName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddRepoPlanReport {
    pub plan: dw_workspace::TaskAddRepoPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddRepoExecutionReport {
    pub plan: dw_workspace::TaskAddRepoPlan,
    #[serde(rename = "worktree")]
    pub worktree: WorktreePrepareResult,
    pub manifest: WorkspaceManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeardownPlanReport {
    pub workspace: Option<WorkspacePath>,
    pub steps: Vec<WorkspaceTeardownStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeardownExecutionReport {
    pub workspace: WorkspacePath,
    pub steps: Vec<WorkspaceTeardownStep>,
}

pub fn repo_latest_plan(args: RepoLatestArgs) -> Result<RepoLatestPlanReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.r#continue,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let (manifest, targets) =
        plan_task_repo_latest(&root, &projects, &workspace, &args.repositories)?;

    Ok(RepoLatestPlanReport {
        workspace,
        branch_name: manifest.branch_name,
        targets,
    })
}

pub fn execute_repo_latest(plan: &RepoLatestPlanReport) -> Result<RepoLatestExecutionReport> {
    let mut updated = Vec::new();
    for target in &plan.targets {
        let credential = resolve_git_credential_from_keyring(target.git_credential_secret.as_ref())
            .map_err(|message| anyhow!(message.to_string()))?;
        update_repository(
            target.repository_path.as_str(),
            target.default_branch.as_str(),
            credential.as_ref(),
        )?;
        updated.push(RepoLatestUpdate {
            repository: target.repository.clone(),
            path: target.repository_path.clone(),
            default_branch: target.default_branch.clone(),
        });
    }

    Ok(RepoLatestExecutionReport {
        workspace: plan.workspace.clone(),
        branch_name: plan.branch_name.clone(),
        updated,
    })
}

pub fn commit_plan(args: CommitArgs) -> Result<CommitPlanReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.r#continue,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let (manifest, targets) = plan_task_commit(&projects, &workspace)?;
    let statuses = targets
        .into_iter()
        .map(|target| {
            let status = repository_status(target.path.as_str());
            CommitTargetStatus { target, status }
        })
        .collect::<Vec<_>>();

    Ok(CommitPlanReport {
        workspace,
        branch_name: manifest.branch_name.clone(),
        message: build_commit_message(&manifest, args.message.as_ref()),
        targets: statuses,
    })
}

pub fn execute_commit(plan: &CommitPlanReport) -> Result<CommitExecutionReport> {
    let changed = changed_commit_targets(plan);
    for item in &changed {
        commit_repository(item.target.path.as_str(), plan.message.as_str())?;
    }

    Ok(CommitExecutionReport {
        workspace: plan.workspace.clone(),
        branch_name: plan.branch_name.clone(),
        message: plan.message.clone(),
        committed: changed
            .into_iter()
            .map(|item| item.target.repository.clone())
            .collect(),
    })
}

pub fn add_repo_choices(args: AddRepoChoicesArgs) -> Result<AddRepoChoicesReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        false,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let manifest = dw_workspace::read_manifest_path(&format!("{workspace}/task.json"))?;

    Ok(AddRepoChoicesReport {
        workspace,
        choices: add_repo_choices_for_manifest(&projects, &manifest),
    })
}

pub fn add_repo_plan(args: AddRepoArgs) -> Result<AddRepoPlanReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        false,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let (_manifest, plan) = plan_task_add_repo(&root, &projects, &workspace, args.repo.as_str())?;
    Ok(AddRepoPlanReport { plan })
}

pub fn execute_add_repo(plan: &AddRepoPlanReport) -> Result<AddRepoExecutionReport> {
    let manifest = dw_workspace::read_manifest_path(&format!("{}/task.json", plan.plan.workspace))?;
    let credential = resolve_git_credential_from_keyring(plan.plan.git_credential_secret.as_ref())
        .map_err(|message| anyhow!(message.to_string()))?;
    let worktree = prepare_worktree(&WorktreePrepareRequest {
        project_root: plan.plan.project_root.to_string(),
        repository: plan.plan.repository.to_string(),
        url: plan.plan.url.clone(),
        default_branch: plan.plan.default_branch.to_string(),
        anchor_name: plan.plan.anchor_name.clone(),
        branch_name: plan.plan.branch_name.to_string(),
        worktree_path: plan.plan.worktree_path.to_string(),
        credential,
    })?;
    let updated = execute_task_add_repo(&manifest, &plan.plan)?;
    write_workspace_agent_configs(plan.plan.workspace.as_str(), &updated)?;

    Ok(AddRepoExecutionReport {
        plan: plan.plan.clone(),
        worktree,
        manifest: updated,
    })
}

pub fn teardown_plan(args: TeardownArgs) -> Result<TeardownPlanReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = match resolve_workspace_by_work_item_ids(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.project.as_ref().map(ProjectKey::as_str),
        &args.work_item_ids,
        args.r#continue,
    ) {
        Ok(workspace) => workspace,
        Err(WorkspaceError::NoWorkspaceFound | WorkspaceError::NoCurrentWorkspace) => {
            return Ok(TeardownPlanReport {
                workspace: None,
                steps: Vec::new(),
            });
        }
        Err(error) => return Err(error.into()),
    };
    let projects = load_projects_config(&root);
    let (_manifest, steps) = plan_task_teardown(&root, &projects, &workspace)?;

    Ok(TeardownPlanReport {
        workspace: Some(workspace),
        steps,
    })
}

pub fn execute_teardown(plan: &TeardownPlanReport) -> Result<TeardownExecutionReport> {
    let workspace = plan
        .workspace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Aucun workspace task trouvé."))?;
    execute_task_teardown(workspace, &plan.steps, |git_dir, args| match args {
        ["worktree", "remove", "--force", target] => {
            worktree_remove(git_dir, target).map_err(|error| error.to_string())
        }
        ["worktree", "prune"] => worktree_prune(git_dir).map_err(|error| error.to_string()),
        _ => Err(format!("commande git non supportée: {}", args.join(" "))),
    })?;

    Ok(TeardownExecutionReport {
        workspace: workspace.clone(),
        steps: plan.steps.clone(),
    })
}

pub fn changed_commit_targets(plan: &CommitPlanReport) -> Vec<&CommitTargetStatus> {
    plan.targets
        .iter()
        .filter(|item| item.status.is_git_repository && item.status.has_changes)
        .collect()
}

pub fn add_repo_choices_for_manifest(
    projects: &dw_config::ProjectsConfig,
    manifest: &dw_workspace::WorkspaceManifest,
) -> Vec<WorkspaceRepositoryName> {
    resolve_project(projects, manifest.project.as_str())
        .map(|project| {
            project
                .repositories
                .keys()
                .filter(|repository| {
                    !manifest
                        .repositories
                        .iter()
                        .any(|existing| existing.as_str().eq_ignore_ascii_case(repository))
                })
                .cloned()
                .map(WorkspaceRepositoryName::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{add_repo_choices_for_manifest, changed_commit_targets};

    #[test]
    fn add_repo_choices_hide_repositories_already_in_workspace() {
        let projects: dw_config::ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": { "url": "", "defaultBranch": "main" },
        "back": { "url": "", "defaultBranch": "main" },
        "db": { "url": "", "defaultBranch": "main" }
      }
    }
  }
}"#,
        )
        .expect("projects config should parse");
        let manifest = dw_workspace::WorkspaceManifest {
            schema: 1,
            work_item_id: "42".into(),
            task_id: None,
            project: "ha".into(),
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/42-demo".into(),
            created_at: "2026-07-03T10:00:00Z".into(),
            repositories: vec!["front".into()],
            status: dw_workspace::WorkspaceManifestStatus::Created,
            work_item_type: None,
            work_item_title: None,
            work_item_state: None,
            child_task_ids: None,
            child_tasks: None,
            work_items: None,
        };

        assert_eq!(
            add_repo_choices_for_manifest(&projects, &manifest),
            vec![
                dw_core::WorkspaceRepositoryName::from("back"),
                dw_core::WorkspaceRepositoryName::from("db")
            ]
        );
    }

    #[test]
    fn changed_commit_targets_only_keep_git_repositories_with_changes() {
        let report = dw_task_report_with_statuses(vec![
            ("front", true, true),
            ("back", true, false),
            ("docs", false, true),
        ]);

        let changed = changed_commit_targets(&report)
            .into_iter()
            .map(|item| item.target.repository.as_str())
            .collect::<Vec<_>>();

        assert_eq!(changed, vec!["front"]);
    }

    fn dw_task_report_with_statuses(items: Vec<(&str, bool, bool)>) -> super::CommitPlanReport {
        super::CommitPlanReport {
            workspace: dw_core::WorkspacePath::from("/tmp/ws"),
            branch_name: dw_core::BranchName::from("feat/42-demo"),
            message: dw_core::CommitMessage::from("feat(42): demo"),
            targets: items
                .into_iter()
                .map(
                    |(repository, is_git_repository, has_changes)| super::CommitTargetStatus {
                        target: dw_workspace::TaskCommitTarget {
                            repository: dw_core::WorkspaceRepositoryName::from(repository),
                            path: dw_core::RepositoryPath::from(format!("/tmp/ws/{repository}")),
                        },
                        status: dw_git::RepositoryStatus {
                            path: format!("/tmp/ws/{repository}"),
                            is_git_repository,
                            has_changes,
                            has_unpushed: false,
                            detail: String::new(),
                        },
                    },
                )
                .collect(),
        }
    }
}
