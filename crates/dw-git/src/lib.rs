use anyhow::{Result, anyhow};
use dw_core::{GitRevision, RepositoryPath, SecretValue};
use git2::{
    Cred, FetchOptions, IndexAddOption, ObjectType, PushOptions, RebaseOptions, RemoteCallbacks,
    Repository, Sort, StashFlags, StatusOptions, WorktreeAddOptions, build::RepoBuilder,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use thiserror::Error;

const ENV_DW_ADO_TOKEN: &str = "DW_ADO_TOKEN";
const ENV_AZURE_DEVOPS_EXT_PAT: &str = "AZURE_DEVOPS_EXT_PAT";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitRewriteNote {
    pub strategy: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryStatus {
    pub path: String,
    #[serde(rename = "isGitRepository")]
    pub is_git_repository: bool,
    #[serde(rename = "hasChanges")]
    pub has_changes: bool,
    #[serde(rename = "hasUnpushed")]
    pub has_unpushed: bool,
    pub detail: String,
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
    pub project_root: String,
    pub repository: String,
    pub url: String,
    #[serde(rename = "defaultBranch")]
    pub default_branch: String,
    #[serde(rename = "anchorName")]
    pub anchor_name: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
    #[serde(rename = "worktreePath")]
    pub worktree_path: String,
    #[serde(skip)]
    pub credential: Option<GitCredential>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreePrepareResult {
    pub repository: String,
    pub status: String,
    pub message: String,
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
    pub repository_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
                "Configurer gitCredentialSecret dans projects.json puis stocker le PAT avec dw secret set <key>, ou définir DW_ADO_TOKEN/AZURE_DEVOPS_EXT_PAT, ou configurer un credential helper Git non interactif",
            ),
            Self::VerifyHttpsCredential => formatter.write_str(
                "Vérifier que le PAT Git a accès au repository et qu'il n'est pas expiré",
            ),
            Self::TrustSshHostKey => formatter.write_str(
                "Précharger l'empreinte SSH hors de dw, par exemple avec ssh-keyscan/known_hosts ou une connexion ssh manuelle validée",
            ),
            Self::ConfigureSshKey => formatter.write_str(
                "Charger une clé SSH valide dans l'agent ou configurer l'accès repository avant de relancer dw",
            ),
        }
    }
}

impl fmt::Display for GitAuthFailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpsCredentialMissing => formatter.write_str("credential HTTPS Git manquant"),
            Self::HttpsCredentialRejected => formatter.write_str("credential HTTPS Git refusé"),
            Self::SshHostKeyMissing => formatter.write_str("empreinte SSH inconnue"),
            Self::SshKeyUnavailable => formatter.write_str("clé SSH indisponible ou refusée"),
        }
    }
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("{kind}. {remediation}. Détail Git: {detail}")]
    Authentication {
        kind: GitAuthFailureKind,
        remediation: GitAuthRemediation,
        detail: GitErrorDetail,
        invocation: GitOperationInvocation,
    },
    #[error("Git {operation:?} a échoué: {detail}")]
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

    output.trim_matches('-').to_string()
}

pub fn slug_from_phrase_or_fallback(value: Option<&str>, fallback: &str) -> String {
    let normalized = normalize_slug(value.unwrap_or_default());
    if normalized.is_empty() {
        normalize_slug(fallback)
    } else {
        normalized
    }
}

pub fn build_branch_name(type_name: &str, work_item_ids: &[String], slug: &str) -> String {
    let clean_type = if type_name.trim().is_empty() {
        "feat"
    } else {
        type_name.trim()
    }
    .to_ascii_lowercase();
    let id_part = distinct_non_empty(work_item_ids).join("-");
    format!("{clean_type}/{id_part}-{}", normalize_slug(slug))
}

pub fn build_subject_name(type_name: &str, work_item_ids: &[String], slug: &str) -> String {
    let clean_type = if type_name.trim().is_empty() {
        "feat"
    } else {
        type_name.trim()
    }
    .to_ascii_lowercase();
    let id_part = distinct_non_empty(work_item_ids).join("-");
    format!("{clean_type}-{id_part}-{}", normalize_slug(slug))
}

pub fn resolve_remote_source_branch(default_branch: &str) -> String {
    format!("origin/{default_branch}")
}

pub fn update_repository(
    repository_path: &str,
    default_branch: &str,
    credential: Option<&GitCredential>,
) -> Result<()> {
    let mut repository = Repository::open(repository_path).map_err(git2_command_error)?;
    let has_changes = repository_has_changes(&repository)?;
    let mut stashed = false;

    if has_changes {
        let signature = repository.signature().map_err(git2_command_error)?;
        repository
            .stash_save(
                &signature,
                "dw task repo-latest autostash",
                Some(StashFlags::INCLUDE_UNTRACKED),
            )
            .map_err(git2_command_error)?;
        stashed = true;
    }

    fetch_anchor_repository(&repository, credential)?;
    let source_branch = resolve_remote_source_branch(default_branch);
    rebase_current_branch(&repository, &source_branch).map_err(|error| {
        anyhow!(
            "Conflit de rebase. Relancer manuellement avec: git -C \"{}\" fetch --prune origin puis git -C \"{}\" rebase {}. Cause: {}",
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

pub fn repository_status(repository_path: &str) -> RepositoryStatus {
    if !Path::new(repository_path).is_dir() {
        return RepositoryStatus {
            path: repository_path.into(),
            is_git_repository: false,
            has_changes: false,
            has_unpushed: false,
            detail: "Dossier absent.".into(),
        };
    }

    let repository = match Repository::open(repository_path) {
        Ok(repository) => repository,
        Err(error) => {
            return RepositoryStatus {
                path: repository_path.into(),
                is_git_repository: false,
                has_changes: false,
                has_unpushed: false,
                detail: error.to_string(),
            };
        }
    };
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = match repository.statuses(Some(&mut options)) {
        Ok(statuses) => statuses,
        Err(error) => {
            return RepositoryStatus {
                path: repository_path.into(),
                is_git_repository: false,
                has_changes: false,
                has_unpushed: false,
                detail: error.message().into(),
            };
        }
    };
    let status = statuses
        .iter()
        .filter_map(|entry| entry.path().ok().map(ToOwned::to_owned))
        .collect::<Vec<_>>()
        .join("\n");

    if !status.is_empty() {
        return RepositoryStatus {
            path: repository_path.into(),
            is_git_repository: true,
            has_changes: true,
            has_unpushed: false,
            detail: status,
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
        path: repository_path.into(),
        is_git_repository: true,
        has_changes: false,
        has_unpushed: ahead > 0,
        detail: if ahead > 0 {
            format!("{ahead} commit(s) non pousse(s).")
        } else {
            String::new()
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
    let repository = Repository::discover(".").map_err(git2_command_error)?;
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

pub fn commit_repository(repository_path: &str, message: &str) -> Result<()> {
    let repository = Repository::open(repository_path).map_err(git2_command_error)?;
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
            message,
            &tree,
            &parents,
        )
        .map_err(git2_command_error)?;
    Ok(())
}

pub fn push_repository(repository_path: &str, branch_name: &str) -> Result<()> {
    let repository = Repository::open(repository_path).map_err(git2_command_error)?;
    let mut remote = repository
        .find_remote("origin")
        .map_err(git2_command_error)?;
    let environment_credential = remote
        .url()
        .ok()
        .filter(|url| is_azure_devops_url(url))
        .and_then(|_| git_credential_from_environment());
    let callbacks = remote_callbacks(environment_credential.as_ref());
    let mut options = PushOptions::new();
    options.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");
    remote
        .push(&[refspec.as_str()], Some(&mut options))
        .map_err(|error| git2_auth_error(error, environment_credential.is_some()))?;
    Ok(())
}

pub fn prepare_worktree(request: &WorktreePrepareRequest) -> Result<WorktreePrepareResult> {
    if request.url.trim().is_empty() {
        std::fs::create_dir_all(&request.worktree_path)?;
        return Ok(WorktreePrepareResult {
            repository: request.repository.clone(),
            status: "placeholder".into(),
            message: "URL distante absente dans projects.json.".into(),
        });
    }

    let repositories_root = Path::new(&request.project_root).join("repositories");
    let anchor_path = repositories_root.join(&request.anchor_name);
    std::fs::create_dir_all(&repositories_root)?;
    let environment_credential =
        if request.credential.is_none() && is_azure_devops_url(&request.url) {
            git_credential_from_environment()
        } else {
            None
        };
    let credential = request
        .credential
        .as_ref()
        .or(environment_credential.as_ref());

    let anchor_repository = if !anchor_path.is_dir() {
        clone_bare_repository(&request.url, &anchor_path, credential)?
    } else {
        Repository::open_bare(&anchor_path).map_err(git2_command_error)?
    };
    configure_anchor_fetch_refspec(&anchor_repository)?;
    fetch_anchor_repository(&anchor_repository, credential)?;

    if Path::new(&request.worktree_path).is_dir() {
        return Ok(WorktreePrepareResult {
            repository: request.repository.clone(),
            status: "prepared".into(),
            message: "Worktree déjà présent.".into(),
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
            "Branche de base introuvable: {}. Références testées: origin/{}, refs/heads/{}.",
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
                    .map_err(|_| git2::Error::from_str("référence de base sans commit"))
            })
            .map_err(git2_command_error)?;
        anchor_repository
            .branch(&request.branch_name, &base_commit, false)
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
            Path::new(&request.worktree_path),
            Some(&options),
        )
        .map_err(git2_command_error)?;

    Ok(WorktreePrepareResult {
        repository: request.repository.clone(),
        status: "prepared".into(),
        message: if branch_exists {
            format!(
                "Worktree créé depuis la branche existante {}.",
                request.branch_name
            )
        } else {
            format!("Worktree créé depuis {base_ref}.")
        },
    })
}

fn clone_bare_repository(
    url: &str,
    anchor_path: &Path,
    credential: Option<&GitCredential>,
) -> std::result::Result<Repository, GitError> {
    let fetch_options = fetch_options(credential);
    let mut builder = RepoBuilder::new();
    builder.bare(true).fetch_options(fetch_options);
    builder
        .clone(url, anchor_path)
        .map_err(|error| git2_auth_error(error, credential.is_some()))
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
) -> std::result::Result<(), GitError> {
    let mut remote = repository
        .find_remote("origin")
        .map_err(git2_command_error)?;
    let mut fetch_options = fetch_options(credential);
    remote
        .fetch(
            &["+refs/heads/*:refs/remotes/origin/*"],
            Some(&mut fetch_options),
            None,
        )
        .map_err(|error| git2_auth_error(error, credential.is_some()))
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
    upstream_ref: &str,
) -> std::result::Result<(), GitError> {
    let upstream = repository
        .revparse_single(upstream_ref)
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
                detail: GitErrorDetail::new("Conflit de rebase"),
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

fn fetch_options(credential: Option<&GitCredential>) -> FetchOptions<'_> {
    let callbacks = remote_callbacks(credential);
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    fetch_options
}

fn remote_callbacks(credential: Option<&GitCredential>) -> RemoteCallbacks<'_> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(move |_url, username_from_url, _allowed_types| {
        if let Some(credential) = credential {
            return Cred::userpass_plaintext(
                username_from_url.unwrap_or("dw"),
                credential.token().as_str(),
            );
        }
        if let Some(username) = username_from_url {
            return Cred::ssh_key_from_agent(username);
        }
        Err(git2::Error::from_str("credential HTTPS Git manquant"))
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

pub fn worktree_remove(git_dir: &str, worktree_path: &str) -> Result<()> {
    let repository = Repository::open_bare(git_dir).map_err(git2_command_error)?;
    if Path::new(worktree_path).exists() {
        std::fs::remove_dir_all(worktree_path)?;
    }
    if let Some(name) = find_worktree_name_by_path(&repository, Path::new(worktree_path))? {
        let worktree = repository
            .find_worktree(&name)
            .map_err(git2_command_error)?;
        worktree.prune(None).map_err(git2_command_error)?;
    }
    Ok(())
}

pub fn worktree_prune(git_dir: &str) -> Result<()> {
    let repository = Repository::open_bare(git_dir).map_err(git2_command_error)?;
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
        || normalized.contains("credential https git manquant")
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

fn distinct_non_empty(values: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if value.trim().is_empty() {
            continue;
        }

        if !result
            .iter()
            .any(|item: &String| item.eq_ignore_ascii_case(value))
        {
            result.push(value.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_creates_ascii_dash_slug() {
        assert_eq!(normalize_slug("descriptif cours"), "descriptif-cours");
        assert_eq!(
            normalize_slug("heures PSFs côté pré-réservation"),
            "heures-psfs-cote-pre-reservation"
        );
        assert_eq!(normalize_slug("  Trop   d'espaces !!! "), "trop-d-espaces");
        assert_eq!(
            normalize_slug("ceci est un Test hehe"),
            "ceci-est-un-test-hehe"
        );
    }

    #[test]
    fn from_phrase_or_fallback_uses_fallback_when_phrase_becomes_empty() {
        assert_eq!(
            slug_from_phrase_or_fallback(Some("!!!"), "work item 55222"),
            "work-item-55222"
        );
    }

    #[test]
    fn build_uses_work_item_and_task_when_task_exists() {
        assert_eq!(
            build_branch_name(
                "feat",
                &["27485".into(), "55201".into()],
                "descriptif cours"
            ),
            "feat/27485-55201-descriptif-cours"
        );
    }

    #[test]
    fn build_omits_task_when_absent() {
        assert_eq!(
            build_branch_name("bug", &["53020".into()], "ouverture dossier recherche"),
            "bug/53020-ouverture-dossier-recherche"
        );
    }

    #[test]
    fn build_subject_name_uses_folder_format() {
        assert_eq!(
            build_subject_name("fix", &["53635".into()], "reprendre numéro HE"),
            "fix-53635-reprendre-numero-he"
        );
    }

    #[test]
    fn build_uses_all_work_item_ids() {
        assert_eq!(
            build_branch_name(
                "feat",
                &["11010".into(), "55206".into(), "55207".into()],
                "descriptif cours"
            ),
            "feat/11010-55206-55207-descriptif-cours"
        );
    }

    #[test]
    fn resolve_remote_source_branch_returns_origin_default_branch() {
        assert_eq!(resolve_remote_source_branch("develop"), "origin/develop");
    }
}
