use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorktreePrepareResult {
    pub repository: String,
    pub status: String,
    pub message: String,
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

pub fn update_repository(repository_path: &str, default_branch: &str) -> Result<()> {
    let status = run_git(repository_path, &["status", "--short"])?;
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
        )?;
        stashed = true;
    }

    run_git(repository_path, &["fetch", "--prune", "origin"])?;
    let source_branch = resolve_remote_source_branch(default_branch);
    if let Err(error) = run_git(repository_path, &["rebase", &source_branch]) {
        let _ = run_git(repository_path, &["rebase", "--abort"]);
        return Err(anyhow!(
            "Conflit de rebase. Relancer manuellement avec: git -C \"{}\" fetch --prune origin puis git -C \"{}\" rebase {}. Cause: {}",
            repository_path,
            repository_path,
            source_branch,
            error
        ));
    }

    if stashed {
        run_git(repository_path, &["stash", "pop"])?;
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

    let status = match run_git(repository_path, &["status", "--short"]) {
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

    let ahead = run_git(repository_path, &["rev-list", "--count", "@{u}..HEAD"])
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
    run_git(repository_path, &["add", "."])?;
    run_git(repository_path, &["commit", "-m", message])?;
    Ok(())
}

pub fn push_repository(repository_path: &str, branch_name: &str) -> Result<()> {
    run_git(repository_path, &["push", "-u", "origin", branch_name])?;
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

    if !anchor_path.is_dir() {
        run_git_in(
            &request.project_root,
            &[
                "clone",
                "--bare",
                &request.url,
                anchor_path.to_str().unwrap_or_default(),
            ],
        )?;
        run_git_dir(
            anchor_path.to_str().unwrap_or_default(),
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        )?;
    } else {
        run_git_dir(
            anchor_path.to_str().unwrap_or_default(),
            &[
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        )?;
        run_git_dir(
            anchor_path.to_str().unwrap_or_default(),
            &["fetch", "--prune", "origin"],
        )?;
    }

    if Path::new(&request.worktree_path).is_dir() {
        return Ok(WorktreePrepareResult {
            repository: request.repository.clone(),
            status: "prepared".into(),
            message: "Worktree deja present.".into(),
        });
    }

    let anchor = anchor_path.to_str().unwrap_or_default();
    let base_ref = [
        format!("origin/{}", request.default_branch),
        format!("refs/heads/{}", request.default_branch),
    ]
    .into_iter()
    .find(|candidate| run_git_dir(anchor, &["rev-parse", "--verify", candidate]).is_ok())
    .ok_or_else(|| {
        anyhow!(
            "Branche de base introuvable: {}. References testees: origin/{}, refs/heads/{}.",
            request.default_branch,
            request.default_branch,
            request.default_branch
        )
    })?;
    let branch_ref = format!("refs/heads/{}", request.branch_name);
    let branch_exists = run_git_dir(anchor, &["rev-parse", "--verify", &branch_ref]).is_ok();
    if branch_exists {
        run_git_dir(
            anchor,
            &[
                "worktree",
                "add",
                &request.worktree_path,
                &request.branch_name,
            ],
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
        )?;
    }

    Ok(WorktreePrepareResult {
        repository: request.repository.clone(),
        status: "prepared".into(),
        message: if branch_exists {
            format!(
                "Worktree cree depuis la branche existante {}.",
                request.branch_name
            )
        } else {
            format!("Worktree cree depuis {base_ref}.")
        },
    })
}

pub fn worktree_remove(git_dir: &str, worktree_path: &str) -> Result<()> {
    run_git_dir(git_dir, &["worktree", "remove", "--force", worktree_path]).map(|_| ())
}

pub fn worktree_prune(git_dir: &str) -> Result<()> {
    run_git_dir(git_dir, &["worktree", "prune"]).map(|_| ())
}

fn run_git(repository_path: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repository_path)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_in(working_directory: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_directory)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_dir(git_dir: &str, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(git_dir)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
