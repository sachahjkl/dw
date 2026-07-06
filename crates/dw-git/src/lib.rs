use anyhow::{Result, anyhow};
use base64::Engine;
use dw_core::SecretValue;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fmt;
use std::path::Path;
use std::process::{Command, Output};
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

    fn authorization_header(&self) -> String {
        let value =
            base64::engine::general_purpose::STANDARD.encode(format!(":{}", self.token.as_str()));
        format!("Authorization: Basic {value}")
    }
}

impl fmt::Debug for GitCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitCredential(***)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandInvocation {
    pub cwd: Option<String>,
    pub git_dir: Option<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandStderr(String);

impl GitCommandStderr {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GitCommandStderr {
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
    #[error("Git introuvable: {0}")]
    MissingGitExecutable(String),
    #[error("{kind}. {remediation}. Détail Git: {stderr}")]
    Authentication {
        kind: GitAuthFailureKind,
        remediation: GitAuthRemediation,
        stderr: GitCommandStderr,
        invocation: GitCommandInvocation,
    },
    #[error("Git a échoué: {stderr}")]
    CommandFailed {
        stderr: GitCommandStderr,
        invocation: GitCommandInvocation,
    },
}

pub fn current_strategy() -> GitRewriteNote {
    GitRewriteNote {
        strategy: "shell-out-to-git",
        status: "planned",
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
    let status = run_git(repository_path, &["status", "--short"], credential)?;
    let has_changes = !status.trim().is_empty();
    let mut stashed = false;

    if has_changes {
        run_git(
            repository_path,
            &[
                "stash",
                "push",
                "--include-untracked",
                "-m",
                "dw task repo-latest autostash",
            ],
            credential,
        )?;
        stashed = true;
    }

    run_git(repository_path, &["fetch", "--prune", "origin"], credential)?;
    let source_branch = resolve_remote_source_branch(default_branch);
    if let Err(error) = run_git(repository_path, &["rebase", &source_branch], credential) {
        let _ = run_git(repository_path, &["rebase", "--abort"], credential);
        return Err(anyhow!(
            "Conflit de rebase. Relancer manuellement avec: git -C \"{}\" fetch --prune origin puis git -C \"{}\" rebase {}. Cause: {}",
            repository_path,
            repository_path,
            source_branch,
            error
        ));
    }

    if stashed {
        run_git(repository_path, &["stash", "pop"], credential)?;
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

    let status = match run_git(repository_path, &["status", "--short"], None) {
        Ok(output) => output.trim().to_string(),
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

    if !status.is_empty() {
        return RepositoryStatus {
            path: repository_path.into(),
            is_git_repository: true,
            has_changes: true,
            has_unpushed: false,
            detail: status,
        };
    }

    let ahead = run_git(
        repository_path,
        &["rev-list", "--count", "@{u}..HEAD"],
        None,
    )
    .ok()
    .and_then(|output| output.trim().parse::<u32>().ok())
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

pub fn commit_repository(repository_path: &str, message: &str) -> Result<()> {
    run_git(repository_path, &["add", "."], None)?;
    run_git(repository_path, &["commit", "-m", message], None)?;
    Ok(())
}

pub fn push_repository(repository_path: &str, branch_name: &str) -> Result<()> {
    run_git(
        repository_path,
        &["push", "-u", "origin", branch_name],
        None,
    )?;
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

    if !anchor_path.is_dir() {
        run_git_in(
            &request.project_root,
            &[
                "clone",
                "--bare",
                &request.url,
                anchor_path.to_str().unwrap_or_default(),
            ],
            credential,
        )?;
        run_git_dir(
            anchor_path.to_str().unwrap_or_default(),
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
            credential,
        )?;
    } else {
        run_git_dir(
            anchor_path.to_str().unwrap_or_default(),
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
            credential,
        )?;
        run_git_dir(
            anchor_path.to_str().unwrap_or_default(),
            &["fetch", "--prune", "origin"],
            credential,
        )?;
    }

    if Path::new(&request.worktree_path).is_dir() {
        return Ok(WorktreePrepareResult {
            repository: request.repository.clone(),
            status: "prepared".into(),
            message: "Worktree déjà présent.".into(),
        });
    }

    let anchor = anchor_path.to_str().unwrap_or_default();
    let base_ref = [
        format!("origin/{}", request.default_branch),
        format!("refs/heads/{}", request.default_branch),
    ]
    .into_iter()
    .find(|candidate| {
        run_git_dir(anchor, &["rev-parse", "--verify", candidate], credential).is_ok()
    })
    .ok_or_else(|| {
        anyhow!(
            "Branche de base introuvable: {}. Références testées: origin/{}, refs/heads/{}.",
            request.default_branch,
            request.default_branch,
            request.default_branch
        )
    })?;
    let branch_ref = format!("refs/heads/{}", request.branch_name);
    let branch_exists =
        run_git_dir(anchor, &["rev-parse", "--verify", &branch_ref], credential).is_ok();
    if branch_exists {
        run_git_dir(
            anchor,
            &[
                "worktree",
                "add",
                &request.worktree_path,
                &request.branch_name,
            ],
            credential,
        )?;
    } else {
        run_git_dir(
            anchor,
            &[
                "worktree",
                "add",
                "-b",
                &request.branch_name,
                &request.worktree_path,
                &base_ref,
            ],
            credential,
        )?;
    }

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

pub fn worktree_remove(git_dir: &str, worktree_path: &str) -> Result<()> {
    run_git_dir(
        git_dir,
        &["worktree", "remove", "--force", worktree_path],
        None,
    )?;
    Ok(())
}

pub fn worktree_prune(git_dir: &str) -> Result<()> {
    run_git_dir(git_dir, &["worktree", "prune"], None)?;
    Ok(())
}

fn run_git(
    repository_path: &str,
    args: &[&str],
    credential: Option<&GitCredential>,
) -> std::result::Result<String, GitError> {
    let mut command = Command::new("git");
    command.args(args).current_dir(repository_path);
    run_git_command(
        command,
        GitCommandInvocation {
            cwd: Some(repository_path.into()),
            git_dir: None,
            args: args_to_strings(args),
        },
        credential,
    )
}

fn run_git_in(
    working_directory: &str,
    args: &[&str],
    credential: Option<&GitCredential>,
) -> std::result::Result<String, GitError> {
    let mut command = Command::new("git");
    command.args(args).current_dir(working_directory);
    run_git_command(
        command,
        GitCommandInvocation {
            cwd: Some(working_directory.into()),
            git_dir: None,
            args: args_to_strings(args),
        },
        credential,
    )
}

fn run_git_dir(
    git_dir: &str,
    args: &[&str],
    credential: Option<&GitCredential>,
) -> std::result::Result<String, GitError> {
    let mut command = Command::new("git");
    command.arg("--git-dir").arg(git_dir).args(args);
    run_git_command(
        command,
        GitCommandInvocation {
            cwd: None,
            git_dir: Some(git_dir.into()),
            args: args_to_strings(args),
        },
        credential,
    )
}

fn run_git_command(
    mut command: Command,
    invocation: GitCommandInvocation,
    credential: Option<&GitCredential>,
) -> std::result::Result<String, GitError> {
    configure_non_interactive_git(&mut command, credential);
    let credential_was_available = credential.is_some();
    let output = command
        .output()
        .map_err(|error| GitError::MissingGitExecutable(error.to_string()))?;
    if !output.status.success() {
        return Err(git_command_error(
            output,
            invocation,
            credential_was_available,
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn is_azure_devops_url(url: &str) -> bool {
    let normalized = url.to_ascii_lowercase();
    normalized.contains("dev.azure.com") || normalized.contains("visualstudio.com")
}

fn configure_non_interactive_git(command: &mut Command, credential: Option<&GitCredential>) {
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes",
        );

    if let Some(credential) = credential {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "http.extraHeader")
            .env("GIT_CONFIG_VALUE_0", credential.authorization_header());
    }
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

fn git_command_error(
    output: Output,
    invocation: GitCommandInvocation,
    credential_was_available: bool,
) -> GitError {
    let stderr = command_stderr(output);
    if let Some(kind) = classify_auth_failure(stderr.as_str(), credential_was_available) {
        return GitError::Authentication {
            kind,
            remediation: auth_remediation(kind),
            stderr,
            invocation,
        };
    }
    GitError::CommandFailed { stderr, invocation }
}

fn command_stderr(output: Output) -> GitCommandStderr {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        GitCommandStderr::new(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        GitCommandStderr::new(stderr)
    }
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

fn args_to_strings(args: &[&str]) -> Vec<String> {
    args.iter().map(OsStr::new).map(os_to_string).collect()
}

fn os_to_string(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
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
