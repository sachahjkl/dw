use dw_config::{ProjectConfig, RepositoryConfig, WorkflowConfig, repository_config};
use dw_core::WorkspaceRepositoryName;
use dw_git::RepositoryStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

use crate::{TaskCommitTarget, WorkspaceHandoffSummary, WorkspaceManifest, try_parse_summary};

#[derive(Debug, Error)]
pub enum TaskFinishError {
    #[error("task finish bloqué: vérification échouée.")]
    VerificationFailed,
    #[error("Handoff manquant pour {repository}: {path}")]
    MissingHandoff { repository: String, path: String },
    #[error("Handoff invalide pour {repository}: {error}. Fichier: {path}")]
    InvalidHandoff {
        repository: String,
        error: String,
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub repository: String,
    pub command: String,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    #[serde(rename = "standardOutput")]
    pub standard_output: String,
    #[serde(rename = "standardError")]
    pub standard_error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestCandidate {
    pub repository: String,
    pub path: String,
    pub ado_repository: Option<String>,
    pub target_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFinishOptions {
    pub run_verification: bool,
    pub update_work_item_state: bool,
    pub bug_state: String,
    pub task_state: String,
    pub verification_commands: BTreeMap<String, Vec<String>>,
}

impl Default for TaskFinishOptions {
    fn default() -> Self {
        Self {
            run_verification: true,
            update_work_item_state: true,
            bug_state: "PR en attente".into(),
            task_state: "PR en attente".into(),
            verification_commands: BTreeMap::new(),
        }
    }
}

pub fn task_finish_options(workflow: &WorkflowConfig) -> TaskFinishOptions {
    let Some(value) = workflow.task_finish.as_ref() else {
        return TaskFinishOptions::default();
    };

    let mut options = TaskFinishOptions::default();
    options.run_verification = bool_property(value, "runVerification", options.run_verification);
    options.update_work_item_state =
        bool_property(value, "updateWorkItemState", options.update_work_item_state);
    options.bug_state = string_property(value, "bugState").unwrap_or(options.bug_state);
    options.task_state = string_property(value, "taskState").unwrap_or(options.task_state);
    options.verification_commands = verification_commands(value);
    options
}

pub fn finish_state(work_item_type: Option<&str>, options: &TaskFinishOptions) -> Option<String> {
    match normalize_work_item_type(work_item_type).as_str() {
        "bug" => Some(options.bug_state.clone()),
        "task" | "tache" | "tâche" => Some(options.task_state.clone()),
        _ => None,
    }
    .filter(|state| !state.trim().is_empty())
}

pub fn select_pull_request_candidates(
    statuses: &[(&TaskCommitTarget, RepositoryStatus)],
    actionable_repositories: &[WorkspaceRepositoryName],
    project_config: Option<&ProjectConfig>,
) -> Vec<PullRequestCandidate> {
    let actionable = actionable_repositories
        .iter()
        .filter_map(|repository| candidate_for(statuses, repository.as_str(), project_config))
        .collect::<Vec<_>>();
    if !actionable.is_empty() {
        return actionable;
    }

    statuses
        .iter()
        .filter(|(_, status)| status.is_git_repository)
        .filter(|(target, _)| {
            has_reviewable_commits(
                target.path.as_str(),
                project_config,
                target.repository.as_str(),
            )
        })
        .filter_map(|(target, _)| candidate_from_target(target, project_config))
        .collect()
}

pub fn run_verification(
    options: &TaskFinishOptions,
    candidates: &[PullRequestCandidate],
) -> Vec<VerificationResult> {
    if options.verification_commands.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for candidate in candidates {
        let Some(commands) = options.verification_commands.get(&candidate.repository) else {
            continue;
        };
        for command in commands {
            let resolved = resolve_node_package_manager_command(command);
            let output = run_shell(&candidate.path, &resolved);
            results.push(VerificationResult {
                repository: candidate.repository.clone(),
                command: resolved,
                exit_code: output
                    .as_ref()
                    .ok()
                    .and_then(|output| output.status.code())
                    .unwrap_or(1),
                standard_output: output
                    .as_ref()
                    .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
                    .unwrap_or_default(),
                standard_error: output
                    .as_ref()
                    .map(|output| String::from_utf8_lossy(&output.stderr).to_string())
                    .unwrap_or_else(|error| error.to_string()),
            });
        }
    }
    results
}

pub fn ensure_verification_passed(results: &[VerificationResult]) -> Result<(), TaskFinishError> {
    if results.iter().all(|result| result.exit_code == 0) {
        return Ok(());
    }
    Err(TaskFinishError::VerificationFailed)
}

pub fn pull_request_title(manifest: &WorkspaceManifest) -> String {
    crate::build_commit_message(manifest, None)
}

pub fn pull_request_description(
    manifest: &WorkspaceManifest,
    candidate: &PullRequestCandidate,
    plan: &str,
    verification_results: &[VerificationResult],
    handoff: &WorkspaceHandoffSummary,
) -> String {
    let verification = render_verification(&candidate.repository, verification_results);
    let plan = if plan.trim().is_empty() {
        "_Plan non trouvé._".to_string()
    } else {
        plan.trim().to_string()
    };
    let handoff = structured_handoff_section(handoff);
    let work_items = manifest
        .all_known_work_item_ids()
        .into_iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "## Résumé\n- Travail réalisé pour `{}`\n- Repository concerné : `{}`\n- Work items : `{}`\n\n## Plan\n{}\n\n## Handoff\n{}\n\n## Vérifications\n{}\n",
        manifest.slug, candidate.repository, work_items, plan, handoff, verification
    )
}

pub fn structured_handoff_section(handoff: &WorkspaceHandoffSummary) -> String {
    format!(
        "### Statut\n- `{}`\n\n### Travail Fait\n{}\n\n### Décisions\n{}\n\n### Risques\n{}\n\n### Blockers\n{}\n\n### Follow-up\n{}\n",
        handoff.status,
        render_list(&handoff.done),
        render_list(&handoff.decisions),
        render_list(&handoff.risks),
        render_list(&handoff.blockers),
        render_list(&handoff.follow_up)
    )
}

pub fn read_plan(workspace: &Path) -> String {
    fs::read_to_string(workspace.join("plan.md")).unwrap_or_default()
}

pub fn read_handoff_summary(
    workspace: &Path,
    repository: &str,
) -> Result<WorkspaceHandoffSummary, TaskFinishError> {
    let path = workspace.join(format!("handoff-{repository}.md"));
    let path_text = path.display().to_string();
    let text = fs::read_to_string(&path).map_err(|_| TaskFinishError::MissingHandoff {
        repository: repository.into(),
        path: path_text.clone(),
    })?;
    try_parse_summary(&text, repository).map_err(|error| TaskFinishError::InvalidHandoff {
        repository: repository.into(),
        error,
        path: path_text,
    })
}

fn candidate_for(
    statuses: &[(&TaskCommitTarget, RepositoryStatus)],
    repository: &str,
    project_config: Option<&ProjectConfig>,
) -> Option<PullRequestCandidate> {
    statuses
        .iter()
        .find(|(target, _)| target.repository.as_str().eq_ignore_ascii_case(repository))
        .and_then(|(target, _)| candidate_from_target(target, project_config))
}

fn candidate_from_target(
    target: &TaskCommitTarget,
    project_config: Option<&ProjectConfig>,
) -> Option<PullRequestCandidate> {
    let repo_config =
        project_config.and_then(|project| repository_config(project, target.repository.as_str()));
    let ado_repository = repo_config
        .as_ref()
        .and_then(|repo| repo.azure_dev_ops_repository.clone())
        .filter(|value| !value.trim().is_empty());
    Some(PullRequestCandidate {
        repository: target.repository.as_str().to_owned(),
        path: target.path.as_str().to_owned(),
        ado_repository,
        target_branch: target_branch(repo_config.as_ref()),
    })
}

fn has_reviewable_commits(
    path: &str,
    project_config: Option<&ProjectConfig>,
    repository: &str,
) -> bool {
    let target = project_config
        .and_then(|project| repository_config(project, repository))
        .as_ref()
        .map(|repo| target_branch(Some(repo)))
        .unwrap_or_else(|| "main".into());
    let comparison = format!("origin/{target}..HEAD");
    let Ok(output) = Command::new("git")
        .args(["rev-list", "--count", &comparison])
        .current_dir(path)
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .is_ok_and(|ahead| ahead > 0)
}

fn target_branch(repo: Option<&RepositoryConfig>) -> String {
    repo.and_then(|repo| repo.pull_request_target_branch.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            repo.map(|repo| repo.default_branch.clone())
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "main".into())
}

fn render_verification(repository: &str, results: &[VerificationResult]) -> String {
    let matching = results
        .iter()
        .filter(|result| result.repository.eq_ignore_ascii_case(repository))
        .map(|result| {
            format!(
                "- `{}`: {}",
                result.command,
                if result.exit_code == 0 { "OK" } else { "KO" }
            )
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        "- Aucune commande configurée dans `taskFinish.verificationCommands`.".into()
    } else {
        matching.join("\n")
    }
}

fn render_list(items: &[String]) -> String {
    if items.is_empty() {
        "- (aucun)".into()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn bool_property(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn string_property(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn verification_commands(value: &Value) -> BTreeMap<String, Vec<String>> {
    value
        .get("verificationCommands")
        .and_then(Value::as_object)
        .map(|commands| {
            commands
                .iter()
                .filter_map(|(repository, value)| {
                    let commands = value
                        .as_array()
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::trim)
                                .filter(|command| !command.is_empty())
                                .map(ToOwned::to_owned)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    (!commands.is_empty()).then(|| (repository.clone(), commands))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_node_package_manager_command(command: &str) -> String {
    let trimmed = command.trim_start();
    if !trimmed.starts_with("npm ") || !command_available("pnpm") {
        return command.into();
    }
    let leading_whitespace = command.len() - trimmed.len();
    format!(
        "{}pnpm{}",
        &command[..leading_whitespace],
        &trimmed["npm".len()..]
    )
}

fn command_available(command: &str) -> bool {
    dw_process::command_available(command, &["--version"])
}

fn run_shell(path: &str, command: &str) -> std::io::Result<std::process::Output> {
    if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ])
            .current_dir(PathBuf::from(path))
            .output()
    } else {
        Command::new("sh")
            .args(["-lc", command])
            .current_dir(PathBuf::from(path))
            .output()
    }
}

fn normalize_work_item_type(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .replace('â', "a")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorkspaceHandoffSummary, WorkspaceManifest};

    #[test]
    fn finish_state_never_moves_parent_story_types() {
        let options = TaskFinishOptions::default();

        assert_eq!(finish_state(Some("User Story"), &options), None);
        assert_eq!(finish_state(Some("Anomalie"), &options), None);
        assert_eq!(
            finish_state(Some("Bug"), &options).as_deref(),
            Some("PR en attente")
        );
        assert_eq!(
            finish_state(Some("Task"), &options).as_deref(),
            Some("PR en attente")
        );
        assert_eq!(
            finish_state(Some("Tâche"), &options).as_deref(),
            Some("PR en attente")
        );
    }

    #[test]
    fn task_finish_options_read_configured_values() {
        let workflow = WorkflowConfig {
            task_finish: Some(serde_json::json!({
                "runVerification": false,
                "updateWorkItemState": false,
                "bugState": "Review",
                "taskState": "Done",
                "verificationCommands": {
                    "front": ["npm test", " "],
                    "back": ["cargo test"]
                }
            })),
            ..WorkflowConfig::default()
        };

        let options = task_finish_options(&workflow);

        assert!(!options.run_verification);
        assert!(!options.update_work_item_state);
        assert_eq!(options.bug_state, "Review");
        assert_eq!(options.task_state, "Done");
        assert_eq!(
            options.verification_commands["front"],
            vec!["npm test".to_string()]
        );
    }

    #[test]
    fn structured_handoff_section_renders_summary_lists() {
        let handoff = WorkspaceHandoffSummary {
            repository: "front".into(),
            status: "done".into(),
            done: vec!["Implémenter le composant".into()],
            decisions: vec!["Conserver le libellé métier".into()],
            risks: vec!["Régression responsive".into()],
            blockers: vec![],
            follow_up: vec!["Valider avec le user".into()],
        };

        let rendered = structured_handoff_section(&handoff);

        assert!(rendered.contains("### Statut"));
        assert!(rendered.contains("`done`"));
        assert!(rendered.contains("Implémenter le composant"));
        assert!(rendered.contains("Conserver le libellé métier"));
        assert!(rendered.contains("Régression responsive"));
        assert!(rendered.contains("Valider avec le user"));
    }

    #[test]
    fn pull_request_description_includes_plan_handoff_and_verification() {
        let manifest = WorkspaceManifest {
            schema: 1,
            work_item_id: "53020".into(),
            task_id: Some("55201".into()),
            project: "ha".into(),
            kind: "bug".into(),
            slug: "corriger-ouverture".into(),
            branch_name: "bug/53020-55201-corriger-ouverture".into(),
            created_at: "2026-07-03T00:00:00Z".into(),
            repositories: vec!["front".into()],
            status: "created".into(),
            work_item_type: Some("Bug".into()),
            work_item_title: None,
            work_item_state: None,
            work_items: None,
            child_task_ids: None,
            child_tasks: None,
        };
        let candidate = PullRequestCandidate {
            repository: "front".into(),
            path: "/tmp/front".into(),
            ado_repository: Some("FrontRepo".into()),
            target_branch: "main".into(),
        };
        let handoff = WorkspaceHandoffSummary {
            repository: "front".into(),
            status: "done".into(),
            done: vec!["Changer la page".into()],
            decisions: vec![],
            risks: vec![],
            blockers: vec![],
            follow_up: vec![],
        };
        let verification = vec![VerificationResult {
            repository: "front".into(),
            command: "pnpm test".into(),
            exit_code: 0,
            standard_output: String::new(),
            standard_error: String::new(),
        }];

        let rendered = pull_request_description(
            &manifest,
            &candidate,
            "## Plan\nFaire le correctif",
            &verification,
            &handoff,
        );

        assert!(rendered.contains("Travail réalisé pour `corriger-ouverture`"));
        assert!(rendered.contains("Work items : `#53020, #55201`"));
        assert!(rendered.contains("Faire le correctif"));
        assert!(rendered.contains("Changer la page"));
        assert!(rendered.contains("`pnpm test`: OK"));
    }
}
