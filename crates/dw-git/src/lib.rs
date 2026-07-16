use anyhow::{Result, anyhow};
use dw_core::{
    BranchName, CommitMessage, GitAnchorName, GitReferenceName, GitRemoteUrl, GitRevision,
    ProjectRootPath, RepositoryPath, SecretValue, TaskSlug, TaskSubjectName, WorkItemId,
    WorkItemTypeName, WorkspaceRepositoryName,
};
use git2::{
    CertificateCheckStatus, Config as GitConfig, Cred, CredentialType, FetchOptions,
    IndexAddOption, ObjectType, Oid, PushOptions, RebaseOptions, RemoteCallbacks, Repository, Sort,
    StashFlags, StatusOptions, WorktreeAddOptions, build::RepoBuilder,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use thiserror::Error;

const ENV_DW_ADO_TOKEN: &str = "DW_ADO_TOKEN";
const ENV_AZURE_DEVOPS_EXT_PAT: &str = "AZURE_DEVOPS_EXT_PAT";
const FALLBACK_SSH_REMOTE: &str = "dw-ssh";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitRewriteNote {
    pub strategy: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryStatus {
    pub path: RepositoryPath,
    #[serde(rename = "isGitRepository")]
    pub is_git_repository: bool,
    #[serde(rename = "hasChanges")]
    pub has_changes: bool,
    #[serde(rename = "hasUnpushed")]
    pub has_unpushed: bool,
    pub detail: RepositoryStatusDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RepositoryStatusDetail {
    MissingDirectory,
    OpenFailed { detail: GitErrorDetail },
    StatusFailed { detail: GitErrorDetail },
    Changed { paths: Vec<RepositoryStatusPath> },
    Unpushed { ahead: GitCommitCount },
    Clean,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct RepositoryStatusPath(String);

impl RepositoryStatusPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RepositoryStatusPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for RepositoryStatusPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for RepositoryStatusPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct GitCommitCount(usize);

impl GitCommitCount {
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    pub fn as_usize(self) -> usize {
        self.0
    }
}

impl From<usize> for GitCommitCount {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for GitCommitCount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitRevisionRange {
    pub from: GitRevision,
    pub to: GitRevision,
}

impl GitRevisionRange {
    pub fn new(from: GitRevision, to: GitRevision) -> Self {
        Self { from, to }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct GitCommitMessages(String);

impl GitCommitMessages {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreePrepareRequest {
    #[serde(rename = "projectRoot")]
    pub project_root: ProjectRootPath,
    pub repository: WorkspaceRepositoryName,
    #[serde(rename = "httpUrl")]
    pub http_url: GitRemoteUrl,
    #[serde(rename = "sshUrl")]
    pub ssh_url: Option<GitRemoteUrl>,
    #[serde(rename = "defaultBranch")]
    pub default_branch: BranchName,
    #[serde(rename = "anchorName")]
    pub anchor_name: GitAnchorName,
    #[serde(rename = "branchName")]
    pub branch_name: BranchName,
    #[serde(rename = "worktreePath")]
    pub worktree_path: RepositoryPath,
    #[serde(skip)]
    pub credential: Option<GitCredential>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreePrepareResult {
    pub repository: WorkspaceRepositoryName,
    pub status: WorktreePrepareStatus,
    pub detail: WorktreePrepareDetail,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreePrepareStatus {
    Placeholder,
    Prepared,
}

impl fmt::Display for WorktreePrepareStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Placeholder => formatter.write_str("placeholder"),
            Self::Prepared => formatter.write_str("prepared"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorktreePrepareDetail {
    MissingRemoteUrl,
    AlreadyPresent,
    CreatedFromExistingBranch { branch: BranchName },
    CreatedFromBaseReference { reference: GitReferenceName },
}

#[derive(Clone, PartialEq, Eq)]
pub struct GitCredential {
    token: SecretValue,
}

impl GitCredential {
    pub fn personal_access_token(token: SecretValue) -> Self {
        Self { token }
    }

    fn token(&self) -> &SecretValue {
        &self.token
    }
}

impl fmt::Debug for GitCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitCredential(***)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitOperation {
    OpenRepository,
    Status,
    Log,
    Fetch,
    Rebase,
    Commit,
    Push,
    CloneBare,
    ConfigureRemote,
    WorktreeAdd,
    WorktreeRemove,
    WorktreePrune,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOperationInvocation {
    pub operation: GitOperation,
    pub repository_path: Option<RepositoryPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct GitErrorDetail(String);

impl GitErrorDetail {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.trim())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitAuthFailureKind {
    HttpsCredentialMissing,
    HttpsCredentialRejected,
    SshHostKeyMissing,
    SshKeyUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitAuthRemediation {
    ConfigureHttpsCredential,
    VerifyHttpsCredential,
    TrustSshHostKey,
    ConfigureSshKey,
}

impl fmt::Display for GitAuthRemediation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigureHttpsCredential => formatter.write_str(
                "Configure gitCredentialSecret in projects.json and store the PAT with dw secret set <key>, or set DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT, or configure a non-interactive Git credential helper",
            ),
            Self::VerifyHttpsCredential => formatter.write_str(
                "Verify that the Git PAT can access the repository and is not expired",
            ),
            Self::TrustSshHostKey => formatter.write_str(
                "Preload the SSH fingerprint outside dw, for example with ssh-keyscan/known_hosts or a validated manual ssh connection",
            ),
            Self::ConfigureSshKey => formatter.write_str(
                "Load a valid SSH key into the agent or configure repository access before rerunning dw",
            ),
        }
    }
}

impl fmt::Display for GitAuthFailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpsCredentialMissing => formatter.write_str("missing Git HTTPS credential"),
            Self::HttpsCredentialRejected => formatter.write_str("rejected Git HTTPS credential"),
            Self::SshHostKeyMissing => formatter.write_str("unknown SSH fingerprint"),
            Self::SshKeyUnavailable => formatter.write_str("SSH key unavailable or rejected"),
        }
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("{kind}. {remediation}. Git detail: {detail}")]
    Authentication {
        kind: GitAuthFailureKind,
        remediation: GitAuthRemediation,
        detail: GitErrorDetail,
        invocation: GitOperationInvocation,
    },
    #[error("Git {operation:?} failed: {detail}")]
    OperationFailed {
        operation: GitOperation,
        detail: GitErrorDetail,
        invocation: GitOperationInvocation,
    },
}

pub fn current_strategy() -> GitRewriteNote {
    GitRewriteNote {
        strategy: "git2",
        status: "active",
    }
}

pub fn normalize_slug(value: &str) -> String {
    if value.trim().is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mut previous_dash = false;
    for c in deunicode::deunicode(value).chars() {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            output.push(lower);
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }

    output
        .trim_matches('-')
        .chars()
        .take(50)
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn slug_from_phrase_or_fallback(value: Option<&str>, fallback: &str) -> TaskSlug {
    let normalized = normalize_slug(value.unwrap_or_default());
    if normalized.is_empty() {
        TaskSlug::from(normalize_slug(fallback))
    } else {
        TaskSlug::from(normalized)
    }
}

pub fn build_branch_name(
    type_name: &WorkItemTypeName,
    work_item_ids: &[WorkItemId],
    slug: &TaskSlug,
) -> BranchName {
    let clean_type = if type_name.as_str().trim().is_empty() {
        "feat"
    } else {
        type_name.as_str().trim()
    }
    .to_ascii_lowercase();
    let id_part = distinct_non_empty(work_item_ids).join("-");
    BranchName::from(format!(
        "{clean_type}/{id_part}-{}",
        normalize_slug(slug.as_str())
    ))
}

pub fn build_subject_name(
    type_name: &WorkItemTypeName,
    work_item_ids: &[WorkItemId],
    slug: &TaskSlug,
) -> TaskSubjectName {
    let clean_type = if type_name.as_str().trim().is_empty() {
        "feat"
    } else {
        type_name.as_str().trim()
    }
    .to_ascii_lowercase();
    let id_part = distinct_non_empty(work_item_ids).join("-");
    TaskSubjectName::from(format!(
        "{clean_type}-{id_part}-{}",
        normalize_slug(slug.as_str())
    ))
}

pub fn resolve_remote_source_branch(default_branch: &BranchName) -> GitReferenceName {
    GitReferenceName::from(format!("origin/{default_branch}"))
}

pub fn update_repository(
    repository_path: &RepositoryPath,
    default_branch: &BranchName,
    credential: Option<&GitCredential>,
    ssh_url: Option<&GitRemoteUrl>,
) -> Result<()> {
    let mut repository = Repository::open(repository_path.as_str()).map_err(git2_command_error)?;
    configure_ssh_remote(&repository, ssh_url)?;
    let has_changes = repository_has_changes(&repository)?;
    let mut stashed = false;

    if has_changes {
        let signature = repository.signature().map_err(git2_command_error)?;
        repository
            .stash_save(
                &signature,
                "dw repository update autostash",
                Some(StashFlags::INCLUDE_UNTRACKED),
            )
            .map_err(git2_command_error)?;
        stashed = true;
    }

    fetch_anchor_repository(&repository, credential, ssh_url)?;
    let source_branch = resolve_remote_source_branch(default_branch);
    rebase_current_branch(&repository, &source_branch).map_err(|error| {
        anyhow!(
            "Rebase conflict. Rerun manually with: git -C \"{}\" fetch --prune origin then git -C \"{}\" rebase {}. Cause: {}",
            repository_path,
            repository_path,
            source_branch,
            error
        )
    })?;

    if stashed {
        repository.stash_pop(0, None).map_err(git2_command_error)?;
    }

    Ok(())
}

pub fn repository_status(repository_path: &RepositoryPath) -> RepositoryStatus {
    if !Path::new(repository_path.as_str()).is_dir() {
        return RepositoryStatus {
            path: repository_path.clone(),
            is_git_repository: false,
            has_changes: false,
            has_unpushed: false,
            detail: RepositoryStatusDetail::MissingDirectory,
        };
    }

    let repository = match Repository::open(repository_path.as_str()) {
        Ok(repository) => repository,
        Err(error) => {
            return RepositoryStatus {
                path: repository_path.clone(),
                is_git_repository: false,
                has_changes: false,
                has_unpushed: false,
                detail: RepositoryStatusDetail::OpenFailed {
                    detail: GitErrorDetail::new(error.message()),
                },
            };
        }
    };
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = match repository.statuses(Some(&mut options)) {
        Ok(statuses) => statuses,
        Err(error) => {
            return RepositoryStatus {
                path: repository_path.clone(),
                is_git_repository: false,
                has_changes: false,
                has_unpushed: false,
                detail: RepositoryStatusDetail::StatusFailed {
                    detail: GitErrorDetail::new(error.message()),
                },
            };
        }
    };
    let changed_paths = statuses
        .iter()
        .filter_map(|entry| entry.path().ok().map(RepositoryStatusPath::from))
        .collect::<Vec<_>>();

    if !changed_paths.is_empty() {
        return RepositoryStatus {
            path: repository_path.clone(),
            is_git_repository: true,
            has_changes: true,
            has_unpushed: false,
            detail: RepositoryStatusDetail::Changed {
                paths: changed_paths,
            },
        };
    }

    let ahead = repository
        .revparse_single("HEAD")
        .ok()
        .zip(repository.revparse_single("@{u}").ok())
        .and_then(|(head, upstream)| {
            repository
                .graph_ahead_behind(head.id(), upstream.id())
                .ok()
                .map(|(ahead, _behind)| ahead)
        })
        .unwrap_or(0);

    RepositoryStatus {
        path: repository_path.clone(),
        is_git_repository: true,
        has_changes: false,
        has_unpushed: ahead > 0,
        detail: if ahead > 0 {
            RepositoryStatusDetail::Unpushed {
                ahead: GitCommitCount::from(ahead),
            }
        } else {
            RepositoryStatusDetail::Clean
        },
    }
}

pub fn has_commits_ahead_of(repository_path: &RepositoryPath, base: &GitRevision) -> Result<bool> {
    let repository = Repository::open(repository_path.as_str()).map_err(git2_command_error)?;
    let head = repository
        .revparse_single("HEAD")
        .map_err(git2_command_error)?;
    let base = repository
        .revparse_single(base.as_str())
        .map_err(git2_command_error)?;
    let (ahead, _behind) = repository
        .graph_ahead_behind(head.id(), base.id())
        .map_err(git2_command_error)?;
    Ok(ahead > 0)
}

pub fn commit_messages_in_range(range: &GitRevisionRange) -> Result<GitCommitMessages> {
    commit_messages_in_range_at(&RepositoryPath::from("."), range)
}

pub fn commit_messages_in_range_at(
    repository_path: &RepositoryPath,
    range: &GitRevisionRange,
) -> Result<GitCommitMessages> {
    let repository = Repository::discover(repository_path.as_str()).map_err(git2_command_error)?;
    let from = repository
        .revparse_single(range.from.as_str())
        .map_err(git2_command_error)?;
    let to = repository
        .revparse_single(range.to.as_str())
        .map_err(git2_command_error)?;
    let mut walk = repository.revwalk().map_err(git2_command_error)?;
    walk.push(to.id()).map_err(git2_command_error)?;
    walk.hide(from.id()).map_err(git2_command_error)?;
    walk.set_sorting(Sort::TIME).map_err(git2_command_error)?;

    let mut messages = String::new();
    for oid in walk {
        let oid = oid.map_err(git2_command_error)?;
        let commit = repository.find_commit(oid).map_err(git2_command_error)?;
        let message = commit.message().map_err(git2_command_error)?;
        messages.push_str(message);
        messages.push('\u{1e}');
        messages.push('\n');
    }
    Ok(GitCommitMessages::new(messages))
}

pub fn commit_repository(repository_path: &RepositoryPath, message: &CommitMessage) -> Result<()> {
    let repository = Repository::open(repository_path.as_str()).map_err(git2_command_error)?;
    let mut index = repository.index().map_err(git2_command_error)?;
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .map_err(git2_command_error)?;
    index.write().map_err(git2_command_error)?;
    let tree_id = index.write_tree().map_err(git2_command_error)?;
    let tree = repository.find_tree(tree_id).map_err(git2_command_error)?;
    let signature = repository.signature().map_err(git2_command_error)?;
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok());
    let parents = parent.iter().collect::<Vec<_>>();
    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message.as_str(),
            &tree,
            &parents,
        )
        .map_err(git2_command_error)?;
    Ok(())
}

pub fn push_repository(repository_path: &RepositoryPath, branch_name: &BranchName) -> Result<()> {
    push_repository_inner(repository_path, branch_name, false)
}

pub fn push_repository_force_with_lease(
    repository_path: &RepositoryPath,
    branch_name: &BranchName,
) -> Result<()> {
    push_repository_inner(repository_path, branch_name, true)
}

fn push_repository_inner(
    repository_path: &RepositoryPath,
    branch_name: &BranchName,
    force_with_lease: bool,
) -> Result<()> {
    let repository = Repository::open(repository_path.as_str()).map_err(git2_command_error)?;
    let ssh_url = configured_ssh_remote_url(&repository).map(GitRemoteUrl::from);
    let destination = format!("refs/heads/{branch_name}");
    let refspec = format!(
        "{}refs/heads/{branch_name}:{destination}",
        if force_with_lease { "+" } else { "" }
    );
    let expected_remote_oid = force_with_lease
        .then(|| remote_tracking_oid(&repository, branch_name))
        .transpose()?;
    match push_remote(
        &repository,
        "origin",
        &refspec,
        None,
        expected_remote_oid.map(|oid| (destination.as_str(), oid)),
    ) {
        Ok(()) => {}
        Err(error) if should_try_ssh_fallback(&error) && ssh_url.is_some() => {
            push_remote(
                &repository,
                FALLBACK_SSH_REMOTE,
                &refspec,
                None,
                expected_remote_oid.map(|oid| (destination.as_str(), oid)),
            )?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn remote_tracking_oid(
    repository: &Repository,
    branch_name: &BranchName,
) -> std::result::Result<Oid, GitError> {
    let reference_name = format!("refs/remotes/origin/{branch_name}");
    match repository.find_reference(&reference_name) {
        Ok(reference) => reference
            .peel_to_commit()
            .map(|commit| commit.id())
            .map_err(git2_command_error),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(Oid::ZERO_SHA1),
        Err(error) => Err(git2_command_error(error)),
    }
}

pub fn prepare_worktree(request: &WorktreePrepareRequest) -> Result<WorktreePrepareResult> {
    if request.http_url.as_str().trim().is_empty() {
        std::fs::create_dir_all(request.worktree_path.as_str())?;
        return Ok(WorktreePrepareResult {
            repository: request.repository.clone(),
            status: WorktreePrepareStatus::Placeholder,
            detail: WorktreePrepareDetail::MissingRemoteUrl,
        });
    }

    let repositories_root = Path::new(request.project_root.as_str()).join("repositories");
    let anchor_path = repositories_root.join(request.anchor_name.as_str());
    std::fs::create_dir_all(&repositories_root)?;
    let normalized_url = normalize_git_remote_url(&request.http_url);

    let anchor_repository = if !anchor_path.is_dir() {
        clone_bare_repository(
            &normalized_url,
            request.ssh_url.as_ref(),
            &anchor_path,
            request.credential.as_ref(),
        )?
    } else {
        ensure_origin_url(&anchor_path, &normalized_url)?;
        Repository::open_bare(&anchor_path).map_err(git2_command_error)?
    };
    configure_ssh_remote(&anchor_repository, request.ssh_url.as_ref())?;
    configure_anchor_fetch_refspec(&anchor_repository)?;
    fetch_anchor_repository(
        &anchor_repository,
        request.credential.as_ref(),
        request.ssh_url.as_ref(),
    )?;

    if Path::new(request.worktree_path.as_str()).is_dir() {
        return Ok(WorktreePrepareResult {
            repository: request.repository.clone(),
            status: WorktreePrepareStatus::Prepared,
            detail: WorktreePrepareDetail::AlreadyPresent,
        });
    }

    let base_ref = [
        format!("origin/{}", request.default_branch),
        format!("refs/heads/{}", request.default_branch),
    ]
    .into_iter()
    .find(|candidate| anchor_repository.revparse_single(candidate).is_ok())
    .ok_or_else(|| {
        anyhow!(
            "Base branch not found: {}. Tried references: origin/{}, refs/heads/{}.",
            request.default_branch,
            request.default_branch,
            request.default_branch
        )
    })?;
    let branch_ref = format!("refs/heads/{}", request.branch_name);
    let branch_exists = anchor_repository.find_reference(&branch_ref).is_ok();
    if !branch_exists {
        let base_object = anchor_repository
            .revparse_single(&base_ref)
            .map_err(git2_command_error)?;
        let base_commit = base_object
            .peel(ObjectType::Commit)
            .and_then(|object| {
                object
                    .into_commit()
                    .map_err(|_| git2::Error::from_str("base reference is not a commit"))
            })
            .map_err(git2_command_error)?;
        anchor_repository
            .branch(request.branch_name.as_str(), &base_commit, false)
            .map_err(git2_command_error)?;
    }
    let reference = anchor_repository
        .find_reference(&branch_ref)
        .map_err(git2_command_error)?;
    let mut options = WorktreeAddOptions::new();
    options.reference(Some(&reference));
    anchor_repository
        .worktree(
            &worktree_name(request),
            Path::new(request.worktree_path.as_str()),
            Some(&options),
        )
        .map_err(git2_command_error)?;

    Ok(WorktreePrepareResult {
        repository: request.repository.clone(),
        status: WorktreePrepareStatus::Prepared,
        detail: if branch_exists {
            WorktreePrepareDetail::CreatedFromExistingBranch {
                branch: request.branch_name.clone(),
            }
        } else {
            WorktreePrepareDetail::CreatedFromBaseReference {
                reference: GitReferenceName::from(base_ref),
            }
        },
    })
}

fn clone_bare_repository(
    url: &GitRemoteUrl,
    ssh_url: Option<&GitRemoteUrl>,
    anchor_path: &Path,
    credential: Option<&GitCredential>,
) -> std::result::Result<Repository, GitError> {
    let origin_fetch_options = fetch_options(credential, Some(url.as_str()));
    let mut builder = RepoBuilder::new();
    builder.bare(true).fetch_options(origin_fetch_options);
    match builder.clone(url.as_str(), anchor_path).map_err(|error| {
        git2_auth_error(error, credential_available(credential, Some(url.as_str())))
    }) {
        Ok(repository) => Ok(repository),
        Err(error) if should_try_ssh_fallback(&error) && ssh_url.is_some() => {
            let ssh_url = normalize_git_remote_url(ssh_url.expect("checked is_some"));
            let fetch_options = fetch_options(None, Some(ssh_url.as_str()));
            let mut builder = RepoBuilder::new();
            builder.bare(true).fetch_options(fetch_options);
            let repository = builder
                .clone(ssh_url.as_str(), anchor_path)
                .map_err(|error| git2_auth_error(error, false))?;
            repository
                .remote_set_url("origin", url.as_str())
                .map_err(git2_command_error)?;
            configure_ssh_remote(&repository, Some(&ssh_url))?;
            Ok(repository)
        }
        Err(error) => Err(error),
    }
}

fn ensure_origin_url(anchor_path: &Path, url: &GitRemoteUrl) -> std::result::Result<(), GitError> {
    let repository = Repository::open_bare(anchor_path).map_err(git2_command_error)?;
    let current = repository
        .find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().ok().map(ToString::to_string));
    if current.as_deref() != Some(url.as_str()) {
        repository
            .remote_set_url("origin", url.as_str())
            .map_err(git2_command_error)?;
    }
    Ok(())
}

fn configure_ssh_remote(
    repository: &Repository,
    ssh_url: Option<&GitRemoteUrl>,
) -> std::result::Result<(), GitError> {
    let Some(ssh_url) = ssh_url else {
        return Ok(());
    };
    if ssh_url.as_str().trim().is_empty() {
        return Ok(());
    }
    let ssh_url = normalize_git_remote_url(ssh_url);
    match repository.find_remote(FALLBACK_SSH_REMOTE) {
        Ok(remote) if remote.url().ok() == Some(ssh_url.as_str()) => Ok(()),
        Ok(_) => repository
            .remote_set_url(FALLBACK_SSH_REMOTE, ssh_url.as_str())
            .map_err(git2_command_error),
        Err(_) => repository
            .remote(FALLBACK_SSH_REMOTE, ssh_url.as_str())
            .map(|_| ())
            .map_err(git2_command_error),
    }
}

fn configured_ssh_remote_url(repository: &Repository) -> Option<String> {
    repository
        .find_remote(FALLBACK_SSH_REMOTE)
        .ok()
        .and_then(|remote| remote.url().ok().map(str::to_string))
        .filter(|url| !url.trim().is_empty())
}

fn should_try_ssh_fallback(error: &GitError) -> bool {
    matches!(
        error,
        GitError::Authentication {
            kind: GitAuthFailureKind::HttpsCredentialMissing
                | GitAuthFailureKind::HttpsCredentialRejected,
            ..
        }
    )
}

fn normalize_git_remote_url(url: &GitRemoteUrl) -> GitRemoteUrl {
    let raw = url.as_str().trim();
    if raw.contains("://")
        || raw.starts_with('/')
        || raw.starts_with("./")
        || raw.starts_with("../")
    {
        return GitRemoteUrl::from(raw);
    }
    let Some((authority, path)) = raw.split_once(':') else {
        return GitRemoteUrl::from(raw);
    };
    if authority.len() == 1
        || authority.contains('/')
        || authority.contains('\\')
        || path.trim().is_empty()
    {
        return GitRemoteUrl::from(raw);
    }
    GitRemoteUrl::from(format!("ssh://{authority}/{path}"))
}

fn configure_anchor_fetch_refspec(repository: &Repository) -> std::result::Result<(), GitError> {
    let mut config = repository.config().map_err(git2_command_error)?;
    config
        .set_str("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*")
        .map_err(git2_command_error)
}

fn fetch_anchor_repository(
    repository: &Repository,
    credential: Option<&GitCredential>,
    ssh_url: Option<&GitRemoteUrl>,
) -> std::result::Result<(), GitError> {
    configure_ssh_remote(repository, ssh_url)?;
    match fetch_remote(repository, "origin", credential) {
        Ok(()) => Ok(()),
        Err(error) if should_try_ssh_fallback(&error) && ssh_url.is_some() => {
            fetch_remote(repository, FALLBACK_SSH_REMOTE, None)
        }
        Err(error) => Err(error),
    }
}

fn fetch_remote(
    repository: &Repository,
    remote_name: &str,
    credential: Option<&GitCredential>,
) -> std::result::Result<(), GitError> {
    let mut remote = repository
        .find_remote(remote_name)
        .map_err(git2_command_error)?;
    let remote_url = remote.url().ok().map(str::to_string);
    let mut fetch_options = fetch_options(credential, remote_url.as_deref());
    remote
        .fetch(
            &["+refs/heads/*:refs/remotes/origin/*"],
            Some(&mut fetch_options),
            None,
        )
        .map_err(|error| {
            git2_auth_error(
                error,
                credential_available(credential, remote_url.as_deref()),
            )
        })
}

fn push_remote(
    repository: &Repository,
    remote_name: &str,
    refspec: &str,
    credential: Option<&GitCredential>,
    lease: Option<(&str, Oid)>,
) -> std::result::Result<(), GitError> {
    let mut remote = repository
        .find_remote(remote_name)
        .map_err(git2_command_error)?;
    let remote_url = remote.url().ok().map(str::to_string);
    let mut callbacks = remote_callbacks(credential, remote_url.as_deref());
    callbacks.push_update_reference(|reference, status| match status {
        Some(status) => Err(git2::Error::from_str(&format!(
            "Push rejected for {reference}: {status}"
        ))),
        None => Ok(()),
    });
    if let Some((destination, expected_oid)) = lease {
        let destination = destination.to_string();
        callbacks.push_negotiation(move |updates| {
            let update = updates
                .iter()
                .find(|update| update.dst_refname().ok() == Some(destination.as_str()))
                .ok_or_else(|| {
                    git2::Error::from_str("Force-with-lease target was not negotiated")
                })?;
            if update.src() != expected_oid {
                return Err(git2::Error::from_str(
                    "Force-with-lease rejected: remote branch changed",
                ));
            }
            Ok(())
        });
    }
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks);
    remote
        .push(&[refspec], Some(&mut options))
        .map_err(|error| {
            git2_auth_error(
                error,
                credential_available(credential, remote_url.as_deref()),
            )
        })
}

fn repository_has_changes(repository: &Repository) -> std::result::Result<bool, GitError> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repository
        .statuses(Some(&mut options))
        .map_err(git2_command_error)?;
    Ok(!statuses.is_empty())
}

fn rebase_current_branch(
    repository: &Repository,
    upstream_ref: &GitReferenceName,
) -> std::result::Result<(), GitError> {
    let upstream = repository
        .revparse_single(upstream_ref.as_str())
        .map_err(git2_command_error)?;
    let head = repository
        .revparse_single("HEAD")
        .map_err(git2_command_error)?;
    if head.id() == upstream.id() {
        return Ok(());
    }
    let upstream = repository
        .find_annotated_commit(upstream.id())
        .map_err(git2_command_error)?;
    let signature = repository.signature().map_err(git2_command_error)?;
    let mut options = RebaseOptions::new();
    options.quiet(true);
    let mut rebase = repository
        .rebase(None, Some(&upstream), None, Some(&mut options))
        .map_err(git2_command_error)?;

    while let Some(operation) = rebase.next() {
        if let Err(error) = operation {
            let _ = rebase.abort();
            return Err(git2_command_error(error));
        }
        let index = repository.index().map_err(git2_command_error)?;
        if index.has_conflicts() {
            let _ = rebase.abort();
            return Err(GitError::OperationFailed {
                operation: GitOperation::Rebase,
                detail: GitErrorDetail::new("Rebase conflict"),
                invocation: GitOperationInvocation {
                    operation: GitOperation::Rebase,
                    repository_path: None,
                },
            });
        }
        if let Err(error) = rebase.commit(None, &signature, None) {
            let _ = rebase.abort();
            return Err(git2_command_error(error));
        }
    }
    rebase.finish(Some(&signature)).map_err(git2_command_error)
}

fn fetch_options(
    credential: Option<&GitCredential>,
    remote_url: Option<&str>,
) -> FetchOptions<'static> {
    let callbacks = remote_callbacks(credential, remote_url);
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    fetch_options
}

fn remote_callbacks(
    credential: Option<&GitCredential>,
    remote_url: Option<&str>,
) -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    let credential = credential
        .cloned()
        .or_else(|| remote_url.and_then(|url| fallback_environment_credential(url, credential)));
    callbacks.certificate_check(|_, _| Ok(CertificateCheckStatus::CertificateOk));
    callbacks.credentials(move |url, username_from_url, allowed_types| {
        if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
            if let Some(credential) = credential.as_ref() {
                return Cred::userpass_plaintext(
                    username_from_url.unwrap_or("dw"),
                    credential.token().as_str(),
                );
            }
            if let Ok(config) = GitConfig::open_default()
                && let Ok(credential) = Cred::credential_helper(&config, url, username_from_url)
            {
                return Ok(credential);
            }
        }
        if let Some(username) = username_from_url
            && allowed_types.contains(CredentialType::SSH_KEY)
        {
            return Cred::ssh_key_from_agent(username);
        }
        Err(git2::Error::from_str("missing Git HTTPS credential"))
    });
    callbacks
}

fn worktree_name(request: &WorktreePrepareRequest) -> String {
    let raw = format!("{}-{}", request.repository, request.branch_name);
    let mut name = String::new();
    let mut previous_dash = false;
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            name.push('-');
            previous_dash = true;
        }
    }
    name.trim_matches('-').to_string()
}

pub fn worktree_remove(git_dir: &RepositoryPath, worktree_path: &RepositoryPath) -> Result<()> {
    let repository = Repository::open_bare(git_dir.as_str()).map_err(git2_command_error)?;
    if Path::new(worktree_path.as_str()).exists() {
        std::fs::remove_dir_all(worktree_path.as_str())?;
    }
    if let Some(name) = find_worktree_name_by_path(&repository, Path::new(worktree_path.as_str()))?
    {
        let worktree = repository
            .find_worktree(&name)
            .map_err(git2_command_error)?;
        worktree.prune(None).map_err(git2_command_error)?;
    }
    Ok(())
}

pub fn worktree_prune(git_dir: &RepositoryPath) -> Result<()> {
    let repository = Repository::open_bare(git_dir.as_str()).map_err(git2_command_error)?;
    let worktrees = repository.worktrees().map_err(git2_command_error)?;
    for name in worktrees.iter().filter_map(|name| name.ok().flatten()) {
        let worktree = repository.find_worktree(name).map_err(git2_command_error)?;
        if worktree.is_prunable(None).map_err(git2_command_error)? {
            worktree.prune(None).map_err(git2_command_error)?;
        }
    }
    Ok(())
}

fn find_worktree_name_by_path(repository: &Repository, path: &Path) -> Result<Option<String>> {
    let target = normalize_path_for_compare(path);
    let worktrees = repository.worktrees().map_err(git2_command_error)?;
    for name in worktrees.iter().filter_map(|name| name.ok().flatten()) {
        let worktree = repository.find_worktree(name).map_err(git2_command_error)?;
        if normalize_path_for_compare(worktree.path()) == target {
            return Ok(Some(name.into()));
        }
    }
    Ok(None)
}

fn normalize_path_for_compare(path: &Path) -> String {
    path.components()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string()
}

fn git2_command_error(error: git2::Error) -> GitError {
    git2_auth_error(error, false)
}

fn git2_auth_error(error: git2::Error, credential_was_available: bool) -> GitError {
    let detail = GitErrorDetail::new(error.message().to_string());
    let invocation = GitOperationInvocation {
        operation: GitOperation::OpenRepository,
        repository_path: None,
    };
    if let Some(kind) = classify_auth_failure(detail.as_str(), credential_was_available) {
        return GitError::Authentication {
            kind,
            remediation: auth_remediation(kind),
            detail,
            invocation,
        };
    }
    GitError::OperationFailed {
        operation: GitOperation::OpenRepository,
        detail,
        invocation,
    }
}

fn is_azure_devops_url(url: &str) -> bool {
    let normalized = url.to_ascii_lowercase();
    normalized.contains("dev.azure.com") || normalized.contains("visualstudio.com")
}

fn git_credential_from_environment() -> Option<GitCredential> {
    std::env::var(ENV_DW_ADO_TOKEN)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var(ENV_AZURE_DEVOPS_EXT_PAT)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .map(SecretValue::from)
        .map(GitCredential::personal_access_token)
}

fn fallback_environment_credential(
    remote_url: &str,
    explicit_credential: Option<&GitCredential>,
) -> Option<GitCredential> {
    if should_use_environment_credential(remote_url, explicit_credential.is_some()) {
        return git_credential_from_environment();
    }

    None
}

fn should_use_environment_credential(
    remote_url: &str,
    explicit_credential_available: bool,
) -> bool {
    !explicit_credential_available && is_azure_devops_url(remote_url)
}

fn credential_available(credential: Option<&GitCredential>, remote_url: Option<&str>) -> bool {
    credential.is_some()
        || (remote_url.is_some_and(|url| should_use_environment_credential(url, false))
            && git_credential_from_environment().is_some())
}

fn classify_auth_failure(
    stderr: &str,
    credential_was_available: bool,
) -> Option<GitAuthFailureKind> {
    let normalized = stderr.to_ascii_lowercase();
    if normalized.contains("host key verification failed")
        || normalized.contains("remote host identification has changed")
    {
        return Some(GitAuthFailureKind::SshHostKeyMissing);
    }
    if normalized.contains("permission denied (publickey)")
        || normalized.contains("permission denied (publickey,password)")
    {
        return Some(GitAuthFailureKind::SshKeyUnavailable);
    }
    if normalized.contains("terminal prompts disabled")
        || normalized.contains("could not read username")
        || normalized.contains("could not read password")
        || normalized.contains("authentication failed")
        || normalized.contains("missing git https credential")
        || normalized.contains("authentication required")
    {
        return Some(if credential_was_available {
            GitAuthFailureKind::HttpsCredentialRejected
        } else {
            GitAuthFailureKind::HttpsCredentialMissing
        });
    }
    None
}

fn auth_remediation(kind: GitAuthFailureKind) -> GitAuthRemediation {
    match kind {
        GitAuthFailureKind::HttpsCredentialMissing => GitAuthRemediation::ConfigureHttpsCredential,
        GitAuthFailureKind::HttpsCredentialRejected => GitAuthRemediation::VerifyHttpsCredential,
        GitAuthFailureKind::SshHostKeyMissing => GitAuthRemediation::TrustSshHostKey,
        GitAuthFailureKind::SshKeyUnavailable => GitAuthRemediation::ConfigureSshKey,
    }
}

fn distinct_non_empty(values: &[WorkItemId]) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if value.as_str().trim().is_empty() {
            continue;
        }

        if !result
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(value.as_str()))
        {
            result.push(value.to_string());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{RepositoryInitOptions, ResetType, Signature};
    use std::fs;
    use std::path::Path;

    #[test]
    fn normalize_creates_ascii_dash_slug() {
        assert_eq!(normalize_slug("example description"), "example-description");
        assert_eq!(
            normalize_slug("example values with accents"),
            "example-values-with-accents"
        );
        assert_eq!(normalize_slug("  Trop   d'espaces !!! "), "trop-d-espaces");
        assert_eq!(
            normalize_slug("ceci est un Test hehe"),
            "ceci-est-un-test-hehe"
        );
    }

    #[test]
    fn force_with_lease_rejects_stale_remote_tracking_ref() {
        let temp = tempfile::tempdir().expect("tempdir");
        let remote_path = temp.path().join("remote.git");
        let local_path = temp.path().join("local");
        let remote = Repository::init_bare(&remote_path).expect("bare remote");
        let mut init = RepositoryInitOptions::new();
        init.initial_head("feature");
        let local = Repository::init_opts(&local_path, &init).expect("local repository");
        local
            .remote("origin", &remote_path.display().to_string())
            .expect("origin");
        let branch = BranchName::from("feature");

        let initial = test_commit(&local, "initial");
        push_repository(
            &RepositoryPath::from(local_path.display().to_string()),
            &branch,
        )
        .expect("initial push");
        local
            .reference("refs/remotes/origin/feature", initial, true, "tracking")
            .expect("tracking ref");

        let remote_update = test_commit(&local, "remote update");
        push_repository(
            &RepositoryPath::from(local_path.display().to_string()),
            &branch,
        )
        .expect("remote update push");
        local
            .reference(
                "refs/remotes/origin/feature",
                initial,
                true,
                "simulate stale tracking",
            )
            .expect("stale tracking ref");
        let initial_object = local.find_object(initial, None).expect("initial object");
        local
            .reset(&initial_object, ResetType::Hard, None)
            .expect("reset to initial");
        let rewritten = test_commit(&local, "rewritten");

        let error = push_repository_force_with_lease(
            &RepositoryPath::from(local_path.display().to_string()),
            &branch,
        )
        .expect_err("stale lease must reject force push");
        assert!(error.to_string().contains("remote branch changed"));

        local
            .reference(
                "refs/remotes/origin/feature",
                remote_update,
                true,
                "refresh tracking",
            )
            .expect("updated tracking ref");
        push_repository_force_with_lease(
            &RepositoryPath::from(local_path.display().to_string()),
            &branch,
        )
        .expect("current lease should allow force push");
        assert_eq!(
            remote
                .find_reference("refs/heads/feature")
                .expect("remote feature")
                .target(),
            Some(rewritten)
        );
    }

    #[test]
    fn commit_messages_in_range_reads_the_requested_repository() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repository_path = temp.path().join("repository");
        let nested_path = repository_path.join("nested");
        let repository = Repository::init(&repository_path).expect("repository");
        fs::create_dir(&nested_path).expect("nested directory");
        let from = test_commit(&repository, "initial #10");
        let to = test_commit(&repository, "change #42");
        let range = GitRevisionRange::new(
            GitRevision::from(from.to_string()),
            GitRevision::from(to.to_string()),
        );

        let messages = commit_messages_in_range_at(
            &RepositoryPath::from(nested_path.display().to_string()),
            &range,
        )
        .expect("messages");

        assert!(messages.as_str().contains("change #42"));
        assert!(!messages.as_str().contains("initial #10"));
    }

    fn test_commit(repository: &Repository, contents: &str) -> Oid {
        let workdir = repository.workdir().expect("workdir");
        fs::write(workdir.join("content.txt"), contents).expect("write content");
        let mut index = repository.index().expect("index");
        index
            .add_path(Path::new("content.txt"))
            .expect("add content");
        let tree_id = index.write_tree().expect("write tree");
        let tree = repository.find_tree(tree_id).expect("tree");
        let signature = Signature::now("dw test", "dw@example.test").expect("signature");
        let parent = repository
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok());
        let parents = parent.iter().collect::<Vec<_>>();
        repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                contents,
                &tree,
                &parents,
            )
            .expect("commit")
    }

    #[test]
    fn normalize_slug_caps_length_after_cleanup() {
        assert_eq!(
            normalize_slug(
                "[Acme Portal] - show successful request completion with an excessively long title"
            ),
            "acme-portal-show-successful-request-completion-wit"
        );
    }

    #[test]
    fn from_phrase_or_fallback_uses_fallback_when_phrase_becomes_empty() {
        assert_eq!(
            slug_from_phrase_or_fallback(Some("!!!"), "work item 113"),
            TaskSlug::from("work-item-113")
        );
    }

    #[test]
    fn remote_auth_uses_environment_credential_for_ado_without_explicit_credential() {
        assert!(should_use_environment_credential(
            "https://dev.azure.com/org/project/_git/front",
            false
        ));
        assert!(should_use_environment_credential(
            "https://org.visualstudio.com/project/_git/front",
            false
        ));
        assert!(!should_use_environment_credential(
            "https://dev.azure.com/org/project/_git/front",
            true
        ));
        assert!(!should_use_environment_credential(
            "https://github.com/example/front.git",
            false
        ));
    }

    #[test]
    fn build_uses_work_item_and_task_when_task_exists() {
        assert_eq!(
            build_branch_name(
                &WorkItemTypeName::from("feat"),
                &[WorkItemId::from("102"), WorkItemId::from("109")],
                &TaskSlug::from("example description")
            ),
            BranchName::from("feat/102-109-example-description")
        );
    }

    #[test]
    fn build_omits_task_when_absent() {
        assert_eq!(
            build_branch_name(
                &WorkItemTypeName::from("bug"),
                &[WorkItemId::from("103")],
                &TaskSlug::from("search dialog")
            ),
            BranchName::from("bug/103-search-dialog")
        );
    }

    #[test]
    fn normalize_git_remote_url_accepts_azure_scp_like_ssh_urls() {
        assert_eq!(
            normalize_git_remote_url(&GitRemoteUrl::from(
                "git@ssh.dev.azure.com:v3/org/project/repository"
            )),
            GitRemoteUrl::from("ssh://git@ssh.dev.azure.com/v3/org/project/repository")
        );
        assert_eq!(
            normalize_git_remote_url(&GitRemoteUrl::from(
                "https://dev.azure.com/org/project/_git/repo"
            )),
            GitRemoteUrl::from("https://dev.azure.com/org/project/_git/repo")
        );
    }

    #[test]
    fn build_subject_name_uses_folder_format() {
        assert_eq!(
            build_subject_name(
                &WorkItemTypeName::from("fix"),
                &[WorkItemId::from("42")],
                &TaskSlug::from("update example number")
            ),
            TaskSubjectName::from("fix-42-update-example-number")
        );
    }

    #[test]
    fn build_uses_all_work_item_ids() {
        assert_eq!(
            build_branch_name(
                &WorkItemTypeName::from("feat"),
                &[
                    WorkItemId::from("101"),
                    WorkItemId::from("111"),
                    WorkItemId::from("112"),
                ],
                &TaskSlug::from("example description")
            ),
            BranchName::from("feat/101-111-112-example-description")
        );
    }

    #[test]
    fn resolve_remote_source_branch_returns_origin_default_branch() {
        assert_eq!(
            resolve_remote_source_branch(&BranchName::from("develop")),
            GitReferenceName::from("origin/develop")
        );
    }
}
