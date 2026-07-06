use dw_ado::WorkItemSnapshot;
use dw_config::{
    ProjectConfig, ProjectsConfig, RepositoryConfig, repository_config, resolve_project,
};
use dw_contracts::{
    AdoAiContextItem, HANDOFF_PREFIX, HANDOFF_VALIDATION_VERSION, MARKDOWN_EXTENSION,
    PREFLIGHT_VERSION, TaskHandoffValidationItem, TaskHandoffValidationReport, TaskPreflightIssue,
    TaskPreflightReport,
};
use dw_core::{ProjectKey, WorkItemId};
use dw_git::{
    WorktreePrepareRequest, build_branch_name, build_subject_name, prepare_worktree,
    slug_from_phrase_or_fallback,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub mod completion;
mod finish;
mod start;
mod templates;

pub use finish::{
    PullRequestCandidate, TaskFinishError, TaskFinishOptions, VerificationResult,
    ensure_verification_passed, finish_state, pull_request_description, pull_request_title,
    read_handoff_summary, read_plan, run_verification, select_pull_request_candidates,
    structured_handoff_section, task_finish_options,
};
pub use start::{TaskStartOptions, start_state, task_start_options};
use templates::{handoff_markdown, plan_markdown};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceManifest {
    pub schema: i64,
    #[serde(rename = "workItemId")]
    pub work_item_id: String,
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,
    pub project: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub slug: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub repositories: Vec<String>,
    pub status: String,
    #[serde(rename = "workItemType")]
    pub work_item_type: Option<String>,
    #[serde(rename = "workItemTitle")]
    pub work_item_title: Option<String>,
    #[serde(rename = "workItemState")]
    pub work_item_state: Option<String>,
    #[serde(rename = "childTaskIds")]
    pub child_task_ids: Option<BTreeMap<String, String>>,
    #[serde(rename = "childTasks")]
    pub child_tasks: Option<Vec<WorkspaceChildTask>>,
    #[serde(rename = "workItems")]
    pub work_items: Option<Vec<WorkspaceWorkItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceWorkItem {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub title: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceChildTask {
    pub repository: String,
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub path: String,
    pub manifest: WorkspaceManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskListItem {
    pub path: String,
    pub project: String,
    #[serde(rename = "workItemId")]
    pub work_item_id: String,
    #[serde(rename = "displayWorkItems")]
    pub display_work_items: String,
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,
    #[serde(rename = "allKnownWorkItemIds")]
    pub all_known_work_item_ids: Vec<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub slug: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "workItemType")]
    pub work_item_type: Option<String>,
    #[serde(rename = "workItemTitle")]
    pub work_item_title: Option<String>,
    #[serde(rename = "workItemState")]
    pub work_item_state: Option<String>,
    pub repositories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCurrentItem {
    pub workspace: String,
    pub project: String,
    #[serde(rename = "primaryWorkItemId")]
    pub primary_work_item_id: String,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,
    #[serde(rename = "childTaskIds")]
    pub child_task_ids: BTreeMap<String, String>,
    #[serde(rename = "childTasks")]
    pub child_tasks: Vec<WorkspaceChildTask>,
    pub branch: String,
    pub repositories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskStartPlan {
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<String>,
    #[serde(rename = "primaryWorkItemId")]
    pub primary_work_item_id: String,
    pub project: String,
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub slug: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
    #[serde(rename = "subjectName")]
    pub subject_name: String,
    pub workspace: String,
    pub repositories: Vec<String>,
    #[serde(rename = "repositoryFolders")]
    pub repository_folders: BTreeMap<String, String>,
    #[serde(rename = "repositoryWorktrees")]
    pub repository_worktrees: Vec<TaskStartRepositoryPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskStartRepositoryPlan {
    pub repository: String,
    #[serde(rename = "projectRoot")]
    pub project_root: String,
    #[serde(rename = "worktreePath")]
    pub worktree_path: String,
    pub url: String,
    #[serde(rename = "defaultBranch")]
    pub default_branch: String,
    #[serde(rename = "anchorName")]
    pub anchor_name: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRenamePlan {
    pub workspace: String,
    #[serde(rename = "newWorkspace")]
    pub new_workspace: String,
    #[serde(rename = "oldSlug")]
    pub old_slug: String,
    #[serde(rename = "newSlug")]
    pub new_slug: String,
    #[serde(rename = "oldBranch")]
    pub old_branch: String,
    #[serde(rename = "newBranch")]
    pub new_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRepoLatestTarget {
    pub repository: String,
    pub repository_path: String,
    #[serde(rename = "defaultBranch")]
    pub default_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCommitTarget {
    pub repository: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskWorkItemUpdatePlan {
    pub workspace: String,
    #[serde(rename = "newWorkspace")]
    pub new_workspace: String,
    #[serde(rename = "oldBranch")]
    pub old_branch: String,
    #[serde(rename = "newBranch")]
    pub new_branch: String,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskAddRepoPlan {
    pub workspace: String,
    pub repository: String,
    #[serde(rename = "projectRoot")]
    pub project_root: String,
    #[serde(rename = "worktreePath")]
    pub worktree_path: String,
    pub url: String,
    #[serde(rename = "defaultBranch")]
    pub default_branch: String,
    #[serde(rename = "anchorName")]
    pub anchor_name: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
    pub repositories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceTeardownStep {
    pub repository: String,
    pub action: String,
    pub target: String,
    #[serde(rename = "gitDir")]
    pub git_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskStartRequest<'a> {
    pub root: &'a str,
    pub projects: &'a ProjectsConfig,
    pub work_item_ids: &'a [WorkItemId],
    pub project: Option<&'a str>,
    pub task_id: Option<&'a str>,
    pub type_name: Option<&'a str>,
    pub only: Option<&'a str>,
    pub slug: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceHandoffSummary {
    pub repository: String,
    pub status: String,
    pub done: Vec<String>,
    pub decisions: Vec<String>,
    pub risks: Vec<String>,
    pub blockers: Vec<String>,
    pub follow_up: Vec<String>,
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("Manifest task introuvable: {0}")]
    MissingManifest(String),
    #[error("Manifest task invalide: {0}")]
    InvalidManifest(String),
    #[error("Workspace introuvable: {0}")]
    MissingWorkspace(String),
    #[error("Aucun workspace task trouvé.")]
    NoWorkspaceFound,
    #[error("Aucun workspace task trouvé depuis le dossier courant.")]
    NoCurrentWorkspace,
    #[error("Les deux sélections de work item doivent pointer vers le même work item.")]
    ConflictingWorkItemSelection,
    #[error("Repo absent du workspace: {0}")]
    MissingWorkspaceRepository(String),
    #[error("Workspace déjà existant pour un des work items demandés: {0}")]
    WorkspaceConflict(String),
    #[error("Fichier ai-context introuvable: {0}")]
    MissingAiContext(String),
    #[error("Suppression workspace échouée [{repository}]: {message}")]
    TeardownFailed { repository: String, message: String },
    #[error("Préparation worktree échouée [{repository}]: {message}")]
    WorktreePrepareFailed { repository: String, message: String },
    #[error("Impossible de retirer tous les work items du workspace.")]
    EmptyWorkItemSet,
}

pub fn build_handoff_validation_report(
    workspace: &str,
) -> Result<TaskHandoffValidationReport, WorkspaceError> {
    let workspace_path = Path::new(workspace);
    if !workspace_path.exists() {
        return Err(WorkspaceError::MissingWorkspace(workspace.into()));
    }

    let manifest_path = workspace_path.join("task.json");
    if !manifest_path.exists() {
        return Err(WorkspaceError::MissingManifest(
            manifest_path.display().to_string(),
        ));
    }

    let manifest = read_manifest(&manifest_path)?;

    let mut items = Vec::new();
    for repository in distinct_repositories(&manifest.repositories) {
        let path = workspace_path.join(format!("{HANDOFF_PREFIX}{repository}{MARKDOWN_EXTENSION}"));
        if !path.exists() {
            items.push(TaskHandoffValidationItem {
                repository,
                path: path.display().to_string(),
                status: "missing".into(),
                valid: false,
                message: "Fichier handoff manquant.".into(),
                done_count: 0,
                decision_count: 0,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 0,
            });
            continue;
        }

        let text = fs::read_to_string(&path).unwrap_or_default();
        match try_parse_summary(&text, &repository) {
            Ok(summary) => {
                let allowed = ["todo", "in_progress", "done", "blocked"];
                if !allowed
                    .iter()
                    .any(|status| status.eq_ignore_ascii_case(&summary.status))
                {
                    items.push(TaskHandoffValidationItem {
                        repository,
                        path: path.display().to_string(),
                        status: "invalid".into(),
                        valid: false,
                        message: format!(
                            "Status handoff invalide: {}. Attendus: {}.",
                            summary.status,
                            allowed.join(", ")
                        ),
                        done_count: 0,
                        decision_count: 0,
                        risk_count: 0,
                        blocker_count: 0,
                        follow_up_count: 0,
                    });
                    continue;
                }

                let valid = summary.status.eq_ignore_ascii_case("done");
                let status = if valid {
                    "valid".to_string()
                } else {
                    summary.status.to_lowercase()
                };
                items.push(TaskHandoffValidationItem {
                    repository,
                    path: path.display().to_string(),
                    status,
                    valid,
                    message: if valid {
                        "Handoff valide.".into()
                    } else {
                        format!(
                            "Handoff parseable mais pas prêt pour finish (status: {}).",
                            summary.status
                        )
                    },
                    done_count: summary.done.len(),
                    decision_count: summary.decisions.len(),
                    risk_count: summary.risks.len(),
                    blocker_count: summary.blockers.len(),
                    follow_up_count: summary.follow_up.len(),
                });
            }
            Err(error) => items.push(TaskHandoffValidationItem {
                repository,
                path: path.display().to_string(),
                status: "invalid".into(),
                valid: false,
                message: error,
                done_count: 0,
                decision_count: 0,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 0,
            }),
        }
    }

    let is_valid = items.iter().all(|item| item.valid);
    Ok(TaskHandoffValidationReport {
        schema_version: HANDOFF_VALIDATION_VERSION.into(),
        workspace: workspace.to_string(),
        project: ProjectKey::from(manifest.project),
        items,
        is_valid,
    })
}

pub fn find_workspace_path(start_path: &str) -> Option<String> {
    let current = PathBuf::from(start_path);
    if current.join("task.json").exists() {
        return Some(current.display().to_string());
    }

    let mut directory = current.as_path();
    while let Some(parent) = directory.parent() {
        if parent.join("task.json").exists() {
            return Some(parent.display().to_string());
        }
        directory = parent;
    }

    None
}

pub fn find_workspaces(root: &str) -> Vec<WorkspaceSummary> {
    let projects = Path::new(root).join("projects");
    let mut entries = Vec::new();
    collect_manifests(&projects, &mut entries);
    entries.sort_by(|left, right| right.manifest.created_at.cmp(&left.manifest.created_at));
    entries
}

pub fn filter_workspaces(
    workspaces: Vec<WorkspaceSummary>,
    project: Option<&str>,
    work_item: Option<&str>,
) -> Vec<WorkspaceSummary> {
    let requested_work_items = parse_work_item_selection(work_item);
    filter_workspaces_by_requested_ids(workspaces, project, requested_work_items.as_deref())
}

pub fn filter_workspaces_by_work_item_ids(
    workspaces: Vec<WorkspaceSummary>,
    project: Option<&str>,
    work_item_ids: &[WorkItemId],
) -> Vec<WorkspaceSummary> {
    filter_workspaces_by_requested_ids(workspaces, project, Some(work_item_ids))
}

fn filter_workspaces_by_requested_ids(
    workspaces: Vec<WorkspaceSummary>,
    project: Option<&str>,
    requested_work_items: Option<&[WorkItemId]>,
) -> Vec<WorkspaceSummary> {
    workspaces
        .into_iter()
        .filter(|workspace| {
            project.is_none_or(|project| workspace.manifest.project.eq_ignore_ascii_case(project))
        })
        .filter(|workspace| {
            requested_work_items.as_ref().is_none_or(|work_items| {
                work_items
                    .iter()
                    .all(|work_item| workspace.manifest.matches_work_item(work_item.as_str()))
            })
        })
        .collect()
}

pub fn task_status(root: &str) -> Vec<String> {
    find_workspaces(root)
        .into_iter()
        .map(|workspace| workspace.path)
        .collect()
}

pub fn task_list(root: &str, project: Option<&str>, work_item: Option<&str>) -> Vec<TaskListItem> {
    filter_workspaces(find_workspaces(root), project, work_item)
        .into_iter()
        .map(|workspace| TaskListItem {
            path: workspace.path,
            project: workspace.manifest.project.clone(),
            work_item_id: workspace.manifest.display_work_item_ids(),
            display_work_items: workspace.manifest.display_work_items(),
            task_id: workspace.manifest.task_id.clone(),
            all_known_work_item_ids: workspace.manifest.all_known_work_item_ids(),
            kind: workspace.manifest.kind.clone(),
            slug: workspace.manifest.slug.clone(),
            branch_name: workspace.manifest.branch_name.clone(),
            created_at: workspace.manifest.created_at.clone(),
            work_item_type: workspace.manifest.work_item_type.clone(),
            work_item_title: workspace.manifest.work_item_title.clone(),
            work_item_state: workspace.manifest.work_item_state.clone(),
            repositories: workspace.manifest.repositories.clone(),
        })
        .collect()
}

pub fn plan_task_prune(
    root: &str,
    project: Option<&str>,
    work_item: Option<&str>,
) -> Vec<WorkspaceSummary> {
    filter_workspaces(find_workspaces(root), project, work_item)
        .into_iter()
        .filter(|workspace| {
            workspace
                .manifest
                .parent_work_items()
                .iter()
                .all(|item| is_final_state(item.kind.as_deref(), item.state.as_deref()))
        })
        .collect()
}

pub fn display_work_item(item: &WorkspaceWorkItem, include_state: bool) -> String {
    let title = item.title.clone().unwrap_or_else(|| "(sans titre)".into());
    if include_state {
        let state = item.state.clone().unwrap_or_else(|| "?".into());
        format!("#{} {} [{}]", item.id, title, state)
    } else {
        format!("#{} {}", item.id, title)
    }
}

pub fn display_work_items(items: &[WorkspaceWorkItem], include_state: bool) -> String {
    items
        .iter()
        .map(|item| display_work_item(item, include_state))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn is_final_state(work_item_type: Option<&str>, state: Option<&str>) -> bool {
    dw_ado::is_final_state(work_item_type, state)
}

pub fn task_current(start_path: &str) -> Result<TaskCurrentItem, WorkspaceError> {
    let workspace = find_workspace_path(start_path).ok_or(WorkspaceError::NoCurrentWorkspace)?;
    let manifest = read_manifest(&Path::new(&workspace).join("task.json"))?;
    Ok(TaskCurrentItem {
        workspace,
        project: manifest.project.clone(),
        primary_work_item_id: manifest.primary_work_item_id(),
        work_items: manifest.parent_work_items(),
        task_id: manifest.task_id.clone(),
        child_task_ids: manifest.legacy_child_task_ids(),
        child_tasks: manifest.normalized_child_tasks(),
        branch: manifest.branch_name.clone(),
        repositories: manifest.repositories.clone(),
    })
}

pub fn read_manifest_path(path: &str) -> Result<WorkspaceManifest, WorkspaceError> {
    read_manifest(Path::new(path))
}

pub fn plan_task_rename(
    root: &str,
    projects: &ProjectsConfig,
    workspace: &str,
    slug: &str,
) -> Result<(WorkspaceManifest, TaskRenamePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let _project_config = resolve_project(projects, &manifest.project);
    let new_slug = slug_from_phrase_or_fallback(Some(slug), &manifest.slug);
    let new_branch = build_branch_name(
        &manifest.kind,
        &manifest.all_known_work_item_ids(),
        &new_slug,
    );
    let new_workspace = Path::new(workspace)
        .parent()
        .unwrap_or_else(|| Path::new(root))
        .join(build_subject_name(
            &manifest.kind,
            &manifest
                .parent_work_items()
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            &new_slug,
        ))
        .display()
        .to_string();

    Ok((
        manifest.clone(),
        TaskRenamePlan {
            workspace: workspace.to_string(),
            new_workspace,
            old_slug: manifest.slug,
            new_slug,
            old_branch: manifest.branch_name,
            new_branch,
        },
    ))
}

pub fn execute_task_rename(
    manifest: &WorkspaceManifest,
    plan: &TaskRenamePlan,
) -> Result<WorkspaceManifest, WorkspaceError> {
    let updated = WorkspaceManifest {
        slug: plan.new_slug.clone(),
        branch_name: plan.new_branch.clone(),
        ..manifest.clone()
    };
    write_text(
        &Path::new(&plan.workspace).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.clone()))?,
    )?;

    if !plan.workspace.eq_ignore_ascii_case(&plan.new_workspace) {
        fs::rename(&plan.workspace, &plan.new_workspace)
            .map_err(|_| WorkspaceError::MissingWorkspace(plan.new_workspace.clone()))?;
    }

    Ok(updated)
}

pub fn resolve_workspace_for_workspace_command(
    root: &str,
    workspace: Option<&str>,
    use_latest_workspace: bool,
    current_directory: &str,
) -> Result<String, WorkspaceError> {
    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        return Ok(PathBuf::from(workspace).display().to_string());
    }

    if use_latest_workspace {
        return resolve_workspace(root, None, None, None, None, true);
    }

    find_workspace_path(current_directory).ok_or(WorkspaceError::NoCurrentWorkspace)
}

pub fn plan_task_repo_latest(
    root: &str,
    projects: &ProjectsConfig,
    workspace: &str,
    only: Option<&str>,
) -> Result<(WorkspaceManifest, Vec<TaskRepoLatestTarget>), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let project_config = resolve_project(projects, &manifest.project);
    let repositories = resolve_workspace_repositories(&manifest, only)?;
    let targets = repositories
        .into_iter()
        .map(|repository| {
            let repository_config = project_config
                .as_ref()
                .and_then(|project| repository_config(project, &repository))
                .unwrap_or(RepositoryConfig {
                    url: String::new(),
                    default_branch: "main".into(),
                    pull_request_target_branch: None,
                    azure_dev_ops_repository: None,
                    anchor_name: None,
                    folder: Some(repository.clone()),
                });
            let folder = repository_config
                .folder
                .clone()
                .unwrap_or_else(|| repository.clone());
            TaskRepoLatestTarget {
                repository,
                repository_path: Path::new(workspace).join(folder).display().to_string(),
                default_branch: repository_config.default_branch,
            }
        })
        .collect::<Vec<_>>();
    let _ = root;
    Ok((manifest, targets))
}

pub fn plan_task_commit(
    projects: &ProjectsConfig,
    workspace: &str,
) -> Result<(WorkspaceManifest, Vec<TaskCommitTarget>), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let project_config = resolve_project(projects, &manifest.project);
    let targets = manifest
        .repositories
        .iter()
        .map(|repository| {
            let repository_config = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository));
            let folder = repository_config
                .and_then(|repository| repository.folder)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| repository.clone());
            TaskCommitTarget {
                repository: repository.clone(),
                path: Path::new(workspace).join(folder).display().to_string(),
            }
        })
        .collect();
    Ok((manifest, targets))
}

pub fn plan_task_finish(
    projects: &ProjectsConfig,
    workspace: &str,
) -> Result<
    (
        WorkspaceManifest,
        Vec<TaskCommitTarget>,
        TaskHandoffValidationReport,
    ),
    WorkspaceError,
> {
    let (manifest, targets) = plan_task_commit(projects, workspace)?;
    let handoff = build_handoff_validation_report(workspace)?;
    Ok((manifest, targets, handoff))
}

pub fn build_commit_message(
    manifest: &WorkspaceManifest,
    override_message: Option<&str>,
) -> String {
    if let Some(message) = override_message.filter(|message| !message.trim().is_empty()) {
        return ensure_work_item_reference(message, manifest);
    }

    format!(
        "{}({}): {}",
        commit_prefix(&manifest.kind),
        commit_ids(manifest).join(" "),
        manifest.slug
    )
}

pub fn ensure_work_item_reference(message: &str, manifest: &WorkspaceManifest) -> String {
    let ids = commit_ids_for_reference(manifest);
    if ids.iter().any(|id| message.contains(&format!("#{id}"))) {
        message.into()
    } else if let Some(id) = ids.first() {
        format!("{message} #{id}")
    } else {
        message.into()
    }
}

pub fn plan_add_work_items(
    root: &str,
    workspace: &str,
    ids: &str,
    kind: Option<&str>,
    title: Option<&str>,
    state: Option<&str>,
) -> Result<(WorkspaceManifest, TaskWorkItemUpdatePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let mut work_items = manifest.parent_work_items();
    let mut added_ids = Vec::new();
    for id in parse_work_item_selection(Some(ids)).unwrap_or_default() {
        if manifest.matches_work_item(id.as_str())
            || work_items
                .iter()
                .any(|item| item.id.eq_ignore_ascii_case(id.as_str()))
        {
            continue;
        }
        added_ids.push(id.to_string());
        work_items.push(WorkspaceWorkItem {
            id: id.to_string(),
            kind: kind.map(ToOwned::to_owned),
            title: title.map(ToOwned::to_owned),
            state: state.map(ToOwned::to_owned),
        });
    }
    reject_work_item_conflicts(root, workspace, &manifest.project, &added_ids)?;
    build_work_item_update_plan(root, workspace, &manifest, work_items).map(|plan| (manifest, plan))
}

pub fn plan_add_work_item_snapshots(
    root: &str,
    workspace: &str,
    snapshots: &[WorkItemSnapshot],
) -> Result<(WorkspaceManifest, TaskWorkItemUpdatePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let mut work_items = manifest.parent_work_items();
    let mut added_ids = Vec::new();
    for snapshot in snapshots {
        if manifest.matches_work_item(&snapshot.id)
            || work_items
                .iter()
                .any(|item| item.id.eq_ignore_ascii_case(&snapshot.id))
        {
            continue;
        }
        added_ids.push(snapshot.id.clone());
        work_items.push(WorkspaceWorkItem {
            id: snapshot.id.clone(),
            kind: snapshot.kind.clone(),
            title: snapshot.title.clone(),
            state: snapshot.state.clone(),
        });
    }
    reject_work_item_conflicts(root, workspace, &manifest.project, &added_ids)?;
    build_work_item_update_plan(root, workspace, &manifest, work_items).map(|plan| (manifest, plan))
}

pub fn plan_remove_work_items(
    root: &str,
    workspace: &str,
    ids: &str,
) -> Result<(WorkspaceManifest, TaskWorkItemUpdatePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let selection = parse_work_item_selection(Some(ids)).unwrap_or_default();
    let work_items = manifest
        .parent_work_items()
        .into_iter()
        .filter(|item| {
            !selection
                .iter()
                .any(|id| id.as_str().eq_ignore_ascii_case(&item.id))
        })
        .collect::<Vec<_>>();
    if work_items.is_empty() {
        return Err(WorkspaceError::EmptyWorkItemSet);
    }
    build_work_item_update_plan(root, workspace, &manifest, work_items).map(|plan| (manifest, plan))
}

pub fn execute_work_item_update(
    manifest: &WorkspaceManifest,
    plan: &TaskWorkItemUpdatePlan,
) -> Result<(WorkspaceManifest, String), WorkspaceError> {
    let first = plan
        .work_items
        .first()
        .ok_or(WorkspaceError::EmptyWorkItemSet)?;
    let updated = WorkspaceManifest {
        work_item_id: first.id.clone(),
        work_item_type: first.kind.clone(),
        work_item_title: first.title.clone(),
        work_item_state: first.state.clone(),
        work_items: Some(plan.work_items.clone()),
        branch_name: plan.new_branch.clone(),
        ..manifest.clone()
    };

    let workspace_path = Path::new(&plan.workspace);
    write_text(
        &workspace_path.join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.clone()))?,
    )?;
    write_text(&workspace_path.join("plan.md"), &plan_markdown(&updated))?;
    for repository in &updated.repositories {
        write_text(
            &workspace_path.join(format!("handoff-{repository}.md")),
            &handoff_markdown(&updated, repository),
        )?;
    }

    if plan.workspace != plan.new_workspace {
        if Path::new(&plan.new_workspace).exists() {
            return Err(WorkspaceError::WorkspaceConflict(
                plan.new_workspace.clone(),
            ));
        }
        fs::rename(&plan.workspace, &plan.new_workspace)
            .map_err(|_| WorkspaceError::MissingWorkspace(plan.new_workspace.clone()))?;
    }

    Ok((updated, plan.new_workspace.clone()))
}

pub fn execute_task_sync(
    workspace: &str,
    snapshots: &[WorkItemSnapshot],
) -> Result<WorkspaceManifest, WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    if snapshots.is_empty() {
        return Ok(manifest);
    }
    let work_items = snapshots
        .iter()
        .map(|snapshot| WorkspaceWorkItem {
            id: snapshot.id.clone(),
            kind: snapshot.kind.clone(),
            title: snapshot.title.clone(),
            state: snapshot.state.clone(),
        })
        .collect::<Vec<_>>();
    let first = &work_items[0];
    let updated = WorkspaceManifest {
        work_item_id: first.id.clone(),
        work_item_type: first.kind.clone(),
        work_item_title: first.title.clone(),
        work_item_state: first.state.clone(),
        work_items: Some(work_items),
        ..manifest
    };
    write_text(
        &Path::new(workspace).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(workspace.into()))?,
    )?;
    Ok(updated)
}

pub fn execute_add_child_task(
    workspace: &str,
    repository: &str,
    id: &str,
    title: Option<String>,
) -> Result<WorkspaceManifest, WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let mut child_tasks = manifest.normalized_child_tasks();
    child_tasks.push(WorkspaceChildTask {
        repository: repository.into(),
        id: id.into(),
        title,
    });
    let updated = WorkspaceManifest {
        child_tasks: Some(child_tasks),
        ..manifest
    };
    write_text(
        &Path::new(workspace).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(workspace.into()))?,
    )?;
    Ok(updated)
}

pub fn requires_child_tasks(work_item_type: Option<&str>) -> bool {
    let normalized = work_item_type.unwrap_or_default().trim().to_lowercase();
    normalized == "user story" || normalized == "anomalie"
}

pub fn plan_task_add_repo(
    root: &str,
    projects: &ProjectsConfig,
    workspace: &str,
    repository_key: &str,
) -> Result<(WorkspaceManifest, TaskAddRepoPlan), WorkspaceError> {
    let repository_key = repository_key.trim();
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    if manifest
        .repositories
        .iter()
        .any(|repository| repository.eq_ignore_ascii_case(repository_key))
    {
        let repository = repository_key.to_string();
        return Ok((
            manifest.clone(),
            TaskAddRepoPlan {
                workspace: workspace.into(),
                repository: repository.clone(),
                project_root: Path::new(root)
                    .join("projects")
                    .join(&manifest.project)
                    .display()
                    .to_string(),
                worktree_path: Path::new(workspace).join(&repository).display().to_string(),
                url: String::new(),
                default_branch: "main".into(),
                anchor_name: format!("{repository}.git"),
                branch_name: manifest.branch_name.clone(),
                repositories: manifest.repositories.clone(),
            },
        ));
    }

    let project_config = resolve_project(projects, &manifest.project)
        .ok_or_else(|| WorkspaceError::MissingWorkspaceRepository(repository_key.into()))?;
    let repository_config = repository_config(&project_config, repository_key)
        .ok_or_else(|| WorkspaceError::MissingWorkspaceRepository(repository_key.into()))?;
    let folder = repository_config
        .folder
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| repository_key.into());
    let anchor_name = repository_config
        .anchor_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{repository_key}.git"));
    let mut repositories = manifest.repositories.clone();
    repositories.push(repository_key.into());
    repositories = distinct_repositories(&repositories);

    Ok((
        manifest.clone(),
        TaskAddRepoPlan {
            workspace: workspace.into(),
            repository: repository_key.into(),
            project_root: Path::new(root)
                .join("projects")
                .join(&manifest.project)
                .display()
                .to_string(),
            worktree_path: Path::new(workspace).join(folder).display().to_string(),
            url: repository_config.url,
            default_branch: repository_config.default_branch,
            anchor_name,
            branch_name: manifest.branch_name.clone(),
            repositories,
        },
    ))
}

pub fn execute_task_add_repo(
    manifest: &WorkspaceManifest,
    plan: &TaskAddRepoPlan,
) -> Result<WorkspaceManifest, WorkspaceError> {
    let updated = WorkspaceManifest {
        repositories: plan.repositories.clone(),
        ..manifest.clone()
    };
    write_text(
        &Path::new(&plan.workspace).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.clone()))?,
    )?;
    write_text(
        &Path::new(&plan.workspace).join(format!("handoff-{}.md", plan.repository)),
        &handoff_markdown(&updated, &plan.repository),
    )?;
    Ok(updated)
}

pub fn plan_task_teardown(
    root: &str,
    projects: &ProjectsConfig,
    workspace: &str,
) -> Result<(WorkspaceManifest, Vec<WorkspaceTeardownStep>), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let project_config = resolve_project(projects, &manifest.project);
    let project_root = Path::new(root).join("projects").join(&manifest.project);
    let mut steps = Vec::new();

    for repository_key in distinct_repositories(&manifest.repositories) {
        let repository = project_config
            .as_ref()
            .and_then(|project| repository_config(project, &repository_key))
            .unwrap_or(RepositoryConfig {
                url: String::new(),
                default_branch: "main".into(),
                pull_request_target_branch: None,
                azure_dev_ops_repository: None,
                anchor_name: None,
                folder: Some(repository_key.clone()),
            });
        let folder = repository
            .folder
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| repository_key.clone());
        let anchor_name = repository
            .anchor_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("{repository_key}.git"));
        let git_dir = project_root
            .join("repositories")
            .join(anchor_name)
            .display()
            .to_string();
        steps.push(WorkspaceTeardownStep {
            repository: repository_key.clone(),
            action: "worktree remove".into(),
            target: Path::new(workspace).join(folder).display().to_string(),
            git_dir: Some(git_dir.clone()),
        });

        if !repository.url.trim().is_empty() {
            steps.push(WorkspaceTeardownStep {
                repository: repository_key,
                action: "worktree prune".into(),
                target: git_dir.clone(),
                git_dir: Some(git_dir),
            });
        }
    }

    steps.push(WorkspaceTeardownStep {
        repository: "workspace".into(),
        action: "delete directory".into(),
        target: workspace.into(),
        git_dir: None,
    });

    Ok((manifest, steps))
}

pub fn execute_task_teardown<F>(
    workspace: &str,
    steps: &[WorkspaceTeardownStep],
    mut run_git_dir: F,
) -> Result<(), WorkspaceError>
where
    F: FnMut(&str, &[&str]) -> Result<(), String>,
{
    for step in steps.iter().filter(|step| step.action == "worktree remove") {
        let git_dir = require_git_dir(step)?;
        if !Path::new(git_dir).exists() {
            return Err(WorkspaceError::TeardownFailed {
                repository: step.repository.clone(),
                message: format!("gitDir introuvable {git_dir}"),
            });
        }
        run_git_dir(git_dir, &["worktree", "remove", "--force", &step.target]).map_err(
            |message| WorkspaceError::TeardownFailed {
                repository: step.repository.clone(),
                message,
            },
        )?;
    }

    for step in steps.iter().filter(|step| step.action == "worktree prune") {
        let git_dir = require_git_dir(step)?;
        if !Path::new(git_dir).exists() {
            continue;
        }
        run_git_dir(git_dir, &["worktree", "prune"]).map_err(|message| {
            WorkspaceError::TeardownFailed {
                repository: step.repository.clone(),
                message,
            }
        })?;
    }

    if Path::new(workspace).exists() {
        fs::remove_dir_all(workspace).map_err(|error| WorkspaceError::TeardownFailed {
            repository: "workspace".into(),
            message: error.to_string(),
        })?;
    }

    Ok(())
}

pub fn execute_task_start(
    plan: &TaskStartPlan,
    work_item_type: Option<&str>,
    work_item_title: Option<&str>,
    work_item_state: Option<&str>,
) -> Result<WorkspaceManifest, WorkspaceError> {
    execute_task_start_with_work_items(
        plan,
        plan.work_item_ids
            .iter()
            .map(|id| WorkspaceWorkItem {
                id: id.clone(),
                kind: work_item_type.map(ToOwned::to_owned),
                title: work_item_title.map(ToOwned::to_owned),
                state: work_item_state.map(ToOwned::to_owned),
            })
            .collect(),
    )
}

pub fn execute_task_start_with_work_items(
    plan: &TaskStartPlan,
    work_items: Vec<WorkspaceWorkItem>,
) -> Result<WorkspaceManifest, WorkspaceError> {
    execute_task_start_with_work_items_and_child_tasks(plan, work_items, Vec::new())
}

pub fn execute_task_start_with_work_items_and_child_tasks(
    plan: &TaskStartPlan,
    work_items: Vec<WorkspaceWorkItem>,
    child_tasks: Vec<WorkspaceChildTask>,
) -> Result<WorkspaceManifest, WorkspaceError> {
    let workspace = Path::new(&plan.workspace);
    fs::create_dir_all(workspace)
        .map_err(|_| WorkspaceError::MissingWorkspace(plan.workspace.clone()))?;
    if plan.repository_worktrees.is_empty() {
        for folder in plan.repository_folders.values() {
            fs::create_dir_all(workspace.join(folder)).map_err(|_| {
                WorkspaceError::MissingWorkspace(workspace.join(folder).display().to_string())
            })?;
        }
    } else {
        for target in &plan.repository_worktrees {
            prepare_worktree(&WorktreePrepareRequest {
                project_root: target.project_root.clone(),
                repository: target.repository.clone(),
                url: target.url.clone(),
                default_branch: target.default_branch.clone(),
                anchor_name: target.anchor_name.clone(),
                branch_name: target.branch_name.clone(),
                worktree_path: target.worktree_path.clone(),
            })
            .map_err(|error| WorkspaceError::WorktreePrepareFailed {
                repository: target.repository.clone(),
                message: error.to_string(),
            })?;
        }
    }

    let work_items = if work_items.is_empty() {
        plan.work_item_ids
            .iter()
            .map(|id| WorkspaceWorkItem {
                id: id.clone(),
                kind: None,
                title: None,
                state: None,
            })
            .collect::<Vec<_>>()
    } else {
        work_items
    };
    let first = work_items
        .first()
        .ok_or_else(|| WorkspaceError::InvalidManifest(plan.workspace.clone()))?;

    let manifest = WorkspaceManifest {
        schema: 1,
        work_item_id: plan.primary_work_item_id.clone(),
        task_id: plan.task_id.clone(),
        project: plan.project.clone(),
        kind: plan.kind.clone(),
        slug: plan.slug.clone(),
        branch_name: plan.branch_name.clone(),
        created_at: current_timestamp_string(),
        repositories: plan.repositories.clone(),
        status: "created".into(),
        work_item_type: first.kind.clone(),
        work_item_title: first.title.clone(),
        work_item_state: first.state.clone(),
        child_task_ids: None,
        child_tasks: if child_tasks.is_empty() {
            None
        } else {
            Some(child_tasks)
        },
        work_items: Some(work_items),
    };

    write_text(
        &workspace.join("task.json"),
        &serde_json::to_string_pretty(&manifest)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.clone()))?,
    )?;
    write_text(&workspace.join("plan.md"), &plan_markdown(&manifest))?;
    for repository in &manifest.repositories {
        write_text(
            &workspace.join(format!("handoff-{repository}.md")),
            &handoff_markdown(&manifest, repository),
        )?;
    }

    Ok(manifest)
}

pub fn start_plan_with_child_tasks(
    mut plan: TaskStartPlan,
    child_tasks: &[WorkspaceChildTask],
) -> TaskStartPlan {
    if plan.task_id.as_ref().is_none_or(|id| id.trim().is_empty())
        && child_tasks.len() == 1
        && !child_tasks[0].id.trim().is_empty()
    {
        plan.task_id = Some(child_tasks[0].id.clone());
    }

    let mut branch_work_item_ids = plan.work_item_ids.clone();
    if let Some(task_id) = plan.task_id.as_ref().filter(|id| !id.trim().is_empty())
        && !branch_work_item_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case(task_id))
    {
        branch_work_item_ids.push(task_id.clone());
    }
    for child_task in child_tasks {
        if !child_task.id.trim().is_empty()
            && !branch_work_item_ids
                .iter()
                .any(|id| id.eq_ignore_ascii_case(&child_task.id))
        {
            branch_work_item_ids.push(child_task.id.clone());
        }
    }
    plan.branch_name = build_branch_name(&plan.kind, &branch_work_item_ids, &plan.slug);
    for worktree in &mut plan.repository_worktrees {
        worktree.branch_name = plan.branch_name.clone();
    }
    plan
}

pub fn build_preflight_report_from_ai_context_files(
    workspace: &str,
    ai_context_files: &[String],
) -> Result<TaskPreflightReport, WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace).join("task.json"))?;
    let mut issues = Vec::new();

    for file in ai_context_files {
        let text =
            fs::read_to_string(file).map_err(|_| WorkspaceError::MissingAiContext(file.clone()))?;
        let ai_context: AdoAiContextItem = serde_json::from_str(&text)
            .map_err(|_| WorkspaceError::MissingAiContext(file.clone()))?;
        issues.extend(build_stale_context_issues(&ai_context, &manifest));
        issues.extend(build_attachment_issues(&ai_context));
    }

    let has_blocking_issues = issues.iter().any(|issue| issue.severity == "blocking");
    Ok(TaskPreflightReport {
        schema_version: PREFLIGHT_VERSION.into(),
        workspace: workspace.into(),
        project: ProjectKey::from(manifest.project.clone()),
        work_item_ids: manifest
            .parent_work_items()
            .into_iter()
            .map(|item| WorkItemId::from(item.id))
            .collect(),
        issues,
        has_blocking_issues,
    })
}

pub fn plan_task_start(request: TaskStartRequest<'_>) -> Result<TaskStartPlan, WorkspaceError> {
    let project = request.project.unwrap_or("default").to_string();
    let work_item_ids = request
        .work_item_ids
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect::<Vec<_>>();
    let primary_work_item_id = work_item_ids.first().cloned().unwrap_or_default();
    let project_config = resolve_project(request.projects, &project);
    let repositories = resolve_repositories(project_config.as_ref(), request.only);
    let repository_folders = repositories
        .iter()
        .map(|repository| {
            let folder = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository))
                .and_then(|repository| repository.folder)
                .unwrap_or_else(|| repository.clone());
            (repository.clone(), folder)
        })
        .collect::<BTreeMap<_, _>>();
    reject_workspace_conflicts(request.root, &project, &work_item_ids)?;

    let kind = request
        .type_name
        .unwrap_or("feat")
        .trim()
        .to_ascii_lowercase();
    let slug =
        slug_from_phrase_or_fallback(request.slug, &format!("work item {primary_work_item_id}"));
    let mut branch_work_item_ids = work_item_ids.clone();
    if let Some(task_id) = request.task_id.filter(|value| !value.trim().is_empty())
        && !branch_work_item_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case(task_id))
    {
        branch_work_item_ids.push(task_id.to_string());
    }
    let subject_name = build_subject_name(&kind, &work_item_ids, &slug);
    let branch_name = build_branch_name(&kind, &branch_work_item_ids, &slug);
    let workspace = Path::new(request.root)
        .join("projects")
        .join(&project)
        .join("workspaces")
        .join(&subject_name)
        .display()
        .to_string();
    let project_root = Path::new(request.root)
        .join("projects")
        .join(&project)
        .display()
        .to_string();
    let repository_worktrees = repositories
        .iter()
        .map(|repository_key| {
            let repository = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository_key))
                .unwrap_or(RepositoryConfig {
                    url: String::new(),
                    default_branch: "main".into(),
                    pull_request_target_branch: None,
                    azure_dev_ops_repository: None,
                    anchor_name: None,
                    folder: Some(repository_key.clone()),
                });
            let folder = repository_folders
                .get(repository_key)
                .cloned()
                .unwrap_or_else(|| repository_key.clone());
            let anchor_name = repository
                .anchor_name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("{repository_key}.git"));
            TaskStartRepositoryPlan {
                repository: repository_key.clone(),
                project_root: project_root.clone(),
                worktree_path: Path::new(&workspace).join(folder).display().to_string(),
                url: repository.url,
                default_branch: repository.default_branch,
                anchor_name,
                branch_name: branch_name.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(TaskStartPlan {
        work_item_ids,
        primary_work_item_id,
        project,
        task_id: request.task_id.map(|value| value.to_string()),
        kind,
        slug,
        branch_name,
        subject_name,
        workspace,
        repositories,
        repository_folders,
        repository_worktrees,
    })
}

pub fn resolve_workspace(
    root: &str,
    workspace: Option<&str>,
    project: Option<&str>,
    work_item: Option<&str>,
    positional_work_item: Option<&str>,
    r#continue: bool,
) -> Result<String, WorkspaceError> {
    let work_item = resolve_work_item_ids(work_item, positional_work_item)?;

    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        return Ok(PathBuf::from(workspace).display().to_string());
    }

    let workspaces = filter_workspaces(find_workspaces(root), project, work_item.as_deref());
    if workspaces.is_empty() {
        return Err(WorkspaceError::NoWorkspaceFound);
    }

    if r#continue || workspaces.len() == 1 {
        return Ok(workspaces[0].path.clone());
    }

    Ok(workspaces[0].path.clone())
}

pub fn resolve_workspace_by_work_item_ids(
    root: &str,
    workspace: Option<&str>,
    project: Option<&str>,
    work_item_ids: &[WorkItemId],
    r#continue: bool,
) -> Result<String, WorkspaceError> {
    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        return Ok(PathBuf::from(workspace).display().to_string());
    }

    let workspaces =
        filter_workspaces_by_work_item_ids(find_workspaces(root), project, work_item_ids);
    if workspaces.is_empty() {
        return Err(WorkspaceError::NoWorkspaceFound);
    }

    if r#continue || workspaces.len() == 1 {
        return Ok(workspaces[0].path.clone());
    }

    Ok(workspaces[0].path.clone())
}

pub fn resolve_open_target(
    workspace: &str,
    manifest: &WorkspaceManifest,
    project_config: Option<&ProjectConfig>,
    repository_key: Option<&str>,
) -> Result<String, WorkspaceError> {
    let Some(repository_key) = repository_key.filter(|value| !value.trim().is_empty()) else {
        return Ok(workspace.into());
    };

    if !manifest
        .repositories
        .iter()
        .any(|repo| repo.eq_ignore_ascii_case(repository_key))
    {
        return Err(WorkspaceError::MissingWorkspaceRepository(
            repository_key.into(),
        ));
    }

    let repository = project_config
        .and_then(|project| repository_config(project, repository_key))
        .unwrap_or(RepositoryConfig {
            url: String::new(),
            default_branch: "main".into(),
            pull_request_target_branch: None,
            azure_dev_ops_repository: None,
            anchor_name: None,
            folder: Some(repository_key.into()),
        });
    let folder = repository.folder.unwrap_or_else(|| repository_key.into());
    Ok(Path::new(workspace).join(folder).display().to_string())
}

impl WorkspaceManifest {
    pub fn parent_work_items(&self) -> Vec<WorkspaceWorkItem> {
        let mut normalized = self
            .work_items
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|item| !item.id.trim().is_empty())
            .collect::<Vec<_>>();

        if normalized.is_empty() {
            normalized.push(WorkspaceWorkItem {
                id: self.work_item_id.clone(),
                kind: self.work_item_type.clone(),
                title: self.work_item_title.clone(),
                state: self.work_item_state.clone(),
            });
            return normalized;
        }

        if !normalized
            .iter()
            .any(|item| item.id.eq_ignore_ascii_case(&self.work_item_id))
        {
            normalized.insert(
                0,
                WorkspaceWorkItem {
                    id: self.work_item_id.clone(),
                    kind: self.work_item_type.clone(),
                    title: self.work_item_title.clone(),
                    state: self.work_item_state.clone(),
                },
            );
            return normalized;
        }

        normalized.sort_by_key(|item| !item.id.eq_ignore_ascii_case(&self.work_item_id));
        normalized
    }

    pub fn primary_work_item_id(&self) -> String {
        self.parent_work_items()[0].id.clone()
    }

    pub fn display_work_item_ids(&self) -> String {
        self.parent_work_items()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn display_work_items(&self) -> String {
        self.parent_work_items()
            .into_iter()
            .map(|item| format_work_item(&item))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn normalized_child_tasks(&self) -> Vec<WorkspaceChildTask> {
        let mut normalized = self
            .child_tasks
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|task| !task.id.trim().is_empty() && !task.repository.trim().is_empty())
            .collect::<Vec<_>>();

        if let Some(child_task_ids) = &self.child_task_ids {
            for (repository, id) in child_task_ids {
                if repository.trim().is_empty() || id.trim().is_empty() {
                    continue;
                }

                if normalized
                    .iter()
                    .any(|task| task.id.eq_ignore_ascii_case(id))
                {
                    continue;
                }

                normalized.push(WorkspaceChildTask {
                    repository: repository.clone(),
                    id: id.clone(),
                    title: None,
                });
            }
        }

        normalized
    }

    pub fn legacy_child_task_ids(&self) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();
        for task in self.normalized_child_tasks() {
            result.entry(task.repository).or_insert(task.id);
        }
        result
    }

    pub fn all_known_work_item_ids(&self) -> Vec<String> {
        let mut ids = self
            .parent_work_items()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();

        if let Some(task_id) = &self.task_id
            && !task_id.trim().is_empty()
        {
            ids.push(task_id.clone());
        }

        for child_task in self.normalized_child_tasks() {
            if !ids.iter().any(|id| id.eq_ignore_ascii_case(&child_task.id)) {
                ids.push(child_task.id);
            }
        }

        ids
    }

    pub fn matches_work_item(&self, work_item_id: &str) -> bool {
        self.all_known_work_item_ids()
            .iter()
            .any(|id| id.eq_ignore_ascii_case(work_item_id))
    }
}

pub fn try_parse_summary(
    text: &str,
    expected_repository: &str,
) -> Result<WorkspaceHandoffSummary, String> {
    let normalized = text.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let start = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("```yaml"))
        .ok_or_else(|| "bloc ```yaml absent".to_string())?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim() == "```")
        .map(|(index, _)| index)
        .ok_or_else(|| "fin du bloc yaml absente".to_string())?;

    let mut status = String::new();
    let mut repository = String::new();
    let mut current_section: Option<String> = None;
    let mut current_key: Option<String> = None;
    let mut sections = build_sections();

    for line in lines.iter().take(end).skip(start + 1) {
        if line.trim().is_empty() {
            continue;
        }

        let indent = line.chars().take_while(|c| *c == ' ').count();
        let trimmed = line.trim();
        if indent == 0 {
            current_key = None;
            if ["summary:", "verification:", "artifacts:"].contains(&trimmed) {
                current_section = Some(trimmed.trim_end_matches(':').to_string());
                continue;
            }

            if let Some((key, value)) = split_key_value(trimmed) {
                if key.eq_ignore_ascii_case("status") {
                    status = value;
                } else if key.eq_ignore_ascii_case("repository") {
                    repository = value;
                }
            }
            continue;
        }

        if indent == 2 {
            let Some(section) = current_section.clone() else {
                return Err(format!("section inconnue autour de '{trimmed}'"));
            };
            let Some((key, value)) = split_key_value(trimmed) else {
                return Err(format!("cle inconnue dans {section}: '{trimmed}'"));
            };
            let bucket = sections
                .get_mut(&section)
                .ok_or_else(|| format!("section inconnue autour de '{trimmed}'"))?;
            let list = bucket
                .get_mut(&key)
                .ok_or_else(|| format!("cle inconnue dans {section}: '{trimmed}'"))?;
            current_key = Some(key);
            if value != "[]" && !trim_scalar(&value).is_empty() {
                list.push(trim_scalar(&value));
            }
            continue;
        }

        if indent >= 4 && trimmed.starts_with("- ") {
            let Some(section) = current_section.clone() else {
                return Err(format!("element de liste hors section: '{trimmed}'"));
            };
            let Some(key) = current_key.clone() else {
                return Err(format!("element de liste hors section: '{trimmed}'"));
            };
            sections
                .get_mut(&section)
                .and_then(|bucket| bucket.get_mut(&key))
                .ok_or_else(|| format!("element de liste hors section: '{trimmed}'"))?
                .push(trim_scalar(trimmed.trim_start_matches("- ")));
            continue;
        }

        return Err(format!("ligne handoff non supportée: '{trimmed}'"));
    }

    if status.trim().is_empty() {
        return Err("status absent".into());
    }

    if repository.trim().is_empty() {
        return Err("repository absent".into());
    }

    if !repository.eq_ignore_ascii_case(expected_repository) {
        return Err(format!(
            "repository attendu '{}', trouvé '{}'",
            expected_repository, repository
        ));
    }

    Ok(WorkspaceHandoffSummary {
        repository,
        status,
        done: sections["summary"]["done"].clone(),
        decisions: sections["summary"]["decisions"].clone(),
        risks: sections["summary"]["risks"].clone(),
        blockers: sections["summary"]["blockers"].clone(),
        follow_up: sections["summary"]["follow_up"].clone(),
    })
}

fn build_sections() -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    BTreeMap::from([
        (
            "summary".into(),
            BTreeMap::from([
                ("done".into(), Vec::new()),
                ("decisions".into(), Vec::new()),
                ("risks".into(), Vec::new()),
                ("blockers".into(), Vec::new()),
                ("follow_up".into(), Vec::new()),
            ]),
        ),
        (
            "verification".into(),
            BTreeMap::from([
                ("commands".into(), Vec::new()),
                ("manual_checks".into(), Vec::new()),
            ]),
        ),
        (
            "artifacts".into(),
            BTreeMap::from([
                ("files".into(), Vec::new()),
                ("screenshots".into(), Vec::new()),
                ("attachments".into(), Vec::new()),
            ]),
        ),
    ])
}

fn split_key_value(value: &str) -> Option<(String, String)> {
    let separator = value.find(':')?;
    Some((
        value[..separator].trim().to_string(),
        value[(separator + 1)..].trim().to_string(),
    ))
}

fn trim_scalar(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return trimmed[1..trimmed.len() - 1].to_string();
    }

    trimmed.to_string()
}

fn distinct_repositories(repositories: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for repository in repositories {
        if !result
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(repository))
        {
            result.push(repository.clone());
        }
    }
    result
}

fn format_work_item(item: &WorkspaceWorkItem) -> String {
    display_work_item(item, false)
}

fn resolve_work_item_ids(
    work_item: Option<&str>,
    positional_work_item: Option<&str>,
) -> Result<Option<String>, WorkspaceError> {
    let normalized_option = normalize_work_item_selection(work_item);
    let normalized_positional = normalize_work_item_selection(positional_work_item);

    match (normalized_option, normalized_positional) {
        (Some(option), Some(positional)) if option != positional => {
            Err(WorkspaceError::ConflictingWorkItemSelection)
        }
        (Some(option), Some(_)) => Ok(Some(option)),
        (Some(option), None) => Ok(Some(option)),
        (None, Some(positional)) => Ok(Some(positional)),
        (None, None) => Ok(None),
    }
}

fn normalize_work_item_selection(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let mut ids = value
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    Some(ids.join(","))
}

pub fn parse_work_item_ids(value: &str) -> Vec<String> {
    parse_work_item_selection(Some(value))
        .unwrap_or_default()
        .into_iter()
        .map(|id| id.to_string())
        .collect()
}

fn parse_work_item_selection(value: Option<&str>) -> Option<Vec<WorkItemId>> {
    let normalized = normalize_work_item_selection(value)?;
    Some(normalized.split(',').map(WorkItemId::from).collect())
}

fn resolve_repositories(project_config: Option<&ProjectConfig>, only: Option<&str>) -> Vec<String> {
    if let Some(only) = only.filter(|value| !value.trim().is_empty()) {
        return only
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect();
    }

    if let Some(project_config) = project_config
        && !project_config.repositories.is_empty()
    {
        return project_config.repositories.keys().cloned().collect();
    }

    vec!["front".into(), "back".into()]
}

fn reject_workspace_conflicts(
    root: &str,
    project: &str,
    work_item_ids: &[String],
) -> Result<(), WorkspaceError> {
    let conflicts = find_workspaces(root)
        .into_iter()
        .filter(|workspace| workspace.manifest.project.eq_ignore_ascii_case(project))
        .filter_map(|workspace| {
            let matching = work_item_ids
                .iter()
                .filter(|id| workspace.manifest.matches_work_item(id))
                .cloned()
                .collect::<Vec<_>>();
            if matching.is_empty() {
                None
            } else {
                Some((workspace.path, matching))
            }
        })
        .collect::<Vec<_>>();

    if conflicts.is_empty() {
        return Ok(());
    }

    let details = conflicts
        .into_iter()
        .map(|(path, ids)| format!("{} déjà présent(s) dans {}", ids.join(", "), path))
        .collect::<Vec<_>>()
        .join("; ");
    Err(WorkspaceError::WorkspaceConflict(details))
}

fn resolve_workspace_repositories(
    manifest: &WorkspaceManifest,
    only: Option<&str>,
) -> Result<Vec<String>, WorkspaceError> {
    if let Some(only) = only.filter(|value| !value.trim().is_empty()) {
        let selected = only
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let unknown = selected
            .iter()
            .filter(|repository| {
                !manifest
                    .repositories
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(repository))
            })
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(WorkspaceError::MissingWorkspaceRepository(
                unknown.join(", "),
            ));
        }
        return Ok(selected);
    }

    Ok(manifest.repositories.clone())
}

fn commit_prefix(kind: &str) -> String {
    match kind.to_ascii_lowercase().as_str() {
        "feat" | "fix" | "bug" | "chore" | "refactor" | "test" => kind.to_ascii_lowercase(),
        _ => kind.to_ascii_lowercase(),
    }
}

fn commit_ids(manifest: &WorkspaceManifest) -> Vec<String> {
    distinct_non_empty_owned(
        manifest
            .parent_work_items()
            .into_iter()
            .map(|item| item.id)
            .chain(manifest.task_id.clone())
            .chain(
                manifest
                    .normalized_child_tasks()
                    .into_iter()
                    .map(|task| task.id),
            )
            .map(|id| format!("#{id}"))
            .collect(),
    )
}

fn commit_ids_for_reference(manifest: &WorkspaceManifest) -> Vec<String> {
    distinct_non_empty_owned(
        manifest
            .task_id
            .clone()
            .into_iter()
            .chain(std::iter::once(manifest.work_item_id.clone()))
            .chain(manifest.parent_work_items().into_iter().map(|item| item.id))
            .chain(
                manifest
                    .normalized_child_tasks()
                    .into_iter()
                    .map(|task| task.id),
            )
            .collect(),
    )
}

fn build_work_item_update_plan(
    root: &str,
    workspace: &str,
    manifest: &WorkspaceManifest,
    work_items: Vec<WorkspaceWorkItem>,
) -> Result<TaskWorkItemUpdatePlan, WorkspaceError> {
    let _first = work_items.first().ok_or(WorkspaceError::EmptyWorkItemSet)?;
    let parent_ids = work_items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let mut branch_ids = parent_ids.clone();
    if let Some(task_id) = &manifest.task_id
        && !task_id.trim().is_empty()
    {
        branch_ids.push(task_id.clone());
    }
    branch_ids.extend(
        manifest
            .normalized_child_tasks()
            .into_iter()
            .map(|task| task.id),
    );
    let new_branch = build_branch_name(&manifest.kind, &branch_ids, &manifest.slug);
    let new_workspace = Path::new(workspace)
        .parent()
        .unwrap_or_else(|| Path::new(root))
        .join(build_subject_name(
            &manifest.kind,
            &parent_ids,
            &manifest.slug,
        ))
        .display()
        .to_string();

    Ok(TaskWorkItemUpdatePlan {
        workspace: workspace.into(),
        new_workspace,
        old_branch: manifest.branch_name.clone(),
        new_branch,
        work_items,
    })
}

fn reject_work_item_conflicts(
    root: &str,
    current_workspace: &str,
    project: &str,
    ids: &[String],
) -> Result<(), WorkspaceError> {
    if ids.is_empty() {
        return Ok(());
    }

    let conflicts = find_workspaces(root)
        .into_iter()
        .filter(|workspace| !workspace.path.eq_ignore_ascii_case(current_workspace))
        .filter(|workspace| workspace.manifest.project.eq_ignore_ascii_case(project))
        .filter(|workspace| {
            ids.iter()
                .any(|id| workspace.manifest.matches_work_item(id))
        })
        .map(|workspace| workspace.path)
        .collect::<Vec<_>>();

    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(WorkspaceError::WorkspaceConflict(conflicts.join("; ")))
    }
}

fn distinct_non_empty_owned(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if value.trim().is_empty() {
            continue;
        }
        if !result
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(&value))
        {
            result.push(value);
        }
    }
    result
}

fn require_git_dir(step: &WorkspaceTeardownStep) -> Result<&str, WorkspaceError> {
    step.git_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| WorkspaceError::TeardownFailed {
            repository: step.repository.clone(),
            message: "gitDir manquant".into(),
        })
}

fn collect_manifests(root: &Path, entries: &mut Vec<WorkspaceSummary>) {
    let Ok(read_dir) = fs::read_dir(root) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let manifest_path = path.join("task.json");
            if manifest_path.exists() {
                if let Ok(manifest) = read_manifest(&manifest_path) {
                    entries.push(WorkspaceSummary {
                        path: path.display().to_string(),
                        manifest,
                    });
                }
            } else {
                collect_manifests(&path, entries);
            }
        }
    }
}

fn read_manifest(path: &Path) -> Result<WorkspaceManifest, WorkspaceError> {
    let manifest_text = fs::read_to_string(path)
        .map_err(|_| WorkspaceError::InvalidManifest(path.display().to_string()))?;
    serde_json::from_str(&manifest_text)
        .map_err(|_| WorkspaceError::InvalidManifest(path.display().to_string()))
}

fn write_text(path: &Path, content: &str) -> Result<(), WorkspaceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| WorkspaceError::MissingWorkspace(parent.display().to_string()))?;
    }

    fs::write(path, content)
        .map_err(|_| WorkspaceError::MissingWorkspace(path.display().to_string()))
}

fn current_timestamp_string() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn build_stale_context_issues(
    ai_context: &AdoAiContextItem,
    manifest: &WorkspaceManifest,
) -> Vec<TaskPreflightIssue> {
    let Some(manifest_item) = manifest
        .parent_work_items()
        .into_iter()
        .find(|item| item.id == ai_context.work_item.id.as_str())
    else {
        return vec![];
    };

    let mut stale_reasons = Vec::new();
    if manifest_item.title != ai_context.work_item.title {
        stale_reasons.push("titre local différent d'ADO".to_string());
    }
    if manifest_item.state != ai_context.work_item.state {
        stale_reasons.push("état local différent d'ADO".to_string());
    }
    if manifest_item.kind != ai_context.work_item.kind {
        stale_reasons.push("type local différent d'ADO".to_string());
    }
    if stale_reasons.is_empty() {
        return vec![];
    }

    vec![TaskPreflightIssue {
        code: "workspace.ado-context.stale".into(),
        severity: "warning".into(),
        work_item_id: ai_context.work_item.id.clone(),
        message: format!(
            "Le contexte ADO local du workspace semble stale pour #{}.",
            ai_context.work_item.id
        ),
        details: Some(stale_reasons.join("; ")),
        related_ids: vec![ai_context.work_item.id.clone()],
    }]
}

fn build_attachment_issues(ai_context: &AdoAiContextItem) -> Vec<TaskPreflightIssue> {
    if ai_context.attachments.items.is_empty() {
        return vec![];
    }

    let names = ai_context
        .attachments
        .items
        .iter()
        .filter_map(|item| item.name.clone())
        .collect::<Vec<_>>();

    vec![TaskPreflightIssue {
        code: "ado.attachments.present".into(),
        severity: "warning".into(),
        work_item_id: ai_context.work_item.id.clone(),
        message: format!(
            "Le work item #{} a des pièces jointes à traiter comme source factuelle.",
            ai_context.work_item.id
        ),
        details: Some(if names.is_empty() {
            format!(
                "Pièces jointes présentes. Dossier attendu: {}",
                ai_context.attachments.directory_hint
            )
        } else {
            format!(
                "Pièces jointes présentes: {}. Dossier attendu: {}",
                names.join(", "),
                ai_context.attachments.directory_hint
            )
        }),
        related_ids: vec![ai_context.work_item.id.clone()],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn run_git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn parses_valid_handoff_summary() {
        let text = r#"
# Handoff

```yaml
status: done
repository: front
summary:
  done:
    - "point 1"
  decisions:
    - "decision 1"
  risks: []
  blockers: []
  follow_up:
    - "suite"
verification:
  commands:
    - "dotnet test"
  manual_checks: []
artifacts:
  files:
    - "src/file.ts"
  screenshots: []
  attachments: []
```
"#;

        let summary = try_parse_summary(text, "front").expect("summary should parse");
        assert_eq!(summary.status, "done");
        assert_eq!(summary.repository, "front");
        assert_eq!(summary.done.len(), 1);
        assert_eq!(summary.decisions.len(), 1);
        assert_eq!(summary.follow_up.len(), 1);
    }

    #[test]
    fn rejects_repository_mismatch() {
        let text = r#"
```yaml
status: done
repository: back
summary:
  done: []
  decisions: []
  risks: []
  blockers: []
  follow_up: []
verification:
  commands: []
  manual_checks: []
artifacts:
  files: []
  screenshots: []
  attachments: []
```
"#;

        let error = try_parse_summary(text, "front").expect_err("repository mismatch should fail");
        assert!(error.contains("repository attendu 'front'"));
    }

    #[test]
    fn rejects_unsupported_line() {
        let text = r#"
```yaml
status: done
repository: front
summary:
  done:
    - "ok"
  decisions: []
  risks: []
  blockers: []
  follow_up: []
verification:
  commands: []
  manual_checks: []
artifacts:
  files: []
  screenshots: []
  attachments: []
    stray: nope
```
"#;

        let error = try_parse_summary(text, "front").expect_err("unsupported line should fail");
        assert!(error.contains("ligne handoff non supportée"));
    }

    #[test]
    fn finds_and_filters_workspaces() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "123",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/123-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front"],
  "status": "created"
}"#,
        )
        .expect("manifest should be written");

        let workspaces = find_workspaces(root.to_str().expect("utf8 path"));
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].manifest.project, "ha");

        let filtered = filter_workspaces(workspaces, Some("ha"), Some("123"));
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn task_status_lists_detected_workspace_paths() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "123",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/123-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front"],
  "status": "created"
}"#,
        )
        .expect("manifest should be written");

        let result = task_status(root.to_str().expect("utf8 path"));
        assert_eq!(result, vec![workspace.display().to_string()]);
    }

    #[test]
    fn task_list_returns_expected_display_fields() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "123",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/123-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front"],
  "status": "created",
  "workItems": [
    { "id": "123", "type": "User Story", "title": "Titre HA", "state": "En réalisation" }
  ]
}"#,
        )
        .expect("manifest should be written");

        let result = task_list(root.to_str().expect("utf8 path"), Some("ha"), Some("123"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].project, "ha");
        assert_eq!(result[0].work_item_id, "123");
        assert_eq!(result[0].display_work_items, "#123 Titre HA");
        assert_eq!(result[0].branch_name, "feat/123-demo");
    }

    #[test]
    fn resolves_current_workspace_from_nested_path() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let nested = workspace.join("front/src/app");
        fs::create_dir_all(&nested).expect("nested path should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "123",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/123-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front", "back"],
  "status": "created"
}"#,
        )
        .expect("manifest should be written");

        let current = task_current(nested.to_str().expect("utf8 path"))
            .expect("current workspace should resolve");
        assert_eq!(current.project, "ha");
        assert_eq!(current.primary_work_item_id, "123");
        assert_eq!(current.repositories, vec!["front", "back"]);
    }

    #[test]
    fn handoff_validation_report_is_invalid_when_one_repo_stays_todo() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "123",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/123-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front", "back"],
  "status": "created"
}"#,
        )
        .expect("manifest should be written");
        fs::write(
            workspace.join("handoff-front.md"),
            valid_handoff("front", "done"),
        )
        .expect("front handoff should be written");
        fs::write(
            workspace.join("handoff-back.md"),
            valid_handoff("back", "todo"),
        )
        .expect("back handoff should be written");

        let report = build_handoff_validation_report(workspace.to_str().expect("utf8 path"))
            .expect("report should be built");

        assert!(!report.is_valid);
        assert!(report.items.iter().any(|item| item.status == "todo"));
    }

    #[test]
    fn handoff_validation_report_marks_missing_file_invalid() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "123",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/123-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front"],
  "status": "created"
}"#,
        )
        .expect("manifest should be written");

        let report = build_handoff_validation_report(workspace.to_str().expect("utf8 path"))
            .expect("report should be built");

        assert!(!report.is_valid);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].status, "missing");
    }

    #[test]
    fn resolve_workspace_continue_uses_latest_matching_workspace() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let old_workspace = root.join("projects/ha/workspaces/feat-1-old");
        let new_workspace = root.join("projects/ha/workspaces/feat-2-new");
        fs::create_dir_all(&old_workspace).expect("old workspace should exist");
        fs::create_dir_all(&new_workspace).expect("new workspace should exist");
        fs::write(
            old_workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"1","taskId":null,"project":"ha","type":"feat","slug":"old","branchName":"feat/1-old","createdAt":"2026-06-20T00:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("old manifest should be written");
        fs::write(
            new_workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"2","taskId":null,"project":"ha","type":"feat","slug":"new","branchName":"feat/2-new","createdAt":"2026-06-21T00:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("new manifest should be written");

        let workspace = resolve_workspace(
            root.to_str().expect("utf8 path"),
            None,
            Some("ha"),
            None,
            None,
            true,
        )
        .expect("workspace should resolve");

        assert_eq!(workspace, new_workspace.display().to_string());
    }

    #[test]
    fn resolve_workspace_uses_positional_work_item_id() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-11010-new");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"new","branchName":"feat/11010-new","createdAt":"2026-06-21T00:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");

        let resolved = resolve_workspace(
            root.to_str().expect("utf8 path"),
            None,
            Some("ha"),
            None,
            Some("11010"),
            false,
        )
        .expect("workspace should resolve");

        assert_eq!(resolved, workspace.display().to_string());
    }

    #[test]
    fn resolve_workspace_matches_secondary_work_item_id() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-11010-new");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"new","branchName":"feat/11010-new","createdAt":"2026-06-21T00:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"11010","type":"User Story","title":"Principal","state":"En réalisation"},{"id":"55206","type":"Bug","title":"Secondaire","state":"En développement"}]}"#,
        )
        .expect("manifest should be written");

        let resolved = resolve_workspace(
            root.to_str().expect("utf8 path"),
            None,
            Some("ha"),
            Some("55206"),
            None,
            false,
        )
        .expect("workspace should resolve");

        assert_eq!(resolved, workspace.display().to_string());
    }

    #[test]
    fn resolve_workspace_matches_all_requested_work_items() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-11010-new");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"new","branchName":"feat/11010-new","createdAt":"2026-06-21T00:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"11010","type":"User Story","title":"Principal","state":"En réalisation"},{"id":"55206","type":"Bug","title":"Secondaire","state":"En développement"}]}"#,
        )
        .expect("manifest should be written");

        let resolved = resolve_workspace(
            root.to_str().expect("utf8 path"),
            None,
            Some("ha"),
            Some("55206,11010"),
            None,
            false,
        )
        .expect("workspace should resolve");

        assert_eq!(resolved, workspace.display().to_string());
    }

    #[test]
    fn resolve_workspace_rejects_conflicting_positional_and_option_work_item_ids() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();

        let error = resolve_workspace(
            root.to_str().expect("utf8 path"),
            None,
            Some("ha"),
            Some("55206"),
            Some("11010"),
            false,
        )
        .expect_err("conflicting ids should fail");

        assert!(matches!(
            error,
            WorkspaceError::ConflictingWorkItemSelection
        ));
    }

    #[test]
    fn resolve_open_target_returns_repo_folder() {
        let manifest: WorkspaceManifest = serde_json::from_str(
            r#"{"schema":1,"workItemId":"2","taskId":null,"project":"ha","type":"feat","slug":"new","branchName":"feat/2-new","createdAt":"2026-06-21T00:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should parse");
        let project_config: dw_config::ProjectConfig = serde_json::from_str(
            r#"{"displayName":"HA","repositories":{"front":{"url":"","defaultBranch":"develop","folder":"custom-front"}}}"#,
        )
        .expect("project config should parse");

        let target = resolve_open_target(
            "/tmp/workspace",
            &manifest,
            Some(&project_config),
            Some("front"),
        )
        .expect("target should resolve");

        assert_eq!(target, "/tmp/workspace/custom-front");
    }

    #[test]
    fn plan_task_start_uses_default_project_type_and_fallback_slug() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"default":{"displayName":"Default","repositories":{"front":{"url":"","defaultBranch":"develop"},"back":{"url":"","defaultBranch":"main"}}}}}"#,
        )
        .expect("projects should parse");

        let plan = plan_task_start(TaskStartRequest {
            root: root.to_str().expect("utf8 path"),
            projects: &projects,
            work_item_ids: &[WorkItemId::from("55222")],
            project: None,
            task_id: None,
            type_name: None,
            only: None,
            slug: None,
        })
        .expect("plan should build");

        assert_eq!(plan.project, "default");
        assert_eq!(plan.kind, "feat");
        assert_eq!(plan.slug, "work-item-55222");
        assert_eq!(plan.branch_name, "feat/55222-work-item-55222");
    }

    #[test]
    fn plan_task_start_rejects_existing_workspace_conflict() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"123","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/123-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{"url":"","defaultBranch":"develop"}}}}}"#,
        )
        .expect("projects should parse");

        let error = plan_task_start(TaskStartRequest {
            root: root.to_str().expect("utf8 path"),
            projects: &projects,
            work_item_ids: &[WorkItemId::from("123")],
            project: Some("ha"),
            task_id: None,
            type_name: Some("feat"),
            only: None,
            slug: Some("demo"),
        })
        .expect_err("conflict should be rejected");

        assert!(matches!(error, WorkspaceError::WorkspaceConflict(_)));
    }

    #[test]
    fn plan_task_start_uses_repository_folder_override() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{"url":"","defaultBranch":"develop","folder":"custom-front"}}}}}"#,
        )
        .expect("projects should parse");

        let plan = plan_task_start(TaskStartRequest {
            root: root.to_str().expect("utf8 path"),
            projects: &projects,
            work_item_ids: &[WorkItemId::from("123")],
            project: Some("ha"),
            task_id: None,
            type_name: Some("feat"),
            only: None,
            slug: Some("demo"),
        })
        .expect("plan should build");

        assert_eq!(
            plan.repository_folders.get("front").map(String::as_str),
            Some("custom-front")
        );
        assert_eq!(plan.repository_worktrees.len(), 1);
        assert_eq!(plan.repository_worktrees[0].repository, "front");
        assert!(
            plan.repository_worktrees[0]
                .worktree_path
                .ends_with("custom-front")
        );
        assert_eq!(plan.repository_worktrees[0].default_branch, "develop");
    }

    #[test]
    fn execute_task_start_rejects_unpreparable_repository() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let project_root = temp.path().join("projects/ha");
        let plan = TaskStartPlan {
            work_item_ids: vec!["123".into()],
            primary_work_item_id: "123".into(),
            project: "ha".into(),
            task_id: None,
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/123-demo".into(),
            subject_name: "feat-123-demo".into(),
            workspace: workspace.display().to_string(),
            repositories: vec!["front".into()],
            repository_folders: BTreeMap::from([("front".into(), "front".into())]),
            repository_worktrees: vec![TaskStartRepositoryPlan {
                repository: "front".into(),
                project_root: project_root.display().to_string(),
                worktree_path: workspace.join("front").display().to_string(),
                url: temp.path().join("missing-remote.git").display().to_string(),
                default_branch: "develop".into(),
                anchor_name: "front.git".into(),
                branch_name: "feat/123-demo".into(),
            }],
        };

        let error = execute_task_start(&plan, None, None, None)
            .expect_err("invalid repository must not leave a fake workspace");

        assert!(matches!(
            error,
            WorkspaceError::WorktreePrepareFailed { repository, .. } if repository == "front"
        ));
        assert!(!workspace.join("task.json").exists());
    }

    #[test]
    fn execute_task_start_prepares_bare_repository_and_worktree() {
        let temp = tempdir().expect("tempdir should be created");
        let source = temp.path().join("source-front");
        fs::create_dir_all(&source).expect("source should exist");
        run_git(&source, &["init", "-b", "develop"]);
        fs::write(source.join("README.md"), "front\n").expect("file should be written");
        run_git(&source, &["add", "README.md"]);
        run_git(
            &source,
            &[
                "-c",
                "user.name=dw test",
                "-c",
                "user.email=dw@example.invalid",
                "commit",
                "-m",
                "init",
            ],
        );

        let root = temp.path().join("dw-root");
        let projects: ProjectsConfig = serde_json::from_str(&format!(
            r#"{{
  "projects": {{
    "ha": {{
      "displayName": "HA",
      "repositories": {{
        "front": {{
          "url": "{}",
          "defaultBranch": "develop",
          "anchorName": "front.git",
          "folder": "front"
        }}
      }}
    }}
  }}
}}"#,
            source.display()
        ))
        .expect("projects should parse");
        let plan = plan_task_start(TaskStartRequest {
            root: root.to_str().expect("utf8 path"),
            projects: &projects,
            work_item_ids: &[WorkItemId::from("123")],
            project: Some("ha"),
            task_id: None,
            type_name: Some("feat"),
            only: Some("front"),
            slug: Some("demo"),
        })
        .expect("plan should build");

        execute_task_start(&plan, None, None, None).expect("start should execute");

        let anchor = root.join("projects/ha/repositories/front.git");
        let worktree = root.join("projects/ha/workspaces/feat-123-demo/front");
        assert!(anchor.join("HEAD").exists());
        assert!(worktree.join(".git").exists());
        assert!(worktree.join("README.md").exists());
    }

    #[test]
    fn execute_task_start_writes_manifest_plan_and_handoffs() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let plan = TaskStartPlan {
            work_item_ids: vec!["123".into()],
            primary_work_item_id: "123".into(),
            project: "ha".into(),
            task_id: None,
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/123-demo".into(),
            subject_name: "feat-123-demo".into(),
            workspace: workspace.display().to_string(),
            repositories: vec!["front".into(), "back".into()],
            repository_folders: BTreeMap::from([
                ("front".into(), "front".into()),
                ("back".into(), "back".into()),
            ]),
            repository_worktrees: Vec::new(),
        };

        let manifest = execute_task_start(&plan, None, None, None).expect("start should execute");

        assert_eq!(manifest.project, "ha");
        assert!(workspace.join("task.json").exists());
        assert!(workspace.join("plan.md").exists());
        assert!(workspace.join("front").exists());
        assert!(workspace.join("back").exists());
        assert!(workspace.join("handoff-front.md").exists());
        assert!(workspace.join("handoff-back.md").exists());
        let plan_text =
            fs::read_to_string(workspace.join("plan.md")).expect("plan should be readable");
        assert!(plan_text.contains("# Plan - Work items #123"));
        let handoff_text = fs::read_to_string(workspace.join("handoff-front.md"))
            .expect("handoff should be readable");
        assert!(handoff_text.contains("## Synthèse structurée attendue"));
    }

    #[test]
    fn start_plan_with_child_tasks_updates_branch_and_task_id() {
        let plan = TaskStartPlan {
            work_item_ids: vec!["123".into()],
            primary_work_item_id: "123".into(),
            project: "ha".into(),
            task_id: None,
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/123-demo".into(),
            subject_name: "feat-123-demo".into(),
            workspace: "/tmp/workspace".into(),
            repositories: vec!["front".into()],
            repository_folders: BTreeMap::from([("front".into(), "front".into())]),
            repository_worktrees: Vec::new(),
        };

        let updated = start_plan_with_child_tasks(
            plan,
            &[WorkspaceChildTask {
                repository: "front".into(),
                id: "456".into(),
                title: Some("[FRONT] Demo".into()),
            }],
        );

        assert_eq!(updated.task_id.as_deref(), Some("456"));
        assert_eq!(updated.branch_name, "feat/123-456-demo");
    }

    #[test]
    fn execute_task_start_writes_child_tasks() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let plan = TaskStartPlan {
            work_item_ids: vec!["123".into()],
            primary_work_item_id: "123".into(),
            project: "ha".into(),
            task_id: Some("456".into()),
            kind: "feat".into(),
            slug: "demo".into(),
            branch_name: "feat/123-456-demo".into(),
            subject_name: "feat-123-demo".into(),
            workspace: workspace.display().to_string(),
            repositories: vec!["front".into()],
            repository_folders: BTreeMap::from([("front".into(), "front".into())]),
            repository_worktrees: Vec::new(),
        };

        let manifest = execute_task_start_with_work_items_and_child_tasks(
            &plan,
            vec![WorkspaceWorkItem {
                id: "123".into(),
                kind: Some("User Story".into()),
                title: Some("Demo".into()),
                state: Some("En réalisation".into()),
            }],
            vec![WorkspaceChildTask {
                repository: "front".into(),
                id: "456".into(),
                title: Some("[FRONT] Demo".into()),
            }],
        )
        .expect("start should execute");

        assert_eq!(manifest.normalized_child_tasks().len(), 1);
        assert_eq!(manifest.all_known_work_item_ids(), vec!["123", "456"]);
        assert_eq!(manifest.branch_name, "feat/123-456-demo");
    }

    #[test]
    fn preflight_report_models_warning_issues_from_ai_context_file() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-55201-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{
  "schema": 1,
  "workItemId": "55201",
  "taskId": null,
  "project": "ha",
  "type": "feat",
  "slug": "demo",
  "branchName": "feat/55201-demo",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front"],
  "status": "created",
  "workItems": [
    { "id": "55201", "type": "Bug", "title": "Titre local", "state": "New" }
  ]
}"#,
        )
        .expect("manifest should be written");
        let ai_context_path = temp.path().join("ai-context.json");
        fs::write(
            &ai_context_path,
            r#"{
  "schemaVersion": "dw.ado.ai-context.v1",
  "workItem": {
    "id": "55201",
    "url": "https://dev.azure.com/org/Project/_workitems/edit/55201",
    "title": "Titre ADO",
    "type": "Bug",
    "state": "En developpement",
    "assignedTo": null,
    "areaPath": null,
    "iterationPath": null,
    "tags": []
  },
  "core": {
    "createdBy": null,
    "createdDate": null,
    "changedBy": null,
    "changedDate": null,
    "priority": null,
    "valueArea": null
  },
  "content": {
    "description": null,
    "acceptanceCriteria": null,
    "productContext": {}
  },
  "links": {
    "parentIds": [],
    "childIds": [],
    "predecessorIds": [],
    "successorIds": []
  },
  "attachments": {
    "directoryHint": "attachments/ado/55201",
    "items": [
      { "name": "maquette.png", "url": "https://dev.azure.com/org/_apis/wit/attachments/123", "comment": "Source ecran", "directoryHint": "attachments/ado/55201" }
    ]
  },
  "relations": [],
  "comments": []
}"#,
        )
        .expect("ai context should be written");

        let report = build_preflight_report_from_ai_context_files(
            workspace.to_str().expect("utf8 path"),
            &[ai_context_path.display().to_string()],
        )
        .expect("report should build");

        assert_eq!(report.schema_version, "dw.task.preflight.v1");
        assert!(!report.has_blocking_issues);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.severity == "warning")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "workspace.ado-context.stale")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "ado.attachments.present")
        );
    }

    #[test]
    fn resolve_workspace_for_workspace_command_uses_current_workspace_when_not_continue() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let nested = workspace.join("front/src");
        fs::create_dir_all(&nested).expect("nested path should exist");
        fs::write(workspace.join("task.json"), "{}").expect("manifest should exist");

        let resolved = resolve_workspace_for_workspace_command(
            temp.path().to_str().expect("utf8 path"),
            None,
            false,
            nested.to_str().expect("utf8 path"),
        )
        .expect("workspace should resolve");

        assert_eq!(resolved, workspace.display().to_string());
    }

    #[test]
    fn plan_task_repo_latest_uses_default_branch_and_folder_override() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"123","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/123-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front","back"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{"url":"","defaultBranch":"develop","folder":"custom-front"},"back":{"url":"","defaultBranch":"main"}}}}}"#,
        )
        .expect("projects should parse");

        let (_manifest, targets) = plan_task_repo_latest(
            root.to_str().expect("utf8 path"),
            &projects,
            workspace.to_str().expect("utf8 path"),
            Some("front"),
        )
        .expect("plan should build");

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].repository, "front");
        assert_eq!(targets[0].default_branch, "develop");
        assert!(targets[0].repository_path.ends_with("custom-front"));
    }

    #[test]
    fn plan_task_commit_uses_repository_folder_override() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"123","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/123-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front","back"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{"url":"","defaultBranch":"develop","folder":"custom-front"},"back":{"url":"","defaultBranch":"main"}}}}}"#,
        )
        .expect("projects should parse");

        let (_manifest, targets) =
            plan_task_commit(&projects, workspace.to_str().expect("utf8 path"))
                .expect("plan should build");

        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|target| {
            target.repository == "front" && target.path.ends_with("custom-front")
        }));
        assert!(
            targets
                .iter()
                .any(|target| target.repository == "back" && target.path.ends_with("back"))
        );
    }

    #[test]
    fn build_commit_message_uses_parent_task_and_child_ids() {
        let manifest: WorkspaceManifest = serde_json::from_str(
            r#"{
  "schema": 1,
  "workItemId": "53020",
  "taskId": "53312",
  "project": "he",
  "type": "bug",
  "slug": "corriger-ouverture-dossier",
  "branchName": "bug/53020-53312-55201-corriger-ouverture-dossier",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["front"],
  "status": "created",
  "childTasks": [
    { "repository": "front", "id": "55201", "title": "Verifier UI" }
  ]
}"#,
        )
        .expect("manifest should parse");

        assert_eq!(
            build_commit_message(&manifest, None),
            "bug(#53020 #53312 #55201): corriger-ouverture-dossier"
        );
    }

    #[test]
    fn build_commit_message_supports_multiple_parent_ids() {
        let manifest: WorkspaceManifest = serde_json::from_str(
            r#"{
  "schema": 1,
  "workItemId": "53020",
  "taskId": null,
  "project": "he",
  "type": "bug",
  "slug": "corriger-ouverture-dossier",
  "branchName": "bug/53020-53098-corriger-ouverture-dossier",
  "createdAt": "2026-07-02T10:00:00Z",
  "repositories": ["back"],
  "status": "created",
  "workItems": [
    { "id": "53020" },
    { "id": "53098" }
  ]
}"#,
        )
        .expect("manifest should parse");

        assert_eq!(
            build_commit_message(&manifest, None),
            "bug(#53020 #53098): corriger-ouverture-dossier"
        );
    }

    #[test]
    fn build_commit_message_adds_reference_to_override_when_missing() {
        let manifest: WorkspaceManifest = serde_json::from_str(
            r#"{"schema":1,"workItemId":"27485","taskId":"55201","project":"ha","type":"feat","slug":"descriptif","branchName":"feat/27485-55201-descriptif","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should parse");

        assert_eq!(
            build_commit_message(&manifest, Some("feat: descriptif")),
            "feat: descriptif #55201"
        );
        assert_eq!(
            build_commit_message(&manifest, Some("feat: descriptif #27485")),
            "feat: descriptif #27485"
        );
    }

    #[test]
    fn plan_add_work_items_updates_branch_and_workspace_subject() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-11010-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/11010-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");

        let (_manifest, plan) = plan_add_work_items(
            temp.path().to_str().expect("utf8 path"),
            workspace.to_str().expect("utf8 path"),
            "55206",
            Some("Bug"),
            Some("Secondaire"),
            Some("En developpement"),
        )
        .expect("plan should build");

        assert_eq!(plan.new_branch, "feat/11010-55206-demo");
        assert!(plan.new_workspace.ends_with("feat-11010-55206-demo"));
        assert_eq!(plan.work_items.len(), 2);
        assert_eq!(plan.work_items[1].title.as_deref(), Some("Secondaire"));
    }

    #[test]
    fn plan_add_work_item_snapshots_keeps_ado_metadata() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-11010-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/11010-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");

        let (_manifest, plan) = plan_add_work_item_snapshots(
            temp.path().to_str().expect("utf8 path"),
            workspace.to_str().expect("utf8 path"),
            &[WorkItemSnapshot {
                id: "55206".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Secondaire".into()),
                url: None,
            }],
        )
        .expect("plan should build");

        assert_eq!(plan.new_branch, "feat/11010-55206-demo");
        assert_eq!(plan.work_items[1].kind.as_deref(), Some("Bug"));
        assert_eq!(plan.work_items[1].title.as_deref(), Some("Secondaire"));
        assert_eq!(
            plan.work_items[1].state.as_deref(),
            Some("En developpement")
        );
    }

    #[test]
    fn plan_add_work_item_snapshots_rejects_workspace_conflict() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-11010-demo");
        let other = temp.path().join("projects/ha/workspaces/bug-55206-other");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::create_dir_all(&other).expect("other workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/11010-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        fs::write(
            other.join("task.json"),
            r#"{"schema":1,"workItemId":"55206","taskId":null,"project":"ha","type":"bug","slug":"other","branchName":"bug/55206-other","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("other manifest should be written");

        let error = plan_add_work_item_snapshots(
            temp.path().to_str().expect("utf8 path"),
            workspace.to_str().expect("utf8 path"),
            &[WorkItemSnapshot {
                id: "55206".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Secondaire".into()),
                url: None,
            }],
        )
        .expect_err("conflict should be rejected");

        assert!(error.to_string().contains("bug-55206-other"));
    }

    #[test]
    fn execute_work_item_update_writes_manifest_and_renames_workspace() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-11010-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/11010-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let (manifest, plan) = plan_add_work_items(
            temp.path().to_str().expect("utf8 path"),
            workspace.to_str().expect("utf8 path"),
            "55206",
            None,
            None,
            None,
        )
        .expect("plan should build");

        let (updated, new_workspace) =
            execute_work_item_update(&manifest, &plan).expect("update should execute");

        assert_eq!(updated.branch_name, "feat/11010-55206-demo");
        assert!(!workspace.exists());
        assert!(
            std::path::Path::new(&new_workspace)
                .join("task.json")
                .exists()
        );
        assert!(
            std::path::Path::new(&new_workspace)
                .join("handoff-front.md")
                .exists()
        );
    }

    #[test]
    fn plan_remove_work_items_rejects_empty_parent_set() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-11010-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"11010","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/11010-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");

        let error = plan_remove_work_items(
            temp.path().to_str().expect("utf8 path"),
            workspace.to_str().expect("utf8 path"),
            "11010",
        )
        .expect_err("removing every parent should fail");

        assert!(matches!(error, WorkspaceError::EmptyWorkItemSet));
    }

    #[test]
    fn plan_task_add_repo_uses_config_folder_anchor_and_branch() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"123","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/123-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"db":{"url":"https://example/db.git","defaultBranch":"develop","anchorName":"database.git","folder":"database"}}}}}"#,
        )
        .expect("projects should parse");

        let (_manifest, plan) = plan_task_add_repo(
            root.to_str().expect("utf8 path"),
            &projects,
            workspace.to_str().expect("utf8 path"),
            "db",
        )
        .expect("plan should build");

        assert_eq!(plan.repository, "db");
        assert_eq!(plan.default_branch, "develop");
        assert_eq!(plan.anchor_name, "database.git");
        assert_eq!(plan.branch_name, "feat/123-demo");
        assert!(plan.worktree_path.ends_with("database"));
        assert_eq!(plan.repositories, vec!["front", "db"]);
    }

    #[test]
    fn execute_task_add_repo_updates_manifest_and_handoff() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"123","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/123-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"db":{"url":"","defaultBranch":"main"}}}}}"#,
        )
        .expect("projects should parse");
        let (manifest, plan) = plan_task_add_repo(
            root.to_str().expect("utf8 path"),
            &projects,
            workspace.to_str().expect("utf8 path"),
            "db",
        )
        .expect("plan should build");

        let updated = execute_task_add_repo(&manifest, &plan).expect("add repo should execute");

        assert_eq!(updated.repositories, vec!["front", "db"]);
        let manifest_text = fs::read_to_string(workspace.join("task.json")).expect("manifest");
        assert!(manifest_text.contains("db"));
        assert!(workspace.join("handoff-db.md").exists());
    }

    #[test]
    fn execute_task_sync_updates_parent_work_items_from_snapshots() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"123","taskId":null,"project":"ha","type":"feat","slug":"demo","branchName":"feat/123-demo","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"123","type":"Bug","title":"Ancien","state":"New"}]}"#,
        )
        .expect("manifest should be written");

        let updated = execute_task_sync(
            workspace.to_str().expect("utf8 path"),
            &[WorkItemSnapshot {
                id: "123".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Titre ADO".into()),
                url: Some("https://dev.azure.com/org/project/_workitems/edit/123".into()),
            }],
        )
        .expect("sync should execute");

        assert_eq!(updated.work_item_title.as_deref(), Some("Titre ADO"));
        assert_eq!(updated.work_item_state.as_deref(), Some("En developpement"));
        let manifest_text = fs::read_to_string(workspace.join("task.json")).expect("manifest");
        assert!(manifest_text.contains("Titre ADO"));
    }

    #[test]
    fn display_work_item_includes_title_and_state_when_requested() {
        let text = display_work_item(
            &WorkspaceWorkItem {
                id: "55206".into(),
                kind: Some("Bug".into()),
                title: Some("Heures PSFs incoherentes affichees".into()),
                state: Some("Valide".into()),
            },
            true,
        );

        assert_eq!(text, "#55206 Heures PSFs incoherentes affichees [Valide]");
    }

    #[test]
    fn display_work_items_joins_multiple_items_with_titles() {
        let text = display_work_items(
            &[
                WorkspaceWorkItem {
                    id: "26999".into(),
                    kind: Some("User Story".into()),
                    title: Some("Edition de la demande de transport".into()),
                    state: Some("En realisation".into()),
                },
                WorkspaceWorkItem {
                    id: "55264".into(),
                    kind: Some("Task".into()),
                    title: Some("Transmission automatique du dossier".into()),
                    state: Some("En realisation".into()),
                },
            ],
            true,
        );

        assert_eq!(
            text,
            "#26999 Edition de la demande de transport [En realisation], #55264 Transmission automatique du dossier [En realisation]"
        );
    }

    #[test]
    fn plan_task_prune_keeps_only_workspaces_with_all_parents_final() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let final_workspace = root.join("projects/ha/workspaces/feat-1-final");
        let active_workspace = root.join("projects/ha/workspaces/feat-2-active");
        fs::create_dir_all(&final_workspace).expect("final workspace should exist");
        fs::create_dir_all(&active_workspace).expect("active workspace should exist");
        fs::write(
            final_workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"1","taskId":null,"project":"ha","type":"feat","slug":"final","branchName":"feat/1-final","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"1","type":"User Story","title":"Done","state":"Valide"}]}"#,
        )
        .expect("final manifest should be written");
        fs::write(
            active_workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"2","taskId":null,"project":"ha","type":"feat","slug":"active","branchName":"feat/2-active","createdAt":"2026-07-02T10:00:00Z","repositories":["front"],"status":"created","workItems":[{"id":"2","type":"User Story","title":"Active","state":"En realisation"}]}"#,
        )
        .expect("active manifest should be written");

        let candidates = plan_task_prune(root.to_str().expect("utf8 path"), Some("ha"), None);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].path, final_workspace.display().to_string());
    }

    #[test]
    fn plan_task_teardown_removes_each_repo_worktree_and_prunes_git_anchors() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-55222-slug");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        fs::write(
            workspace.join("task.json"),
            r#"{"schema":1,"workItemId":"55222","taskId":null,"project":"ha","type":"feat","slug":"slug","branchName":"feat/55222-slug","createdAt":"2026-06-22T12:00:00Z","repositories":["front"],"status":"created"}"#,
        )
        .expect("manifest should be written");
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{"projects":{"ha":{"displayName":"HA","repositories":{"front":{"url":"https://example/front.git","defaultBranch":"develop","anchorName":"front.git","folder":"front"}}}}}"#,
        )
        .expect("projects should parse");

        let (_manifest, steps) = plan_task_teardown(
            root.to_str().expect("utf8 path"),
            &projects,
            workspace.to_str().expect("utf8 path"),
        )
        .expect("plan should build");
        let anchor = root.join("projects/ha/repositories/front.git");

        assert!(steps.iter().any(|step| {
            step.repository == "front"
                && step.action == "worktree remove"
                && step.target == workspace.join("front").display().to_string()
                && step.git_dir.as_deref() == Some(anchor.to_str().expect("utf8 path"))
        }));
        assert!(steps.iter().any(|step| {
            step.repository == "front"
                && step.action == "worktree prune"
                && step.target == anchor.display().to_string()
                && step.git_dir.as_deref() == Some(anchor.to_str().expect("utf8 path"))
        }));
        assert!(steps.iter().any(|step| {
            step.repository == "workspace"
                && step.action == "delete directory"
                && step.target == workspace.display().to_string()
        }));
    }

    #[test]
    fn execute_task_teardown_runs_git_from_anchor_and_deletes_workspace() {
        let temp = tempdir().expect("tempdir should be created");
        let root = temp.path();
        let workspace = root.join("projects/ha/workspaces/feat-55222-slug");
        let anchor = root.join("projects/ha/repositories/front.git");
        fs::create_dir_all(workspace.join("front")).expect("worktree should exist");
        fs::create_dir_all(&anchor).expect("anchor should exist");
        let steps = vec![
            WorkspaceTeardownStep {
                repository: "front".into(),
                action: "worktree remove".into(),
                target: workspace.join("front").display().to_string(),
                git_dir: Some(anchor.display().to_string()),
            },
            WorkspaceTeardownStep {
                repository: "front".into(),
                action: "worktree prune".into(),
                target: anchor.display().to_string(),
                git_dir: Some(anchor.display().to_string()),
            },
            WorkspaceTeardownStep {
                repository: "workspace".into(),
                action: "delete directory".into(),
                target: workspace.display().to_string(),
                git_dir: None,
            },
        ];
        let mut calls: Vec<(String, Vec<String>)> = Vec::new();

        execute_task_teardown(
            workspace.to_str().expect("utf8 path"),
            &steps,
            |git_dir, args| {
                calls.push((
                    git_dir.to_string(),
                    args.iter().map(|arg| arg.to_string()).collect(),
                ));
                Ok(())
            },
        )
        .expect("teardown should execute");

        assert!(calls.iter().any(|(git_dir, args)| {
            git_dir == anchor.to_str().expect("utf8 path")
                && args
                    == &vec![
                        "worktree".to_string(),
                        "remove".to_string(),
                        "--force".to_string(),
                        workspace.join("front").display().to_string(),
                    ]
        }));
        assert!(calls.iter().any(|(git_dir, args)| {
            git_dir == anchor.to_str().expect("utf8 path")
                && args == &vec!["worktree".to_string(), "prune".to_string()]
        }));
        assert!(!workspace.exists());
    }

    #[test]
    fn execute_task_teardown_skips_prune_when_anchor_is_missing() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace should exist");
        let missing_anchor = temp.path().join("missing/front.git");
        let steps = vec![
            WorkspaceTeardownStep {
                repository: "front".into(),
                action: "worktree prune".into(),
                target: missing_anchor.display().to_string(),
                git_dir: Some(missing_anchor.display().to_string()),
            },
            WorkspaceTeardownStep {
                repository: "workspace".into(),
                action: "delete directory".into(),
                target: workspace.display().to_string(),
                git_dir: None,
            },
        ];
        let mut calls = 0;

        execute_task_teardown(
            workspace.to_str().expect("utf8 path"),
            &steps,
            |_git_dir, _args| {
                calls += 1;
                Ok(())
            },
        )
        .expect("teardown should skip missing prune anchor");

        assert_eq!(calls, 0);
        assert!(!workspace.exists());
    }

    fn valid_handoff(repository: &str, status: &str) -> String {
        format!(
            r#"```yaml
status: {status}
repository: {repository}
summary:
  done:
    - "ok"
  decisions: []
  risks: []
  blockers: []
  follow_up: []
verification:
  commands: []
  manual_checks: []
artifacts:
  files: []
  screenshots: []
  attachments: []
```"#
        )
    }
}
