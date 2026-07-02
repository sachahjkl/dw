use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitRewriteNote {
    pub strategy: &'static str,
    pub status: &'static str,
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
