use dw_ado::WorkItemSnapshot;
use dw_config::{
    ProjectConfig, ProjectsConfig, RepositoryConfig, repository_config, resolve_project,
};
use dw_contracts::{
    AdoAiContextItem, HANDOFF_PREFIX, HANDOFF_VALIDATION_VERSION, MARKDOWN_EXTENSION,
    PREFLIGHT_VERSION, TaskHandoffValidationDetail, TaskHandoffValidationItem,
    TaskHandoffValidationReport, TaskHandoffValidationStatus, TaskPreflightIssue,
    TaskPreflightIssueCode, TaskPreflightIssueDetail, TaskPreflightReport, TaskPreflightSeverity,
    TaskPreflightStaleReason,
};
use dw_core::{
    AiContextFilePath, BranchName, CommitMessage, GitAnchorName, GitRemoteUrl, HandoffFilePath,
    HandoffParseError, ProjectKey, ProjectRootPath, RepositoryPath, SecretKey, TaskId, TaskSlug,
    TaskSubjectName, Timestamp, WorkItemId, WorkItemState, WorkItemTitle, WorkItemTypeName,
    WorkspaceOperationError, WorkspacePath, WorkspaceRepositoryName,
};
use dw_git::{
    GitCredential, WorktreePrepareRequest, build_branch_name, build_subject_name, prepare_worktree,
    slug_from_phrase_or_fallback,
};
use dw_secret::{KeyringSecretStore, SecretStore};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
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
    pub work_item_id: WorkItemId,
    #[serde(rename = "taskId")]
    pub task_id: Option<TaskId>,
    pub project: ProjectKey,
    #[serde(rename = "type")]
    pub kind: WorkItemTypeName,
    pub slug: TaskSlug,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    #[serde(rename = "createdAt")]
    pub created_at: Timestamp,
    pub repositories: Vec<WorkspaceRepositoryName>,
    pub status: WorkspaceManifestStatus,
    #[serde(rename = "workItemType")]
    pub work_item_type: Option<WorkItemTypeName>,
    #[serde(rename = "workItemTitle")]
    pub work_item_title: Option<WorkItemTitle>,
    #[serde(rename = "workItemState")]
    pub work_item_state: Option<WorkItemState>,
    #[serde(rename = "childTaskIds")]
    pub child_task_ids: Option<BTreeMap<WorkspaceRepositoryName, WorkItemId>>,
    #[serde(rename = "childTasks")]
    pub child_tasks: Option<Vec<WorkspaceChildTask>>,
    #[serde(rename = "workItems")]
    pub work_items: Option<Vec<WorkspaceWorkItem>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceManifestStatus {
    Created,
}

impl WorkspaceManifestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
        }
    }
}

impl fmt::Display for WorkspaceManifestStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceWorkItem {
    pub id: WorkItemId,
    #[serde(rename = "type")]
    pub kind: Option<WorkItemTypeName>,
    pub title: Option<WorkItemTitle>,
    pub state: Option<WorkItemState>,
}

impl fmt::Display for WorkspaceWorkItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self
            .title
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "(sans titre)".into());
        match &self.state {
            Some(state) => write!(formatter, "#{} {} [{}]", self.id, title, state),
            None => write!(formatter, "#{} {}", self.id, title),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceChildTask {
    pub repository: WorkspaceRepositoryName,
    pub id: WorkItemId,
    pub title: Option<WorkItemTitle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub path: WorkspacePath,
    pub manifest: WorkspaceManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskListItem {
    pub path: WorkspacePath,
    pub project: ProjectKey,
    #[serde(rename = "workItemId")]
    pub work_item_id: WorkItemId,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
    #[serde(rename = "taskId")]
    pub task_id: Option<TaskId>,
    #[serde(rename = "allKnownWorkItemIds")]
    pub all_known_work_item_ids: Vec<WorkItemId>,
    #[serde(rename = "type")]
    pub kind: WorkItemTypeName,
    pub slug: TaskSlug,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    #[serde(rename = "createdAt")]
    pub created_at: Timestamp,
    #[serde(rename = "workItemType")]
    pub work_item_type: Option<WorkItemTypeName>,
    #[serde(rename = "workItemTitle")]
    pub work_item_title: Option<WorkItemTitle>,
    #[serde(rename = "workItemState")]
    pub work_item_state: Option<WorkItemState>,
    pub repositories: Vec<WorkspaceRepositoryName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCurrentItem {
    pub workspace: WorkspacePath,
    pub project: ProjectKey,
    #[serde(rename = "primaryWorkItemId")]
    pub primary_work_item_id: WorkItemId,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
    #[serde(rename = "taskId")]
    pub task_id: Option<TaskId>,
    #[serde(rename = "childTaskIds")]
    pub child_task_ids: BTreeMap<WorkspaceRepositoryName, WorkItemId>,
    #[serde(rename = "childTasks")]
    pub child_tasks: Vec<WorkspaceChildTask>,
    pub branch: BranchName,
    pub repositories: Vec<WorkspaceRepositoryName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskStartPlan {
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<WorkItemId>,
    #[serde(rename = "primaryWorkItemId")]
    pub primary_work_item_id: WorkItemId,
    pub project: ProjectKey,
    #[serde(rename = "taskId")]
    pub task_id: Option<TaskId>,
    #[serde(rename = "type")]
    pub kind: WorkItemTypeName,
    pub slug: TaskSlug,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    #[serde(rename = "subjectName")]
    pub subject_name: TaskSubjectName,
    pub workspace: WorkspacePath,
    pub repositories: Vec<WorkspaceRepositoryName>,
    #[serde(rename = "repositoryFolders")]
    pub repository_folders: BTreeMap<WorkspaceRepositoryName, RepositoryPath>,
    #[serde(rename = "repositoryWorktrees")]
    pub repository_worktrees: Vec<TaskStartRepositoryPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskStartRepositoryPlan {
    pub repository: WorkspaceRepositoryName,
    #[serde(rename = "projectRoot")]
    pub project_root: ProjectRootPath,
    #[serde(rename = "worktreePath")]
    pub worktree_path: RepositoryPath,
    pub url: GitRemoteUrl,
    #[serde(rename = "defaultBranch")]
    pub default_branch: BranchName,
    #[serde(rename = "anchorName")]
    pub anchor_name: GitAnchorName,
    #[serde(rename = "gitCredentialSecret")]
    pub git_credential_secret: Option<SecretKey>,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRenamePlan {
    pub workspace: WorkspacePath,
    #[serde(rename = "newWorkspace")]
    pub new_workspace: WorkspacePath,
    #[serde(rename = "oldSlug")]
    pub old_slug: TaskSlug,
    #[serde(rename = "newSlug")]
    pub new_slug: TaskSlug,
    #[serde(rename = "oldBranch")]
    pub old_branch: BranchName,
    #[serde(rename = "newBranch")]
    pub new_branch: BranchName,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRepoLatestTarget {
    pub repository: WorkspaceRepositoryName,
    pub repository_path: RepositoryPath,
    #[serde(rename = "defaultBranch")]
    pub default_branch: BranchName,
    #[serde(rename = "gitCredentialSecret")]
    pub git_credential_secret: Option<SecretKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCommitTarget {
    pub repository: WorkspaceRepositoryName,
    pub path: RepositoryPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskWorkItemUpdatePlan {
    pub workspace: WorkspacePath,
    #[serde(rename = "newWorkspace")]
    pub new_workspace: WorkspacePath,
    #[serde(rename = "oldBranch")]
    pub old_branch: BranchName,
    #[serde(rename = "newBranch")]
    pub new_branch: BranchName,
    #[serde(rename = "workItems")]
    pub work_items: Vec<WorkspaceWorkItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskAddRepoPlan {
    pub workspace: WorkspacePath,
    pub repository: WorkspaceRepositoryName,
    #[serde(rename = "projectRoot")]
    pub project_root: ProjectRootPath,
    #[serde(rename = "worktreePath")]
    pub worktree_path: RepositoryPath,
    pub url: GitRemoteUrl,
    #[serde(rename = "defaultBranch")]
    pub default_branch: BranchName,
    #[serde(rename = "anchorName")]
    pub anchor_name: GitAnchorName,
    #[serde(rename = "gitCredentialSecret")]
    pub git_credential_secret: Option<SecretKey>,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    pub repositories: Vec<WorkspaceRepositoryName>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceTeardownStep {
    pub subject: WorkspaceTeardownSubject,
    pub action: WorkspaceTeardownAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkspaceTeardownSubject {
    Workspace,
    Repository { repository: WorkspaceRepositoryName },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkspaceTeardownAction {
    WorktreeRemove {
        #[serde(rename = "worktreePath")]
        worktree_path: RepositoryPath,
        #[serde(rename = "gitDir")]
        git_dir: RepositoryPath,
    },
    WorktreePrune {
        #[serde(rename = "gitDir")]
        git_dir: RepositoryPath,
    },
    DeleteWorkspace {
        workspace: WorkspacePath,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkspaceGitOperation {
    WorktreeRemove {
        #[serde(rename = "gitDir")]
        git_dir: RepositoryPath,
        #[serde(rename = "worktreePath")]
        worktree_path: RepositoryPath,
    },
    WorktreePrune {
        #[serde(rename = "gitDir")]
        git_dir: RepositoryPath,
    },
}

impl WorkspaceTeardownStep {
    pub fn repository_name(&self) -> Option<&WorkspaceRepositoryName> {
        match &self.subject {
            WorkspaceTeardownSubject::Workspace => None,
            WorkspaceTeardownSubject::Repository { repository } => Some(repository),
        }
    }

    pub fn target_path(&self) -> &str {
        match &self.action {
            WorkspaceTeardownAction::WorktreeRemove { worktree_path, .. } => worktree_path.as_str(),
            WorkspaceTeardownAction::WorktreePrune { git_dir } => git_dir.as_str(),
            WorkspaceTeardownAction::DeleteWorkspace { workspace } => workspace.as_str(),
        }
    }
}

impl fmt::Display for WorkspaceTeardownSubject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace => formatter.write_str("workspace"),
            Self::Repository { repository } => repository.fmt(formatter),
        }
    }
}

impl fmt::Display for WorkspaceTeardownAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorktreeRemove { .. } => formatter.write_str("worktree remove"),
            Self::WorktreePrune { .. } => formatter.write_str("worktree prune"),
            Self::DeleteWorkspace { .. } => formatter.write_str("delete directory"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskStartRequest<'a> {
    pub root: &'a str,
    pub projects: &'a ProjectsConfig,
    pub work_item_ids: &'a [WorkItemId],
    pub project: Option<&'a ProjectKey>,
    pub task_id: Option<&'a str>,
    pub type_name: Option<&'a str>,
    pub repositories: &'a [WorkspaceRepositoryName],
    pub slug: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceHandoffSummary {
    pub repository: WorkspaceRepositoryName,
    pub status: WorkspaceHandoffStatus,
    pub done: Vec<HandoffSummaryEntry>,
    pub decisions: Vec<HandoffSummaryEntry>,
    pub risks: Vec<HandoffSummaryEntry>,
    pub blockers: Vec<HandoffSummaryEntry>,
    pub follow_up: Vec<HandoffSummaryEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceHandoffStatus {
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl WorkspaceHandoffStatus {
    pub const ALL: [Self; 4] = [Self::Todo, Self::InProgress, Self::Done, Self::Blocked];

    pub fn parse(value: &str) -> Result<Self, HandoffParseError> {
        let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
        match normalized.as_str() {
            "todo" => Ok(Self::Todo),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            "blocked" => Ok(Self::Blocked),
            _ => Err(HandoffParseError::from(format!(
                "status handoff invalide: {}. Attendus: {}.",
                value,
                Self::allowed_values()
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }

    pub fn validation_status(self) -> TaskHandoffValidationStatus {
        match self {
            Self::Done => TaskHandoffValidationStatus::Valid,
            Self::Blocked => TaskHandoffValidationStatus::Blocked,
            Self::InProgress => TaskHandoffValidationStatus::InProgress,
            Self::Todo => TaskHandoffValidationStatus::Todo,
        }
    }

    pub fn is_finish_ready(self) -> bool {
        self == Self::Done
    }

    pub fn allowed_values() -> String {
        Self::ALL
            .iter()
            .map(|status| status.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for WorkspaceHandoffStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct HandoffSummaryEntry(String);

impl HandoffSummaryEntry {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for HandoffSummaryEntry {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for HandoffSummaryEntry {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for HandoffSummaryEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
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
    TeardownFailed {
        repository: WorkspaceTeardownSubject,
        message: WorkspaceOperationError,
    },
    #[error("Préparation worktree échouée [{repository}]: {message}")]
    WorktreePrepareFailed {
        repository: WorkspaceRepositoryName,
        message: WorkspaceOperationError,
    },
    #[error("Impossible de retirer tous les work items du workspace.")]
    EmptyWorkItemSet,
}

pub fn build_handoff_validation_report(
    workspace: &WorkspacePath,
) -> Result<TaskHandoffValidationReport, WorkspaceError> {
    let workspace_path = Path::new(workspace.as_str());
    if !workspace_path.exists() {
        return Err(WorkspaceError::MissingWorkspace(workspace.to_string()));
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
                repository: repository.clone(),
                path: HandoffFilePath::from(path.display().to_string()),
                status: TaskHandoffValidationStatus::Missing,
                valid: false,
                detail: TaskHandoffValidationDetail::MissingFile,
                done_count: 0,
                decision_count: 0,
                risk_count: 0,
                blocker_count: 0,
                follow_up_count: 0,
            });
            continue;
        }

        let text = fs::read_to_string(&path).unwrap_or_default();
        match try_parse_summary(&text, repository.as_str()) {
            Ok(summary) => {
                let valid = summary.status.is_finish_ready();
                let status = summary.status.validation_status();
                items.push(TaskHandoffValidationItem {
                    repository: repository.clone(),
                    path: HandoffFilePath::from(path.display().to_string()),
                    status,
                    valid,
                    detail: if valid {
                        TaskHandoffValidationDetail::Valid
                    } else {
                        TaskHandoffValidationDetail::NotFinishReady
                    },
                    done_count: summary.done.len(),
                    decision_count: summary.decisions.len(),
                    risk_count: summary.risks.len(),
                    blocker_count: summary.blockers.len(),
                    follow_up_count: summary.follow_up.len(),
                });
            }
            Err(error) => items.push(TaskHandoffValidationItem {
                repository: repository.clone(),
                path: HandoffFilePath::from(path.display().to_string()),
                status: TaskHandoffValidationStatus::Invalid,
                valid: false,
                detail: TaskHandoffValidationDetail::InvalidFile { reason: error },
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
        workspace: workspace.clone(),
        project: manifest.project,
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
            project.is_none_or(|project| {
                workspace
                    .manifest
                    .project
                    .as_str()
                    .eq_ignore_ascii_case(project)
            })
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

pub fn task_list(root: &str, project: Option<&str>, work_item: Option<&str>) -> Vec<TaskListItem> {
    filter_workspaces(find_workspaces(root), project, work_item)
        .into_iter()
        .map(|workspace| TaskListItem {
            path: workspace.path,
            project: workspace.manifest.project.clone(),
            work_item_id: workspace.manifest.primary_work_item_id(),
            work_items: workspace.manifest.parent_work_items(),
            task_id: workspace.manifest.task_id.clone(),
            all_known_work_item_ids: workspace
                .manifest
                .all_known_work_item_ids()
                .into_iter()
                .collect(),
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
    let requested_work_items = parse_work_item_selection(work_item);
    plan_task_prune_by_requested_ids(root, project, requested_work_items.as_deref())
}

pub fn plan_task_prune_by_work_item_ids(
    root: &str,
    project: Option<&str>,
    work_item_ids: &[WorkItemId],
) -> Vec<WorkspaceSummary> {
    plan_task_prune_by_requested_ids(root, project, Some(work_item_ids))
}

fn plan_task_prune_by_requested_ids(
    root: &str,
    project: Option<&str>,
    requested_work_items: Option<&[WorkItemId]>,
) -> Vec<WorkspaceSummary> {
    filter_workspaces_by_requested_ids(find_workspaces(root), project, requested_work_items)
        .into_iter()
        .filter(|workspace| {
            workspace.manifest.parent_work_items().iter().all(|item| {
                is_final_state(
                    item.kind.as_ref().map(WorkItemTypeName::as_str),
                    item.state.as_ref().map(WorkItemState::as_str),
                )
            })
        })
        .collect()
}

pub fn is_final_state(work_item_type: Option<&str>, state: Option<&str>) -> bool {
    dw_ado::is_final_state(work_item_type, state)
}

pub fn task_current(start_path: &str) -> Result<TaskCurrentItem, WorkspaceError> {
    let workspace = find_workspace_path(start_path).ok_or(WorkspaceError::NoCurrentWorkspace)?;
    let manifest = read_manifest(&Path::new(&workspace).join("task.json"))?;
    Ok(TaskCurrentItem {
        workspace: WorkspacePath::from(workspace),
        project: manifest.project.clone(),
        primary_work_item_id: manifest.primary_work_item_id(),
        work_items: manifest.parent_work_items(),
        task_id: manifest.task_id.clone(),
        child_task_ids: manifest.child_task_ids_by_repository(),
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
    workspace: &WorkspacePath,
    slug: &str,
) -> Result<(WorkspaceManifest, TaskRenamePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let _project_config = resolve_project(projects, manifest.project.as_str());
    let new_slug = slug_from_phrase_or_fallback(Some(slug), manifest.slug.as_str());
    let new_branch = build_branch_name(
        &manifest.kind,
        &manifest.all_known_work_item_ids(),
        &new_slug,
    );
    let new_subject = build_subject_name(
        &manifest.kind,
        &manifest
            .parent_work_items()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        &new_slug,
    );
    let new_workspace = Path::new(workspace.as_str())
        .parent()
        .unwrap_or_else(|| Path::new(root))
        .join(new_subject.as_str())
        .display()
        .to_string();

    Ok((
        manifest.clone(),
        TaskRenamePlan {
            workspace: workspace.clone(),
            new_workspace: WorkspacePath::from(new_workspace),
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
        &Path::new(plan.workspace.as_str()).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.to_string()))?,
    )?;

    if !plan
        .workspace
        .as_str()
        .eq_ignore_ascii_case(plan.new_workspace.as_str())
    {
        fs::rename(plan.workspace.as_str(), plan.new_workspace.as_str())
            .map_err(|_| WorkspaceError::MissingWorkspace(plan.new_workspace.to_string()))?;
    }

    Ok(updated)
}

pub fn resolve_workspace_for_workspace_command(
    root: &str,
    workspace: Option<&str>,
    use_latest_workspace: bool,
    current_directory: &str,
) -> Result<WorkspacePath, WorkspaceError> {
    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        return Ok(WorkspacePath::from(
            PathBuf::from(workspace).display().to_string(),
        ));
    }

    if use_latest_workspace {
        return resolve_workspace(root, None, None, None, None, true);
    }

    find_workspace_path(current_directory)
        .map(WorkspacePath::from)
        .ok_or(WorkspaceError::NoCurrentWorkspace)
}

pub fn plan_task_repo_latest(
    root: &str,
    projects: &ProjectsConfig,
    workspace: &WorkspacePath,
    requested_repositories: &[WorkspaceRepositoryName],
) -> Result<(WorkspaceManifest, Vec<TaskRepoLatestTarget>), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let project_config = resolve_project(projects, manifest.project.as_str());
    let repositories = resolve_workspace_repositories(&manifest, requested_repositories)?;
    let targets = repositories
        .into_iter()
        .map(|repository| {
            let repository_config = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository.as_str()))
                .unwrap_or(RepositoryConfig {
                    url: String::new(),
                    default_branch: "main".into(),
                    pull_request_target_branch: None,
                    azure_dev_ops_repository: None,
                    anchor_name: None,
                    git_credential_secret: None,
                    folder: Some(repository.to_string()),
                });
            let folder = repository_config
                .folder
                .clone()
                .unwrap_or_else(|| repository.to_string());
            TaskRepoLatestTarget {
                repository,
                repository_path: RepositoryPath::from(
                    Path::new(workspace.as_str())
                        .join(folder)
                        .display()
                        .to_string(),
                ),
                default_branch: BranchName::from(repository_config.default_branch),
                git_credential_secret: repository_config.git_credential_secret,
            }
        })
        .collect::<Vec<_>>();
    let _ = root;
    Ok((manifest, targets))
}

pub fn plan_task_commit(
    projects: &ProjectsConfig,
    workspace: &WorkspacePath,
) -> Result<(WorkspaceManifest, Vec<TaskCommitTarget>), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let project_config = resolve_project(projects, manifest.project.as_str());
    let targets = manifest
        .repositories
        .iter()
        .map(|repository| {
            let repository_config = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository.as_str()));
            let folder = repository_config
                .and_then(|repository| repository.folder)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| repository.to_string());
            TaskCommitTarget {
                repository: repository.clone(),
                path: RepositoryPath::from(
                    Path::new(workspace.as_str())
                        .join(folder)
                        .display()
                        .to_string(),
                ),
            }
        })
        .collect();
    Ok((manifest, targets))
}

pub fn plan_task_finish(
    projects: &ProjectsConfig,
    workspace: &WorkspacePath,
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
    override_message: Option<&CommitMessage>,
) -> CommitMessage {
    if let Some(message) = override_message.filter(|message| !message.as_str().trim().is_empty()) {
        return ensure_work_item_reference(message.as_str(), manifest);
    }

    CommitMessage::from(format!(
        "{}({}): {}",
        commit_prefix(manifest.kind.as_str()),
        commit_ids(manifest).join(" "),
        manifest.slug
    ))
}

pub fn ensure_work_item_reference(message: &str, manifest: &WorkspaceManifest) -> CommitMessage {
    let ids = commit_ids_for_reference(manifest);
    if ids.iter().any(|id| message.contains(&format!("#{id}"))) {
        CommitMessage::from(message)
    } else if let Some(id) = ids.first() {
        CommitMessage::from(format!("{message} #{id}"))
    } else {
        CommitMessage::from(message)
    }
}

pub fn plan_add_work_items(
    root: &str,
    workspace: &WorkspacePath,
    ids: &[WorkItemId],
    kind: Option<&str>,
    title: Option<&str>,
    state: Option<&str>,
) -> Result<(WorkspaceManifest, TaskWorkItemUpdatePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let mut work_items = manifest.parent_work_items();
    let mut added_ids = Vec::new();
    for id in ids {
        if manifest.matches_work_item(id.as_str())
            || work_items
                .iter()
                .any(|item| item.id.as_str().eq_ignore_ascii_case(id.as_str()))
        {
            continue;
        }
        added_ids.push(id.clone());
        work_items.push(WorkspaceWorkItem {
            id: id.clone(),
            kind: kind.map(WorkItemTypeName::from),
            title: title.map(WorkItemTitle::from),
            state: state.map(WorkItemState::from),
        });
    }
    reject_work_item_conflicts(
        root,
        workspace.as_str(),
        manifest.project.as_str(),
        &added_ids,
    )?;
    build_work_item_update_plan(root, workspace.as_str(), &manifest, work_items)
        .map(|plan| (manifest, plan))
}

pub fn plan_add_work_item_snapshots(
    root: &str,
    workspace: &WorkspacePath,
    snapshots: &[WorkItemSnapshot],
) -> Result<(WorkspaceManifest, TaskWorkItemUpdatePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let mut work_items = manifest.parent_work_items();
    let mut added_ids = Vec::new();
    for snapshot in snapshots {
        if manifest.matches_work_item(snapshot.id.as_str())
            || work_items
                .iter()
                .any(|item| item.id.as_str().eq_ignore_ascii_case(snapshot.id.as_str()))
        {
            continue;
        }
        added_ids.push(snapshot.id.clone());
        work_items.push(WorkspaceWorkItem {
            id: snapshot.id.clone(),
            kind: snapshot.kind.clone().map(WorkItemTypeName::from),
            title: snapshot.title.clone().map(WorkItemTitle::from),
            state: snapshot.state.clone().map(WorkItemState::from),
        });
    }
    reject_work_item_conflicts(
        root,
        workspace.as_str(),
        manifest.project.as_str(),
        &added_ids,
    )?;
    build_work_item_update_plan(root, workspace.as_str(), &manifest, work_items)
        .map(|plan| (manifest, plan))
}

pub fn plan_remove_work_items(
    root: &str,
    workspace: &WorkspacePath,
    ids: &[WorkItemId],
) -> Result<(WorkspaceManifest, TaskWorkItemUpdatePlan), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let work_items = manifest
        .parent_work_items()
        .into_iter()
        .filter(|item| {
            !ids.iter()
                .any(|id| id.as_str().eq_ignore_ascii_case(item.id.as_str()))
        })
        .collect::<Vec<_>>();
    if work_items.is_empty() {
        return Err(WorkspaceError::EmptyWorkItemSet);
    }
    build_work_item_update_plan(root, workspace.as_str(), &manifest, work_items)
        .map(|plan| (manifest, plan))
}

pub fn execute_work_item_update(
    manifest: &WorkspaceManifest,
    plan: &TaskWorkItemUpdatePlan,
) -> Result<(WorkspaceManifest, WorkspacePath), WorkspaceError> {
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

    let workspace_path = Path::new(plan.workspace.as_str());
    write_text(
        &workspace_path.join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.to_string()))?,
    )?;
    write_text(&workspace_path.join("plan.md"), &plan_markdown(&updated))?;
    for repository in &updated.repositories {
        write_text(
            &workspace_path.join(format!("handoff-{repository}.md")),
            &handoff_markdown(&updated, repository.as_str()),
        )?;
    }

    if plan.workspace != plan.new_workspace {
        if Path::new(plan.new_workspace.as_str()).exists() {
            return Err(WorkspaceError::WorkspaceConflict(
                plan.new_workspace.to_string(),
            ));
        }
        fs::rename(plan.workspace.as_str(), plan.new_workspace.as_str())
            .map_err(|_| WorkspaceError::MissingWorkspace(plan.new_workspace.to_string()))?;
    }

    Ok((updated, plan.new_workspace.clone()))
}

pub fn execute_task_sync(
    workspace: &WorkspacePath,
    snapshots: &[WorkItemSnapshot],
) -> Result<WorkspaceManifest, WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    if snapshots.is_empty() {
        return Ok(manifest);
    }
    let work_items = snapshots
        .iter()
        .map(|snapshot| WorkspaceWorkItem {
            id: snapshot.id.clone(),
            kind: snapshot.kind.clone().map(WorkItemTypeName::from),
            title: snapshot.title.clone().map(WorkItemTitle::from),
            state: snapshot.state.clone().map(WorkItemState::from),
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
        &Path::new(workspace.as_str()).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(workspace.to_string()))?,
    )?;
    Ok(updated)
}

pub fn execute_add_child_task(
    workspace: &WorkspacePath,
    repository: &str,
    id: &WorkItemId,
    title: Option<String>,
) -> Result<WorkspaceManifest, WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let mut child_tasks = manifest.normalized_child_tasks();
    child_tasks.push(WorkspaceChildTask {
        repository: WorkspaceRepositoryName::from(repository),
        id: id.clone(),
        title: title.map(WorkItemTitle::from),
    });
    let updated = WorkspaceManifest {
        child_tasks: Some(child_tasks),
        ..manifest
    };
    write_text(
        &Path::new(workspace.as_str()).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(workspace.to_string()))?,
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
    workspace: &WorkspacePath,
    repository_key: &str,
) -> Result<(WorkspaceManifest, TaskAddRepoPlan), WorkspaceError> {
    let repository_key = repository_key.trim();
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    if manifest
        .repositories
        .iter()
        .any(|repository| repository.as_str().eq_ignore_ascii_case(repository_key))
    {
        let repository = repository_key.to_string();
        return Ok((
            manifest.clone(),
            TaskAddRepoPlan {
                workspace: workspace.clone(),
                repository: WorkspaceRepositoryName::from(repository.clone()),
                project_root: ProjectRootPath::from(
                    Path::new(root)
                        .join("projects")
                        .join(manifest.project.as_str())
                        .display()
                        .to_string(),
                ),
                worktree_path: RepositoryPath::from(
                    Path::new(workspace.as_str())
                        .join(&repository)
                        .display()
                        .to_string(),
                ),
                url: GitRemoteUrl::from(""),
                default_branch: BranchName::from("main"),
                anchor_name: GitAnchorName::from(format!("{repository}.git")),
                git_credential_secret: None,
                branch_name: manifest.branch_name.clone(),
                repositories: manifest.repositories.clone(),
            },
        ));
    }

    let project_config = resolve_project(projects, manifest.project.as_str())
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
    repositories.push(WorkspaceRepositoryName::from(repository_key));
    repositories = distinct_repositories(&repositories);

    Ok((
        manifest.clone(),
        TaskAddRepoPlan {
            workspace: workspace.clone(),
            repository: WorkspaceRepositoryName::from(repository_key),
            project_root: ProjectRootPath::from(
                Path::new(root)
                    .join("projects")
                    .join(manifest.project.as_str())
                    .display()
                    .to_string(),
            ),
            worktree_path: RepositoryPath::from(
                Path::new(workspace.as_str())
                    .join(folder)
                    .display()
                    .to_string(),
            ),
            url: GitRemoteUrl::from(repository_config.url),
            default_branch: BranchName::from(repository_config.default_branch),
            anchor_name: GitAnchorName::from(anchor_name),
            git_credential_secret: repository_config.git_credential_secret,
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
        &Path::new(plan.workspace.as_str()).join("task.json"),
        &serde_json::to_string_pretty(&updated)
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.to_string()))?,
    )?;
    write_text(
        &Path::new(plan.workspace.as_str()).join(format!("handoff-{}.md", plan.repository)),
        &handoff_markdown(&updated, plan.repository.as_str()),
    )?;
    Ok(updated)
}

pub fn plan_task_teardown(
    root: &str,
    projects: &ProjectsConfig,
    workspace: &WorkspacePath,
) -> Result<(WorkspaceManifest, Vec<WorkspaceTeardownStep>), WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let project_config = resolve_project(projects, manifest.project.as_str());
    let project_root = Path::new(root)
        .join("projects")
        .join(manifest.project.as_str());
    let mut steps = Vec::new();

    for repository_key in distinct_repositories(&manifest.repositories) {
        let repository = project_config
            .as_ref()
            .and_then(|project| repository_config(project, repository_key.as_str()))
            .unwrap_or(RepositoryConfig {
                url: String::new(),
                default_branch: "main".into(),
                pull_request_target_branch: None,
                azure_dev_ops_repository: None,
                anchor_name: None,
                git_credential_secret: None,
                folder: Some(repository_key.to_string()),
            });
        let folder = repository
            .folder
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| repository_key.to_string());
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
        let git_dir = RepositoryPath::from(git_dir);
        steps.push(WorkspaceTeardownStep {
            subject: WorkspaceTeardownSubject::Repository {
                repository: repository_key.clone(),
            },
            action: WorkspaceTeardownAction::WorktreeRemove {
                worktree_path: RepositoryPath::from(
                    Path::new(workspace.as_str())
                        .join(folder)
                        .display()
                        .to_string(),
                ),
                git_dir: git_dir.clone(),
            },
        });

        if !repository.url.trim().is_empty() {
            steps.push(WorkspaceTeardownStep {
                subject: WorkspaceTeardownSubject::Repository {
                    repository: repository_key.clone(),
                },
                action: WorkspaceTeardownAction::WorktreePrune { git_dir },
            });
        }
    }

    steps.push(WorkspaceTeardownStep {
        subject: WorkspaceTeardownSubject::Workspace,
        action: WorkspaceTeardownAction::DeleteWorkspace {
            workspace: workspace.clone(),
        },
    });

    Ok((manifest, steps))
}

pub fn execute_task_teardown<F>(
    workspace: &WorkspacePath,
    steps: &[WorkspaceTeardownStep],
    mut run_git_operation: F,
) -> Result<(), WorkspaceError>
where
    F: FnMut(WorkspaceGitOperation) -> Result<(), WorkspaceOperationError>,
{
    for step in steps
        .iter()
        .filter(|step| matches!(step.action, WorkspaceTeardownAction::WorktreeRemove { .. }))
    {
        let git_dir = teardown_git_dir(step)?;
        if !Path::new(git_dir).exists() {
            return Err(WorkspaceError::TeardownFailed {
                repository: step.subject.clone(),
                message: WorkspaceOperationError::from(format!("gitDir introuvable {git_dir}")),
            });
        }
        let WorkspaceTeardownAction::WorktreeRemove { worktree_path, .. } = &step.action else {
            unreachable!("filtered to worktree remove");
        };
        run_git_operation(WorkspaceGitOperation::WorktreeRemove {
            git_dir: RepositoryPath::from(git_dir),
            worktree_path: worktree_path.clone(),
        })
        .map_err(|message| WorkspaceError::TeardownFailed {
            repository: step.subject.clone(),
            message,
        })?;
    }

    for step in steps
        .iter()
        .filter(|step| matches!(step.action, WorkspaceTeardownAction::WorktreePrune { .. }))
    {
        let git_dir = teardown_git_dir(step)?;
        if !Path::new(git_dir).exists() {
            continue;
        }
        run_git_operation(WorkspaceGitOperation::WorktreePrune {
            git_dir: RepositoryPath::from(git_dir),
        })
        .map_err(|message| WorkspaceError::TeardownFailed {
            repository: step.subject.clone(),
            message,
        })?;
    }

    if Path::new(workspace.as_str()).exists() {
        fs::remove_dir_all(workspace.as_str()).map_err(|error| WorkspaceError::TeardownFailed {
            repository: WorkspaceTeardownSubject::Workspace,
            message: WorkspaceOperationError::from(error.to_string()),
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
                kind: work_item_type.map(WorkItemTypeName::from),
                title: work_item_title.map(WorkItemTitle::from),
                state: work_item_state.map(WorkItemState::from),
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
    let workspace = Path::new(plan.workspace.as_str());
    fs::create_dir_all(workspace)
        .map_err(|_| WorkspaceError::MissingWorkspace(plan.workspace.to_string()))?;
    if plan.repository_worktrees.is_empty() {
        for folder in plan.repository_folders.values() {
            fs::create_dir_all(workspace.join(folder.as_str())).map_err(|_| {
                WorkspaceError::MissingWorkspace(
                    workspace.join(folder.as_str()).display().to_string(),
                )
            })?;
        }
    } else {
        for target in &plan.repository_worktrees {
            let credential =
                resolve_git_credential_from_keyring(target.git_credential_secret.as_ref())
                    .map_err(|message| WorkspaceError::WorktreePrepareFailed {
                        repository: target.repository.clone(),
                        message,
                    })?;
            prepare_worktree(&WorktreePrepareRequest {
                project_root: target.project_root.clone(),
                repository: target.repository.clone(),
                url: target.url.clone(),
                default_branch: target.default_branch.clone(),
                anchor_name: target.anchor_name.clone(),
                branch_name: target.branch_name.clone(),
                worktree_path: target.worktree_path.clone(),
                credential,
            })
            .map_err(|error| WorkspaceError::WorktreePrepareFailed {
                repository: target.repository.clone(),
                message: WorkspaceOperationError::from(error.to_string()),
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
        .ok_or_else(|| WorkspaceError::InvalidManifest(plan.workspace.to_string()))?;

    let manifest = WorkspaceManifest {
        schema: 1,
        work_item_id: plan.primary_work_item_id.clone(),
        task_id: plan.task_id.clone(),
        project: plan.project.clone(),
        kind: plan.kind.clone(),
        slug: plan.slug.clone(),
        branch_name: plan.branch_name.clone(),
        created_at: current_timestamp(),
        repositories: plan.repositories.clone(),
        status: WorkspaceManifestStatus::Created,
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
            .map_err(|_| WorkspaceError::InvalidManifest(plan.workspace.to_string()))?,
    )?;
    write_text(&workspace.join("plan.md"), &plan_markdown(&manifest))?;
    for repository in &manifest.repositories {
        write_text(
            &workspace.join(format!("handoff-{repository}.md")),
            &handoff_markdown(&manifest, repository.as_str()),
        )?;
    }

    Ok(manifest)
}

pub fn start_plan_with_child_tasks(
    mut plan: TaskStartPlan,
    child_tasks: &[WorkspaceChildTask],
) -> TaskStartPlan {
    if plan
        .task_id
        .as_ref()
        .is_none_or(|id| id.as_str().trim().is_empty())
        && child_tasks.len() == 1
        && !child_tasks[0].id.as_str().trim().is_empty()
    {
        plan.task_id = Some(TaskId::from(child_tasks[0].id.as_str()));
    }

    let mut branch_work_item_ids = plan.work_item_ids.clone();
    if let Some(task_id) = plan
        .task_id
        .as_ref()
        .filter(|id| !id.as_str().trim().is_empty())
        && !branch_work_item_ids
            .iter()
            .any(|id| id.as_str().eq_ignore_ascii_case(task_id.as_str()))
    {
        branch_work_item_ids.push(WorkItemId::from(task_id.as_str()));
    }
    for child_task in child_tasks {
        if !child_task.id.as_str().trim().is_empty()
            && !branch_work_item_ids
                .iter()
                .any(|id| id.as_str().eq_ignore_ascii_case(child_task.id.as_str()))
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
    workspace: &WorkspacePath,
    ai_context_files: &[AiContextFilePath],
) -> Result<TaskPreflightReport, WorkspaceError> {
    let manifest = read_manifest(&Path::new(workspace.as_str()).join("task.json"))?;
    let mut issues = Vec::new();

    for file in ai_context_files {
        let text = fs::read_to_string(file.as_str())
            .map_err(|_| WorkspaceError::MissingAiContext(file.to_string()))?;
        let ai_context: AdoAiContextItem = serde_json::from_str(&text)
            .map_err(|_| WorkspaceError::MissingAiContext(file.to_string()))?;
        issues.extend(build_stale_context_issues(&ai_context, &manifest));
        issues.extend(build_attachment_issues(&ai_context));
    }

    let has_blocking_issues = issues.iter().any(|issue| issue.severity.is_blocking());
    Ok(TaskPreflightReport {
        schema_version: PREFLIGHT_VERSION.into(),
        workspace: workspace.clone(),
        project: manifest.project.clone(),
        work_item_ids: manifest
            .parent_work_items()
            .into_iter()
            .map(|item| item.id)
            .collect(),
        issues,
        has_blocking_issues,
    })
}

pub fn plan_task_start(request: TaskStartRequest<'_>) -> Result<TaskStartPlan, WorkspaceError> {
    let project = request
        .project
        .cloned()
        .unwrap_or_else(|| ProjectKey::from("default"));
    let work_item_ids = request.work_item_ids.to_vec();
    let primary_work_item_id = work_item_ids
        .first()
        .cloned()
        .unwrap_or_else(|| WorkItemId::from(""));
    let project_config = resolve_project(request.projects, project.as_str());
    let repositories = resolve_repositories(project_config.as_ref(), request.repositories);
    let repository_folders = repositories
        .iter()
        .map(|repository| {
            let folder = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository.as_str()))
                .and_then(|repository| repository.folder)
                .unwrap_or_else(|| repository.to_string());
            (repository.clone(), RepositoryPath::from(folder))
        })
        .collect::<BTreeMap<_, _>>();
    reject_workspace_conflicts(request.root, project.as_str(), &work_item_ids)?;

    let kind = WorkItemTypeName::from(
        request
            .type_name
            .unwrap_or("feat")
            .trim()
            .to_ascii_lowercase(),
    );
    let slug =
        slug_from_phrase_or_fallback(request.slug, &format!("work item {primary_work_item_id}"));
    let mut branch_work_item_ids = work_item_ids.clone();
    if let Some(task_id) = request.task_id.filter(|value| !value.trim().is_empty())
        && !branch_work_item_ids
            .iter()
            .any(|id| id.as_str().eq_ignore_ascii_case(task_id))
    {
        branch_work_item_ids.push(WorkItemId::from(task_id));
    }
    let subject_name = build_subject_name(&kind, &work_item_ids, &slug);
    let branch_name = build_branch_name(&kind, &branch_work_item_ids, &slug);
    let workspace = Path::new(request.root)
        .join("projects")
        .join(project.as_str())
        .join("workspaces")
        .join(subject_name.as_str())
        .display()
        .to_string();
    let project_root = Path::new(request.root)
        .join("projects")
        .join(project.as_str())
        .display()
        .to_string();
    let repository_worktrees = repositories
        .iter()
        .map(|repository_key| {
            let repository = project_config
                .as_ref()
                .and_then(|project| repository_config(project, repository_key.as_str()))
                .unwrap_or(RepositoryConfig {
                    url: String::new(),
                    default_branch: "main".into(),
                    pull_request_target_branch: None,
                    azure_dev_ops_repository: None,
                    anchor_name: None,
                    git_credential_secret: None,
                    folder: Some(repository_key.to_string()),
                });
            let folder = repository_folders
                .get(repository_key)
                .cloned()
                .unwrap_or_else(|| RepositoryPath::from(repository_key.as_str()));
            let anchor_name = repository
                .anchor_name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("{repository_key}.git"));
            TaskStartRepositoryPlan {
                repository: repository_key.clone(),
                project_root: ProjectRootPath::from(project_root.clone()),
                worktree_path: RepositoryPath::from(
                    Path::new(&workspace)
                        .join(folder.as_str())
                        .display()
                        .to_string(),
                ),
                url: GitRemoteUrl::from(repository.url),
                default_branch: BranchName::from(repository.default_branch),
                anchor_name: GitAnchorName::from(anchor_name),
                git_credential_secret: repository.git_credential_secret,
                branch_name: branch_name.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok(TaskStartPlan {
        work_item_ids,
        primary_work_item_id,
        project,
        task_id: request.task_id.map(TaskId::from),
        kind,
        slug,
        branch_name,
        subject_name,
        workspace: WorkspacePath::from(workspace),
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
) -> Result<WorkspacePath, WorkspaceError> {
    let work_item_ids = resolve_work_item_ids(work_item, positional_work_item)?;

    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        return Ok(WorkspacePath::from(
            PathBuf::from(workspace).display().to_string(),
        ));
    }

    let workspaces = filter_workspaces_by_requested_ids(
        find_workspaces(root),
        project,
        work_item_ids.as_deref(),
    );
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
) -> Result<WorkspacePath, WorkspaceError> {
    if let Some(workspace) = workspace.filter(|value| !value.trim().is_empty()) {
        return Ok(WorkspacePath::from(
            PathBuf::from(workspace).display().to_string(),
        ));
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
    workspace: &WorkspacePath,
    manifest: &WorkspaceManifest,
    project_config: Option<&ProjectConfig>,
    repository_key: Option<&str>,
) -> Result<String, WorkspaceError> {
    let Some(repository_key) = repository_key.filter(|value| !value.trim().is_empty()) else {
        return Ok(workspace.to_string());
    };

    if !manifest
        .repositories
        .iter()
        .any(|repo| repo.as_str().eq_ignore_ascii_case(repository_key))
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
            git_credential_secret: None,
            folder: Some(repository_key.into()),
        });
    let folder = repository.folder.unwrap_or_else(|| repository_key.into());
    Ok(Path::new(workspace.as_str())
        .join(folder)
        .display()
        .to_string())
}

impl WorkspaceManifest {
    pub fn parent_work_items(&self) -> Vec<WorkspaceWorkItem> {
        let mut normalized = self
            .work_items
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|item| !item.id.as_str().trim().is_empty())
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

        if !normalized.iter().any(|item| {
            item.id
                .as_str()
                .eq_ignore_ascii_case(self.work_item_id.as_str())
        }) {
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

        normalized.sort_by_key(|item| {
            !item
                .id
                .as_str()
                .eq_ignore_ascii_case(self.work_item_id.as_str())
        });
        normalized
    }

    pub fn primary_work_item_id(&self) -> WorkItemId {
        self.parent_work_items()[0].id.clone()
    }

    pub fn display_work_item_ids(&self) -> String {
        self.parent_work_items()
            .into_iter()
            .map(|item| item.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn normalized_child_tasks(&self) -> Vec<WorkspaceChildTask> {
        let mut normalized = self
            .child_tasks
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|task| {
                !task.id.as_str().trim().is_empty() && !task.repository.as_str().trim().is_empty()
            })
            .collect::<Vec<_>>();

        if let Some(child_task_ids) = &self.child_task_ids {
            for (repository, id) in child_task_ids {
                if repository.as_str().trim().is_empty() || id.as_str().trim().is_empty() {
                    continue;
                }

                if normalized
                    .iter()
                    .any(|task| task.id.as_str().eq_ignore_ascii_case(id.as_str()))
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

    pub fn child_task_ids_by_repository(&self) -> BTreeMap<WorkspaceRepositoryName, WorkItemId> {
        let mut result = BTreeMap::new();
        for task in self.normalized_child_tasks() {
            result.entry(task.repository).or_insert(task.id);
        }
        result
    }

    pub fn all_known_work_item_ids(&self) -> Vec<WorkItemId> {
        let mut ids = self
            .parent_work_items()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();

        if let Some(task_id) = &self.task_id
            && !task_id.as_str().trim().is_empty()
        {
            ids.push(WorkItemId::from(task_id.as_str()));
        }

        for child_task in self.normalized_child_tasks() {
            if !ids
                .iter()
                .any(|id| id.as_str().eq_ignore_ascii_case(child_task.id.as_str()))
            {
                ids.push(child_task.id);
            }
        }

        ids
    }

    pub fn matches_work_item(&self, work_item_id: &str) -> bool {
        self.all_known_work_item_ids()
            .iter()
            .any(|id| id.as_str().eq_ignore_ascii_case(work_item_id))
    }
}

pub fn resolve_git_credential_from_keyring(
    secret_key: Option<&SecretKey>,
) -> Result<Option<GitCredential>, WorkspaceOperationError> {
    let Some(secret_key) = secret_key else {
        return Ok(None);
    };
    let store = KeyringSecretStore;
    let secret = store.get(secret_key).map_err(|error| {
        WorkspaceOperationError::from(format!("Secret Git illisible `{secret_key}`: {error}"))
    })?;
    let Some(secret) = secret else {
        return Err(WorkspaceOperationError::from(format!(
            "Secret Git introuvable `{secret_key}`. Stocker le PAT avec `dw secret set {secret_key}` ou retirer gitCredentialSecret."
        )));
    };
    Ok(Some(GitCredential::personal_access_token(secret)))
}

pub fn try_parse_summary(
    text: &str,
    expected_repository: &str,
) -> Result<WorkspaceHandoffSummary, HandoffParseError> {
    let normalized = text.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let start = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("```yaml"))
        .ok_or_else(|| HandoffParseError::from("bloc ```yaml absent"))?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim() == "```")
        .map(|(index, _)| index)
        .ok_or_else(|| HandoffParseError::from("fin du bloc yaml absente"))?;

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
                return Err(HandoffParseError::from(format!(
                    "section inconnue autour de '{trimmed}'"
                )));
            };
            let Some((key, value)) = split_key_value(trimmed) else {
                return Err(HandoffParseError::from(format!(
                    "cle inconnue dans {section}: '{trimmed}'"
                )));
            };
            let bucket = sections.get_mut(&section).ok_or_else(|| {
                HandoffParseError::from(format!("section inconnue autour de '{trimmed}'"))
            })?;
            let list = bucket.get_mut(&key).ok_or_else(|| {
                HandoffParseError::from(format!("cle inconnue dans {section}: '{trimmed}'"))
            })?;
            current_key = Some(key);
            if value != "[]" && !trim_scalar(&value).is_empty() {
                list.push(trim_scalar(&value));
            }
            continue;
        }

        if indent >= 4 && trimmed.starts_with("- ") {
            let Some(section) = current_section.clone() else {
                return Err(HandoffParseError::from(format!(
                    "element de liste hors section: '{trimmed}'"
                )));
            };
            let Some(key) = current_key.clone() else {
                return Err(HandoffParseError::from(format!(
                    "element de liste hors section: '{trimmed}'"
                )));
            };
            sections
                .get_mut(&section)
                .and_then(|bucket| bucket.get_mut(&key))
                .ok_or_else(|| {
                    HandoffParseError::from(format!("element de liste hors section: '{trimmed}'"))
                })?
                .push(trim_scalar(trimmed.trim_start_matches("- ")));
            continue;
        }

        return Err(HandoffParseError::from(format!(
            "ligne handoff non supportée: '{trimmed}'"
        )));
    }

    if status.trim().is_empty() {
        return Err(HandoffParseError::from("status absent"));
    }

    if repository.trim().is_empty() {
        return Err(HandoffParseError::from("repository absent"));
    }

    if !repository.eq_ignore_ascii_case(expected_repository) {
        return Err(HandoffParseError::from(format!(
            "repository attendu '{}', trouvé '{}'",
            expected_repository, repository
        )));
    }

    Ok(WorkspaceHandoffSummary {
        repository: WorkspaceRepositoryName::from(repository),
        status: WorkspaceHandoffStatus::parse(&status)?,
        done: handoff_entries(&sections["summary"]["done"]),
        decisions: handoff_entries(&sections["summary"]["decisions"]),
        risks: handoff_entries(&sections["summary"]["risks"]),
        blockers: handoff_entries(&sections["summary"]["blockers"]),
        follow_up: handoff_entries(&sections["summary"]["follow_up"]),
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

fn handoff_entries(values: &[String]) -> Vec<HandoffSummaryEntry> {
    values
        .iter()
        .cloned()
        .map(HandoffSummaryEntry::from)
        .collect()
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

fn distinct_repositories(repositories: &[WorkspaceRepositoryName]) -> Vec<WorkspaceRepositoryName> {
    let mut result = Vec::new();
    for repository in repositories {
        if !result.iter().any(|item: &WorkspaceRepositoryName| {
            item.as_str().eq_ignore_ascii_case(repository.as_str())
        }) {
            result.push(repository.clone());
        }
    }
    result
}

fn resolve_work_item_ids(
    work_item: Option<&str>,
    positional_work_item: Option<&str>,
) -> Result<Option<Vec<WorkItemId>>, WorkspaceError> {
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

fn normalize_work_item_selection(value: Option<&str>) -> Option<Vec<WorkItemId>> {
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
    Some(ids.into_iter().map(WorkItemId::from).collect())
}

pub fn parse_work_item_ids(value: &str) -> Vec<WorkItemId> {
    parse_work_item_selection(Some(value)).unwrap_or_default()
}

fn parse_work_item_selection(value: Option<&str>) -> Option<Vec<WorkItemId>> {
    normalize_work_item_selection(value)
}

fn resolve_repositories(
    project_config: Option<&ProjectConfig>,
    repositories: &[WorkspaceRepositoryName],
) -> Vec<WorkspaceRepositoryName> {
    if !repositories.is_empty() {
        return repositories.to_vec();
    }

    if let Some(project_config) = project_config
        && !project_config.repositories.is_empty()
    {
        return project_config
            .repositories
            .keys()
            .cloned()
            .map(WorkspaceRepositoryName::from)
            .collect();
    }

    vec![
        WorkspaceRepositoryName::from("front"),
        WorkspaceRepositoryName::from("back"),
    ]
}

fn reject_workspace_conflicts(
    root: &str,
    project: &str,
    work_item_ids: &[WorkItemId],
) -> Result<(), WorkspaceError> {
    let conflicts = find_workspaces(root)
        .into_iter()
        .filter(|workspace| {
            workspace
                .manifest
                .project
                .as_str()
                .eq_ignore_ascii_case(project)
        })
        .filter_map(|workspace| {
            let matching = work_item_ids
                .iter()
                .filter(|id| workspace.manifest.matches_work_item(id.as_str()))
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
        .map(|(path, ids)| {
            format!(
                "{} déjà présent(s) dans {}",
                ids.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                path
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(WorkspaceError::WorkspaceConflict(details))
}

fn resolve_workspace_repositories(
    manifest: &WorkspaceManifest,
    requested: &[WorkspaceRepositoryName],
) -> Result<Vec<WorkspaceRepositoryName>, WorkspaceError> {
    if !requested.is_empty() {
        let unknown = requested
            .iter()
            .filter(|repository| {
                !manifest
                    .repositories
                    .iter()
                    .any(|item| item.as_str().eq_ignore_ascii_case(repository.as_str()))
            })
            .map(WorkspaceRepositoryName::as_str)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(WorkspaceError::MissingWorkspaceRepository(
                unknown.join(", "),
            ));
        }
        return Ok(requested.to_vec());
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
            .chain(
                manifest
                    .task_id
                    .clone()
                    .map(|id| WorkItemId::from(id.as_str())),
            )
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
            .map(|id| WorkItemId::from(id.as_str()))
            .chain(std::iter::once(manifest.work_item_id.clone()))
            .chain(manifest.parent_work_items().into_iter().map(|item| item.id))
            .chain(
                manifest
                    .normalized_child_tasks()
                    .into_iter()
                    .map(|task| task.id),
            )
            .map(|id| id.to_string())
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
        && !task_id.as_str().trim().is_empty()
    {
        branch_ids.push(WorkItemId::from(task_id.as_str()));
    }
    branch_ids.extend(
        manifest
            .normalized_child_tasks()
            .into_iter()
            .map(|task| task.id),
    );
    let new_branch = build_branch_name(&manifest.kind, &branch_ids, &manifest.slug);
    let new_subject = build_subject_name(&manifest.kind, &parent_ids, &manifest.slug);
    let new_workspace = Path::new(workspace)
        .parent()
        .unwrap_or_else(|| Path::new(root))
        .join(new_subject.as_str())
        .display()
        .to_string();

    Ok(TaskWorkItemUpdatePlan {
        workspace: WorkspacePath::from(workspace),
        new_workspace: WorkspacePath::from(new_workspace),
        old_branch: manifest.branch_name.clone(),
        new_branch,
        work_items,
    })
}

fn reject_work_item_conflicts(
    root: &str,
    current_workspace: &str,
    project: &str,
    ids: &[WorkItemId],
) -> Result<(), WorkspaceError> {
    if ids.is_empty() {
        return Ok(());
    }

    let conflicts = find_workspaces(root)
        .into_iter()
        .filter(|workspace| {
            !workspace
                .path
                .as_str()
                .eq_ignore_ascii_case(current_workspace)
        })
        .filter(|workspace| {
            workspace
                .manifest
                .project
                .as_str()
                .eq_ignore_ascii_case(project)
        })
        .filter(|workspace| {
            ids.iter()
                .any(|id| workspace.manifest.matches_work_item(id.as_str()))
        })
        .map(|workspace| workspace.path.to_string())
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

fn teardown_git_dir(step: &WorkspaceTeardownStep) -> Result<&str, WorkspaceError> {
    let git_dir = match &step.action {
        WorkspaceTeardownAction::WorktreeRemove { git_dir, .. }
        | WorkspaceTeardownAction::WorktreePrune { git_dir } => git_dir.as_str(),
        WorkspaceTeardownAction::DeleteWorkspace { .. } => {
            return Err(WorkspaceError::TeardownFailed {
                repository: step.subject.clone(),
                message: WorkspaceOperationError::from(
                    "gitDir incompatible avec suppression workspace",
                ),
            });
        }
    };

    if git_dir.trim().is_empty() {
        return Err(WorkspaceError::TeardownFailed {
            repository: step.subject.clone(),
            message: WorkspaceOperationError::from("gitDir manquant"),
        });
    }

    Ok(git_dir)
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
                        path: WorkspacePath::from(path.display().to_string()),
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

fn current_timestamp() -> Timestamp {
    Timestamp::from(chrono::Utc::now().to_rfc3339())
}

fn build_stale_context_issues(
    ai_context: &AdoAiContextItem,
    manifest: &WorkspaceManifest,
) -> Vec<TaskPreflightIssue> {
    let Some(manifest_item) = manifest
        .parent_work_items()
        .into_iter()
        .find(|item| item.id.as_str() == ai_context.work_item.id.as_str())
    else {
        return vec![];
    };

    let mut stale_reasons = Vec::new();
    if manifest_item.title.as_ref().map(WorkItemTitle::as_str)
        != ai_context.work_item.title.as_deref()
    {
        stale_reasons.push(TaskPreflightStaleReason::Title);
    }
    if manifest_item.state.as_ref().map(WorkItemState::as_str)
        != ai_context.work_item.state.as_deref()
    {
        stale_reasons.push(TaskPreflightStaleReason::State);
    }
    if manifest_item.kind.as_ref().map(WorkItemTypeName::as_str)
        != ai_context.work_item.kind.as_deref()
    {
        stale_reasons.push(TaskPreflightStaleReason::Kind);
    }
    if stale_reasons.is_empty() {
        return vec![];
    }

    vec![TaskPreflightIssue {
        code: TaskPreflightIssueCode::WorkspaceAdoContextStale,
        severity: TaskPreflightSeverity::Warning,
        work_item_id: ai_context.work_item.id.clone(),
        detail: TaskPreflightIssueDetail::WorkspaceAdoContextStale {
            reasons: stale_reasons,
        },
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
        code: TaskPreflightIssueCode::AdoAttachmentsPresent,
        severity: TaskPreflightSeverity::Warning,
        work_item_id: ai_context.work_item.id.clone(),
        detail: TaskPreflightIssueDetail::AdoAttachmentsPresent {
            directory_hint: ai_context.attachments.directory_hint.clone(),
            names,
        },
        related_ids: vec![ai_context.work_item.id.clone()],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexAddOption, Repository, Signature};
    use std::fs;
    use tempfile::tempdir;

    fn typed_workspace_path(path: &Path) -> WorkspacePath {
        WorkspacePath::from(path.display().to_string())
    }

    fn init_develop_repository(path: &Path) {
        fs::create_dir_all(path).expect("source should exist");
        let repository = Repository::init(path).expect("repository should init");
        fs::write(path.join("README.md"), "front\n").expect("file should be written");

        let mut index = repository.index().expect("index should open");
        index
            .add_all(["README.md"].iter(), IndexAddOption::DEFAULT, None)
            .expect("readme should be added");
        index.write().expect("index should be written");
        let tree_id = index.write_tree().expect("tree should be written");
        let tree = repository.find_tree(tree_id).expect("tree should exist");
        let signature =
            Signature::now("dw test", "dw@example.invalid").expect("signature should be valid");
        let commit_id = repository
            .commit(
                Some("refs/heads/develop"),
                &signature,
                &signature,
                "init",
                &tree,
                &[],
            )
            .expect("initial commit should be created");
        repository
            .set_head("refs/heads/develop")
            .expect("develop should be checked out");
        repository
            .checkout_head(None)
            .expect("develop worktree should be checked out");
        repository
            .find_commit(commit_id)
            .expect("initial commit should be readable");
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
        assert_eq!(summary.status, WorkspaceHandoffStatus::Done);
        assert_eq!(summary.repository, WorkspaceRepositoryName::from("front"));
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
        assert!(error.to_string().contains("repository attendu 'front'"));
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
        assert!(error.to_string().contains("ligne handoff non supportée"));
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
        assert_eq!(workspaces[0].manifest.project, ProjectKey::from("ha"));

        let filtered = filter_workspaces(workspaces, Some("ha"), Some("123"));
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn task_list_returns_detected_workspace_paths() {
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

        let result = task_list(root.to_str().expect("utf8 path"), None, None);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].path,
            WorkspacePath::from(workspace.display().to_string())
        );
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
        assert_eq!(result[0].project.as_str(), "ha");
        assert_eq!(result[0].work_item_id.as_str(), "123");
        assert_eq!(
            result[0]
                .work_items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            vec![WorkItemId::from("123")]
        );
        assert_eq!(result[0].branch_name.as_str(), "feat/123-demo");
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
        assert_eq!(current.project.as_str(), "ha");
        assert_eq!(current.primary_work_item_id.as_str(), "123");
        assert_eq!(
            current
                .repositories
                .iter()
                .map(WorkspaceRepositoryName::as_str)
                .collect::<Vec<_>>(),
            vec!["front", "back"]
        );
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

        let report = build_handoff_validation_report(&typed_workspace_path(&workspace))
            .expect("report should be built");

        assert!(!report.is_valid);
        assert!(
            report
                .items
                .iter()
                .any(|item| item.status == TaskHandoffValidationStatus::Todo)
        );
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

        let report = build_handoff_validation_report(&typed_workspace_path(&workspace))
            .expect("report should be built");

        assert!(!report.is_valid);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.items[0].status, TaskHandoffValidationStatus::Missing);
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

        assert_eq!(workspace.as_str(), new_workspace.display().to_string());
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

        assert_eq!(resolved.as_str(), workspace.display().to_string());
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

        assert_eq!(resolved.as_str(), workspace.display().to_string());
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

        assert_eq!(resolved.as_str(), workspace.display().to_string());
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
        let workspace = WorkspacePath::from("/tmp/workspace");

        let target =
            resolve_open_target(&workspace, &manifest, Some(&project_config), Some("front"))
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
            repositories: &[],
            slug: None,
        })
        .expect("plan should build");

        assert_eq!(plan.project.as_str(), "default");
        assert_eq!(plan.kind.as_str(), "feat");
        assert_eq!(plan.slug.as_str(), "work-item-55222");
        assert_eq!(plan.branch_name.as_str(), "feat/55222-work-item-55222");
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
            project: Some(&ProjectKey::from("ha")),
            task_id: None,
            type_name: Some("feat"),
            repositories: &[],
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
            project: Some(&ProjectKey::from("ha")),
            task_id: None,
            type_name: Some("feat"),
            repositories: &[],
            slug: Some("demo"),
        })
        .expect("plan should build");

        assert_eq!(
            plan.repository_folders
                .get(&WorkspaceRepositoryName::from("front"))
                .map(RepositoryPath::as_str),
            Some("custom-front")
        );
        assert_eq!(plan.repository_worktrees.len(), 1);
        assert_eq!(plan.repository_worktrees[0].repository.as_str(), "front");
        assert!(
            plan.repository_worktrees[0]
                .worktree_path
                .as_str()
                .ends_with("custom-front")
        );
        assert_eq!(
            plan.repository_worktrees[0].default_branch.as_str(),
            "develop"
        );
    }

    #[test]
    fn execute_task_start_rejects_unpreparable_repository() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let project_root = temp.path().join("projects/ha");
        let plan = TaskStartPlan {
            work_item_ids: vec![WorkItemId::from("123")],
            primary_work_item_id: WorkItemId::from("123"),
            project: ProjectKey::from("ha"),
            task_id: None,
            kind: WorkItemTypeName::from("feat"),
            slug: TaskSlug::from("demo"),
            branch_name: BranchName::from("feat/123-demo"),
            subject_name: TaskSubjectName::from("feat-123-demo"),
            workspace: WorkspacePath::from(workspace.display().to_string()),
            repositories: vec![WorkspaceRepositoryName::from("front")],
            repository_folders: BTreeMap::from([(
                WorkspaceRepositoryName::from("front"),
                RepositoryPath::from("front"),
            )]),
            repository_worktrees: vec![TaskStartRepositoryPlan {
                repository: WorkspaceRepositoryName::from("front"),
                project_root: ProjectRootPath::from(project_root.display().to_string()),
                worktree_path: RepositoryPath::from(workspace.join("front").display().to_string()),
                url: GitRemoteUrl::from(
                    temp.path().join("missing-remote.git").display().to_string(),
                ),
                default_branch: BranchName::from("develop"),
                anchor_name: GitAnchorName::from("front.git"),
                git_credential_secret: None,
                branch_name: BranchName::from("feat/123-demo"),
            }],
        };

        let error = execute_task_start(&plan, None, None, None)
            .expect_err("invalid repository must not leave a fake workspace");

        assert!(matches!(
            error,
            WorkspaceError::WorktreePrepareFailed { repository, .. }
                if repository == WorkspaceRepositoryName::from("front")
        ));
        assert!(!workspace.join("task.json").exists());
    }

    #[test]
    fn execute_task_start_prepares_bare_repository_and_worktree() {
        let temp = tempdir().expect("tempdir should be created");
        let source = temp.path().join("source-front");
        init_develop_repository(&source);

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
            project: Some(&ProjectKey::from("ha")),
            task_id: None,
            type_name: Some("feat"),
            repositories: &[WorkspaceRepositoryName::from("front")],
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
            work_item_ids: vec![WorkItemId::from("123")],
            primary_work_item_id: WorkItemId::from("123"),
            project: ProjectKey::from("ha"),
            task_id: None,
            kind: WorkItemTypeName::from("feat"),
            slug: TaskSlug::from("demo"),
            branch_name: BranchName::from("feat/123-demo"),
            subject_name: TaskSubjectName::from("feat-123-demo"),
            workspace: WorkspacePath::from(workspace.display().to_string()),
            repositories: vec![
                WorkspaceRepositoryName::from("front"),
                WorkspaceRepositoryName::from("back"),
            ],
            repository_folders: BTreeMap::from([
                (
                    WorkspaceRepositoryName::from("front"),
                    RepositoryPath::from("front"),
                ),
                (
                    WorkspaceRepositoryName::from("back"),
                    RepositoryPath::from("back"),
                ),
            ]),
            repository_worktrees: Vec::new(),
        };

        let manifest = execute_task_start(&plan, None, None, None).expect("start should execute");

        assert_eq!(manifest.project, ProjectKey::from("ha"));
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
            work_item_ids: vec![WorkItemId::from("123")],
            primary_work_item_id: WorkItemId::from("123"),
            project: ProjectKey::from("ha"),
            task_id: None,
            kind: WorkItemTypeName::from("feat"),
            slug: TaskSlug::from("demo"),
            branch_name: BranchName::from("feat/123-demo"),
            subject_name: TaskSubjectName::from("feat-123-demo"),
            workspace: WorkspacePath::from("/tmp/workspace"),
            repositories: vec![WorkspaceRepositoryName::from("front")],
            repository_folders: BTreeMap::from([(
                WorkspaceRepositoryName::from("front"),
                RepositoryPath::from("front"),
            )]),
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

        assert_eq!(updated.task_id.as_ref().map(TaskId::as_str), Some("456"));
        assert_eq!(updated.branch_name.as_str(), "feat/123-456-demo");
    }

    #[test]
    fn execute_task_start_writes_child_tasks() {
        let temp = tempdir().expect("tempdir should be created");
        let workspace = temp.path().join("projects/ha/workspaces/feat-123-demo");
        let plan = TaskStartPlan {
            work_item_ids: vec![WorkItemId::from("123")],
            primary_work_item_id: WorkItemId::from("123"),
            project: ProjectKey::from("ha"),
            task_id: Some(TaskId::from("456")),
            kind: WorkItemTypeName::from("feat"),
            slug: TaskSlug::from("demo"),
            branch_name: BranchName::from("feat/123-456-demo"),
            subject_name: TaskSubjectName::from("feat-123-demo"),
            workspace: WorkspacePath::from(workspace.display().to_string()),
            repositories: vec![WorkspaceRepositoryName::from("front")],
            repository_folders: BTreeMap::from([(
                WorkspaceRepositoryName::from("front"),
                RepositoryPath::from("front"),
            )]),
            repository_worktrees: Vec::new(),
        };

        let manifest = execute_task_start_with_work_items_and_child_tasks(
            &plan,
            vec![WorkspaceWorkItem {
                id: WorkItemId::from("123"),
                kind: Some(WorkItemTypeName::from("User Story")),
                title: Some(WorkItemTitle::from("Demo")),
                state: Some(WorkItemState::from("En réalisation")),
            }],
            vec![WorkspaceChildTask {
                repository: WorkspaceRepositoryName::from("front"),
                id: WorkItemId::from("456"),
                title: Some(WorkItemTitle::from("[FRONT] Demo")),
            }],
        )
        .expect("start should execute");

        assert_eq!(manifest.normalized_child_tasks().len(), 1);
        assert_eq!(
            manifest.all_known_work_item_ids(),
            vec![WorkItemId::from("123"), WorkItemId::from("456")]
        );
        assert_eq!(manifest.branch_name, BranchName::from("feat/123-456-demo"));
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
            &typed_workspace_path(&workspace),
            &[AiContextFilePath::from(
                ai_context_path.display().to_string(),
            )],
        )
        .expect("report should build");

        assert_eq!(report.schema_version, "dw.task.preflight.v1");
        assert!(!report.has_blocking_issues);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.severity == TaskPreflightSeverity::Warning)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == TaskPreflightIssueCode::WorkspaceAdoContextStale)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == TaskPreflightIssueCode::AdoAttachmentsPresent)
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

        assert_eq!(resolved.as_str(), workspace.display().to_string());
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
            &typed_workspace_path(&workspace),
            &[WorkspaceRepositoryName::from("front")],
        )
        .expect("plan should build");

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].repository.as_str(), "front");
        assert_eq!(targets[0].default_branch.as_str(), "develop");
        assert!(
            targets[0]
                .repository_path
                .as_str()
                .ends_with("custom-front")
        );
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

        let (_manifest, targets) = plan_task_commit(&projects, &typed_workspace_path(&workspace))
            .expect("plan should build");

        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|target| {
            target.repository.as_str() == "front" && target.path.as_str().ends_with("custom-front")
        }));
        assert!(
            targets
                .iter()
                .any(|target| target.repository.as_str() == "back"
                    && target.path.as_str().ends_with("back"))
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
            build_commit_message(&manifest, None).as_str(),
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
            build_commit_message(&manifest, None).as_str(),
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
            build_commit_message(&manifest, Some(&CommitMessage::from("feat: descriptif")))
                .as_str(),
            "feat: descriptif #55201"
        );
        assert_eq!(
            build_commit_message(
                &manifest,
                Some(&CommitMessage::from("feat: descriptif #27485"))
            )
            .as_str(),
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
            &typed_workspace_path(&workspace),
            &[WorkItemId::from("55206")],
            Some("Bug"),
            Some("Secondaire"),
            Some("En developpement"),
        )
        .expect("plan should build");

        assert_eq!(plan.new_branch.as_str(), "feat/11010-55206-demo");
        assert!(
            plan.new_workspace
                .as_str()
                .ends_with("feat-11010-55206-demo")
        );
        assert_eq!(plan.work_items.len(), 2);
        assert_eq!(
            plan.work_items[1].title.as_ref().map(WorkItemTitle::as_str),
            Some("Secondaire")
        );
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
            &typed_workspace_path(&workspace),
            &[WorkItemSnapshot {
                id: "55206".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Secondaire".into()),
                url: None,
            }],
        )
        .expect("plan should build");

        assert_eq!(plan.new_branch.as_str(), "feat/11010-55206-demo");
        assert_eq!(
            plan.work_items[1]
                .kind
                .as_ref()
                .map(WorkItemTypeName::as_str),
            Some("Bug")
        );
        assert_eq!(
            plan.work_items[1].title.as_ref().map(WorkItemTitle::as_str),
            Some("Secondaire")
        );
        assert_eq!(
            plan.work_items[1].state.as_ref().map(WorkItemState::as_str),
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
            &typed_workspace_path(&workspace),
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
            &typed_workspace_path(&workspace),
            &[WorkItemId::from("55206")],
            None,
            None,
            None,
        )
        .expect("plan should build");

        let (updated, new_workspace) =
            execute_work_item_update(&manifest, &plan).expect("update should execute");

        assert_eq!(
            updated.branch_name,
            BranchName::from("feat/11010-55206-demo")
        );
        assert!(!workspace.exists());
        assert!(
            std::path::Path::new(new_workspace.as_str())
                .join("task.json")
                .exists()
        );
        assert!(
            std::path::Path::new(new_workspace.as_str())
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
            &typed_workspace_path(&workspace),
            &[WorkItemId::from("11010")],
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
            &typed_workspace_path(&workspace),
            "db",
        )
        .expect("plan should build");

        assert_eq!(plan.repository.as_str(), "db");
        assert_eq!(plan.default_branch.as_str(), "develop");
        assert_eq!(plan.anchor_name, GitAnchorName::from("database.git"));
        assert_eq!(plan.branch_name.as_str(), "feat/123-demo");
        assert!(plan.worktree_path.as_str().ends_with("database"));
        assert_eq!(
            plan.repositories,
            vec![
                WorkspaceRepositoryName::from("front"),
                WorkspaceRepositoryName::from("db")
            ]
        );
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
            &typed_workspace_path(&workspace),
            "db",
        )
        .expect("plan should build");

        let updated = execute_task_add_repo(&manifest, &plan).expect("add repo should execute");

        assert_eq!(
            updated.repositories,
            vec![
                WorkspaceRepositoryName::from("front"),
                WorkspaceRepositoryName::from("db")
            ]
        );
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
            &typed_workspace_path(&workspace),
            &[WorkItemSnapshot {
                id: "123".into(),
                kind: Some("Bug".into()),
                state: Some("En developpement".into()),
                title: Some("Titre ADO".into()),
                url: Some("https://dev.azure.com/org/project/_workitems/edit/123".into()),
            }],
        )
        .expect("sync should execute");

        assert_eq!(
            updated.work_item_title.as_ref().map(WorkItemTitle::as_str),
            Some("Titre ADO")
        );
        assert_eq!(
            updated.work_item_state.as_ref().map(WorkItemState::as_str),
            Some("En developpement")
        );
        let manifest_text = fs::read_to_string(workspace.join("task.json")).expect("manifest");
        assert!(manifest_text.contains("Titre ADO"));
    }

    #[test]
    fn workspace_work_item_display_includes_title_and_state() {
        let text = WorkspaceWorkItem {
            id: "55206".into(),
            kind: Some("Bug".into()),
            title: Some("Heures PSFs incoherentes affichees".into()),
            state: Some("Valide".into()),
        }
        .to_string();

        assert_eq!(text, "#55206 Heures PSFs incoherentes affichees [Valide]");
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
        assert_eq!(
            candidates[0].path.as_str(),
            final_workspace.display().to_string()
        );
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
            &typed_workspace_path(&workspace),
        )
        .expect("plan should build");
        let anchor = root.join("projects/ha/repositories/front.git");

        assert!(steps.iter().any(|step| {
            step.subject
                == WorkspaceTeardownSubject::Repository {
                    repository: WorkspaceRepositoryName::from("front"),
                }
                && step.action
                    == WorkspaceTeardownAction::WorktreeRemove {
                        worktree_path: RepositoryPath::from(
                            workspace.join("front").display().to_string(),
                        ),
                        git_dir: RepositoryPath::from(anchor.display().to_string()),
                    }
        }));
        assert!(steps.iter().any(|step| {
            step.subject
                == WorkspaceTeardownSubject::Repository {
                    repository: WorkspaceRepositoryName::from("front"),
                }
                && step.action
                    == WorkspaceTeardownAction::WorktreePrune {
                        git_dir: RepositoryPath::from(anchor.display().to_string()),
                    }
        }));
        assert!(steps.iter().any(|step| {
            step.subject == WorkspaceTeardownSubject::Workspace
                && step.action
                    == WorkspaceTeardownAction::DeleteWorkspace {
                        workspace: WorkspacePath::from(workspace.display().to_string()),
                    }
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
                subject: WorkspaceTeardownSubject::Repository {
                    repository: WorkspaceRepositoryName::from("front"),
                },
                action: WorkspaceTeardownAction::WorktreeRemove {
                    worktree_path: RepositoryPath::from(
                        workspace.join("front").display().to_string(),
                    ),
                    git_dir: RepositoryPath::from(anchor.display().to_string()),
                },
            },
            WorkspaceTeardownStep {
                subject: WorkspaceTeardownSubject::Repository {
                    repository: WorkspaceRepositoryName::from("front"),
                },
                action: WorkspaceTeardownAction::WorktreePrune {
                    git_dir: RepositoryPath::from(anchor.display().to_string()),
                },
            },
            WorkspaceTeardownStep {
                subject: WorkspaceTeardownSubject::Workspace,
                action: WorkspaceTeardownAction::DeleteWorkspace {
                    workspace: WorkspacePath::from(workspace.display().to_string()),
                },
            },
        ];
        let mut calls: Vec<WorkspaceGitOperation> = Vec::new();

        execute_task_teardown(&typed_workspace_path(&workspace), &steps, |operation| {
            calls.push(operation);
            Ok(())
        })
        .expect("teardown should execute");

        assert!(calls.iter().any(|operation| {
            operation
                == &WorkspaceGitOperation::WorktreeRemove {
                    git_dir: RepositoryPath::from(anchor.display().to_string()),
                    worktree_path: RepositoryPath::from(
                        workspace.join("front").display().to_string(),
                    ),
                }
        }));
        assert!(calls.iter().any(|operation| {
            operation
                == &WorkspaceGitOperation::WorktreePrune {
                    git_dir: RepositoryPath::from(anchor.display().to_string()),
                }
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
                subject: WorkspaceTeardownSubject::Repository {
                    repository: WorkspaceRepositoryName::from("front"),
                },
                action: WorkspaceTeardownAction::WorktreePrune {
                    git_dir: RepositoryPath::from(missing_anchor.display().to_string()),
                },
            },
            WorkspaceTeardownStep {
                subject: WorkspaceTeardownSubject::Workspace,
                action: WorkspaceTeardownAction::DeleteWorkspace {
                    workspace: WorkspacePath::from(workspace.display().to_string()),
                },
            },
        ];
        let mut calls = 0;

        execute_task_teardown(&typed_workspace_path(&workspace), &steps, |_operation| {
            calls += 1;
            Ok(())
        })
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
