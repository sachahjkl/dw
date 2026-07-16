use dw_config::{ProjectConfig, RepositoryConfig, WorkflowConfig, repository_config};
use dw_core::{
    AdoRepositoryName, BranchName, GitRevision, HandoffFilePath, HandoffParseError, RepositoryPath,
    WorkItemState, WorkspaceRepositoryName,
};
use dw_git::{RepositoryStatus, has_commits_ahead_of};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::{
    HandoffSummaryEntry, TaskCommitTarget, WorkspaceHandoffSummary, WorkspaceManifest,
    try_parse_summary,
};

#[derive(Debug, Error)]
pub enum TaskFinishError {
    #[error("work finish blocked: verification failed.")]
    VerificationFailed,
    #[error("Missing handoff for {repository}: {path}")]
    MissingHandoff {
        repository: WorkspaceRepositoryName,
        path: HandoffFilePath,
    },
    #[error("Invalid handoff for {repository}: {error}. File: {path}")]
    InvalidHandoff {
        repository: WorkspaceRepositoryName,
        error: HandoffParseError,
        path: HandoffFilePath,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub repository: WorkspaceRepositoryName,
    pub command: VerificationCommand,
    #[serde(rename = "exitCode")]
    pub exit_code: VerificationExitCode,
    #[serde(rename = "standardOutput")]
    pub standard_output: VerificationOutputText,
    #[serde(rename = "standardError")]
    pub standard_error: VerificationOutputText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestCandidate {
    pub repository: WorkspaceRepositoryName,
    pub path: RepositoryPath,
    pub ado_repository: Option<AdoRepositoryName>,
    pub target_branch: BranchName,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerificationCommand(String);

impl VerificationCommand {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for VerificationCommand {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for VerificationCommand {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for VerificationCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerificationExitCode(i32);

impl VerificationExitCode {
    pub fn new(value: i32) -> Self {
        Self(value)
    }

    pub fn success(self) -> bool {
        self.0 == 0
    }
}

impl From<i32> for VerificationExitCode {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for VerificationExitCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VerificationOutputText(String);

impl VerificationOutputText {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl From<String> for VerificationOutputText {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for VerificationOutputText {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for VerificationOutputText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationCommandOutput {
    pub exit_code: VerificationExitCode,
    pub standard_output: VerificationOutputText,
    pub standard_error: VerificationOutputText,
}

pub trait VerificationRunner {
    fn run(
        &self,
        repository_path: &RepositoryPath,
        command: &VerificationCommand,
    ) -> VerificationCommandOutput;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ShellVerificationRunner;

impl VerificationRunner for ShellVerificationRunner {
    fn run(
        &self,
        repository_path: &RepositoryPath,
        command: &VerificationCommand,
    ) -> VerificationCommandOutput {
        shell_verification_output(repository_path, command)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFinishOptions {
    pub run_verification: bool,
    pub update_work_item_state: bool,
    pub bug_state: WorkItemState,
    pub task_state: WorkItemState,
    pub verification_commands: BTreeMap<WorkspaceRepositoryName, Vec<VerificationCommand>>,
}

impl Default for TaskFinishOptions {
    fn default() -> Self {
        Self {
            run_verification: true,
            update_work_item_state: true,
            bug_state: WorkItemState::from("PR en attente"),
            task_state: WorkItemState::from("PR en attente"),
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
    options.bug_state = state_property(value, "bugState").unwrap_or(options.bug_state);
    options.task_state = state_property(value, "taskState").unwrap_or(options.task_state);
    options.verification_commands = verification_commands(value);
    options
}

pub fn finish_state(
    work_item_type: Option<&str>,
    options: &TaskFinishOptions,
) -> Option<WorkItemState> {
    match normalize_work_item_type(work_item_type).as_str() {
        "bug" => Some(options.bug_state.clone()),
        "task" | "tache" | "tâche" => Some(options.task_state.clone()),
        _ => None,
    }
}

pub fn select_pull_request_candidates(
    statuses: &[(&TaskCommitTarget, RepositoryStatus)],
    actionable_repositories: &[WorkspaceRepositoryName],
    project_config: Option<&ProjectConfig>,
) -> Vec<PullRequestCandidate> {
    let actionable = actionable_repositories
        .iter()
        .filter_map(|repository| candidate_for(statuses, repository, project_config))
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
    run_verification_with_runner(options, candidates, &ShellVerificationRunner)
}

pub fn run_verification_with_runner(
    options: &TaskFinishOptions,
    candidates: &[PullRequestCandidate],
    runner: &impl VerificationRunner,
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
            let output = runner.run(&candidate.path, &resolved);
            results.push(VerificationResult {
                repository: candidate.repository.clone(),
                command: resolved,
                exit_code: output.exit_code,
                standard_output: output.standard_output,
                standard_error: output.standard_error,
            });
        }
    }
    results
}

pub fn ensure_verification_passed(results: &[VerificationResult]) -> Result<(), TaskFinishError> {
    if results.iter().all(|result| result.exit_code.success()) {
        return Ok(());
    }
    Err(TaskFinishError::VerificationFailed)
}

pub fn pull_request_title(manifest: &WorkspaceManifest) -> String {
    crate::build_commit_message(manifest, None).to_string()
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
    repository: &WorkspaceRepositoryName,
) -> Result<WorkspaceHandoffSummary, TaskFinishError> {
    let path_buf = workspace.join(format!("handoff-{repository}.md"));
    let path = HandoffFilePath::from(path_buf.display().to_string());
    let text = fs::read_to_string(&path_buf).map_err(|_| TaskFinishError::MissingHandoff {
        repository: repository.clone(),
        path: path.clone(),
    })?;
    try_parse_summary(&text, repository.as_str()).map_err(|error| TaskFinishError::InvalidHandoff {
        repository: repository.clone(),
        error,
        path,
    })
}

fn candidate_for(
    statuses: &[(&TaskCommitTarget, RepositoryStatus)],
    repository: &WorkspaceRepositoryName,
    project_config: Option<&ProjectConfig>,
) -> Option<PullRequestCandidate> {
    statuses
        .iter()
        .find(|(target, _)| {
            target
                .repository
                .as_str()
                .eq_ignore_ascii_case(repository.as_str())
        })
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
        .filter(|value| !value.trim().is_empty())
        .map(AdoRepositoryName::from);
    Some(PullRequestCandidate {
        repository: target.repository.clone(),
        path: target.path.clone(),
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
        .unwrap_or_else(|| BranchName::from("main"));
    let base = GitRevision::from(format!("origin/{target}"));
    has_commits_ahead_of(&RepositoryPath::from(path), &base).unwrap_or(false)
}

fn target_branch(repo: Option<&RepositoryConfig>) -> BranchName {
    repo.and_then(|repo| repo.pull_request_target_branch.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            repo.map(|repo| repo.default_branch.clone())
                .filter(|value| !value.trim().is_empty())
        })
        .map(BranchName::from)
        .unwrap_or_else(|| BranchName::from("main"))
}

fn render_verification(
    repository: &WorkspaceRepositoryName,
    results: &[VerificationResult],
) -> String {
    let matching = results
        .iter()
        .filter(|result| {
            result
                .repository
                .as_str()
                .eq_ignore_ascii_case(repository.as_str())
        })
        .map(|result| {
            format!(
                "- `{}`: {}",
                result.command,
                if result.exit_code.success() {
                    "OK"
                } else {
                    "KO"
                }
            )
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        "- Aucune commande configurée dans `taskFinish.verificationCommands`.".into()
    } else {
        matching.join("\n")
    }
}

fn render_list(items: &[HandoffSummaryEntry]) -> String {
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

fn state_property(value: &Value, key: &str) -> Option<WorkItemState> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(WorkItemState::from)
}

fn verification_commands(
    value: &Value,
) -> BTreeMap<WorkspaceRepositoryName, Vec<VerificationCommand>> {
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
                                .map(VerificationCommand::from)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    (!commands.is_empty())
                        .then(|| (WorkspaceRepositoryName::from(repository.as_str()), commands))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_node_package_manager_command(command: &VerificationCommand) -> VerificationCommand {
    let command = command.as_str();
    let trimmed = command.trim_start();
    if !trimmed.starts_with("npm ") || !command_available("pnpm") {
        return VerificationCommand::from(command);
    }
    let leading_whitespace = command.len() - trimmed.len();
    VerificationCommand::from(format!(
        "{}pnpm{}",
        &command[..leading_whitespace],
        &trimmed["npm".len()..]
    ))
}

fn command_available(command: &str) -> bool {
    dw_process::command_available(command, &["--version"])
}

fn shell_verification_output(
    repository_path: &RepositoryPath,
    command: &VerificationCommand,
) -> VerificationCommandOutput {
    let output = run_shell(repository_path, command);
    VerificationCommandOutput {
        exit_code: output
            .as_ref()
            .ok()
            .and_then(|output| output.status.code())
            .unwrap_or(1)
            .into(),
        standard_output: output
            .as_ref()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .unwrap_or_default()
            .into(),
        standard_error: output
            .as_ref()
            .map(|output| String::from_utf8_lossy(&output.stderr).to_string())
            .unwrap_or_else(|error| error.to_string())
            .into(),
    }
}

fn run_shell(
    repository_path: &RepositoryPath,
    command: &VerificationCommand,
) -> std::io::Result<std::process::Output> {
    if cfg!(windows) {
        dw_process::output_in(
            "powershell",
            [
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command.as_str(),
            ],
            Some(Path::new(repository_path.as_str())),
        )
    } else {
        dw_process::output_in(
            "sh",
            ["-lc", command.as_str()],
            Some(Path::new(repository_path.as_str())),
        )
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
    use crate::{
        HandoffSummaryEntry, WorkspaceHandoffStatus, WorkspaceHandoffSummary, WorkspaceManifest,
    };

    #[test]
    fn finish_state_never_moves_parent_story_types() {
        let options = TaskFinishOptions::default();

        assert_eq!(finish_state(Some("User Story"), &options), None);
        assert_eq!(finish_state(Some("Anomalie"), &options), None);
        assert_eq!(
            finish_state(Some("Bug"), &options),
            Some(WorkItemState::from("PR en attente"))
        );
        assert_eq!(
            finish_state(Some("Task"), &options),
            Some(WorkItemState::from("PR en attente"))
        );
        assert_eq!(
            finish_state(Some("Tâche"), &options),
            Some(WorkItemState::from("PR en attente"))
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
        assert_eq!(options.bug_state, WorkItemState::from("Review"));
        assert_eq!(options.task_state, WorkItemState::from("Done"));
        assert_eq!(
            options.verification_commands[&WorkspaceRepositoryName::from("front")],
            vec![VerificationCommand::from("npm test")]
        );
    }

    #[test]
    fn structured_handoff_section_renders_summary_lists() {
        let handoff = WorkspaceHandoffSummary {
            repository: WorkspaceRepositoryName::from("front"),
            status: WorkspaceHandoffStatus::Done,
            done: vec![HandoffSummaryEntry::from("Implémenter le composant")],
            decisions: vec![HandoffSummaryEntry::from("Conserver le libellé métier")],
            risks: vec![HandoffSummaryEntry::from("Régression responsive")],
            blockers: vec![],
            follow_up: vec![HandoffSummaryEntry::from("Valider avec le user")],
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
            work_item_id: "103".into(),
            task_id: Some("109".into()),
            project: "acme".into(),
            kind: "bug".into(),
            slug: "corriger-ouverture".into(),
            branch_name: "bug/103-109-corriger-ouverture".into(),
            created_at: "2026-07-03T00:00:00Z".into(),
            repositories: vec!["front".into()],
            status: crate::WorkspaceManifestStatus::Created,
            work_item_type: Some("Bug".into()),
            work_item_title: None,
            work_item_state: None,
            work_items: None,
            child_task_ids: None,
            child_tasks: None,
        };
        let candidate = PullRequestCandidate {
            repository: WorkspaceRepositoryName::from("front"),
            path: RepositoryPath::from("/tmp/front"),
            ado_repository: Some(AdoRepositoryName::from("FrontRepo")),
            target_branch: BranchName::from("main"),
        };
        let handoff = WorkspaceHandoffSummary {
            repository: WorkspaceRepositoryName::from("front"),
            status: WorkspaceHandoffStatus::Done,
            done: vec![HandoffSummaryEntry::from("Changer la page")],
            decisions: vec![],
            risks: vec![],
            blockers: vec![],
            follow_up: vec![],
        };
        let verification = vec![VerificationResult {
            repository: WorkspaceRepositoryName::from("front"),
            command: VerificationCommand::from("pnpm test"),
            exit_code: VerificationExitCode::from(0),
            standard_output: VerificationOutputText::from(""),
            standard_error: VerificationOutputText::from(""),
        }];

        let rendered = pull_request_description(
            &manifest,
            &candidate,
            "## Plan\nFaire le correctif",
            &verification,
            &handoff,
        );

        assert!(rendered.contains("Travail réalisé pour `corriger-ouverture`"));
        assert!(rendered.contains("Work items : `#103, #109`"));
        assert!(rendered.contains("Faire le correctif"));
        assert!(rendered.contains("Changer la page"));
        assert!(rendered.contains("`pnpm test`: OK"));
    }
}
