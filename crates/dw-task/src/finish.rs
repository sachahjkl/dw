use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    CreatePullRequestInput, PullRequestCreateResult, create_pull_request_authenticated,
    get_work_item_snapshot_authenticated, link_work_item_to_pull_request_authenticated,
    run_blocking_ado, try_find_active_pull_request_authenticated,
    update_work_item_state_authenticated,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_core::{
    DevWorkflowRoot, GitOperation, RepositoryPath, TaskActionEvent, WorkItemId, WorkspacePath,
    WorkspaceRepositoryName,
};
use dw_git::{RepositoryStatus, commit_repository, push_repository, repository_status};
use dw_workspace::{
    WorkspaceHandoffSummary, WorkspaceManifest, build_commit_message, ensure_verification_passed,
    finish_state, plan_task_finish, pull_request_description, pull_request_title,
    read_handoff_summary, read_plan, resolve_workspace_for_workspace_command, run_verification,
    select_pull_request_candidates, task_finish_options,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FinishArgs {
    pub workspace: Option<WorkspacePath>,
    pub r#continue: bool,
    pub root: Option<DevWorkflowRoot>,
    pub mode: dw_core::ExecutionMode,
    pub yes: bool,
    pub message: Option<String>,
    pub create_pr: bool,
    pub ready: bool,
    pub skip_verify: bool,
    pub skip_ado: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FinishPlanReport {
    pub root: DevWorkflowRoot,
    pub workspace: WorkspacePath,
    pub manifest: WorkspaceManifest,
    pub targets: Vec<FinishTargetStatus>,
    pub handoff: dw_contracts::TaskHandoffValidationReport,
    #[serde(rename = "handoffSummaries")]
    pub handoff_summaries: Vec<WorkspaceHandoffSummary>,
    #[serde(rename = "commitMessage")]
    pub commit_message: String,
    #[serde(rename = "createPr")]
    pub create_pr: bool,
    pub ready: bool,
    #[serde(rename = "skipAdo")]
    pub skip_ado: bool,
    #[serde(rename = "changedRepositories")]
    pub changed_repositories: Vec<WorkspaceRepositoryName>,
    #[serde(rename = "unpushedRepositories")]
    pub unpushed_repositories: Vec<WorkspaceRepositoryName>,
    #[serde(rename = "actionableRepositories")]
    pub actionable_repositories: Vec<WorkspaceRepositoryName>,
    #[serde(rename = "pullRequestCandidates")]
    pub pull_request_candidates: Vec<dw_workspace::PullRequestCandidate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FinishTargetStatus {
    pub target: dw_workspace::TaskCommitTarget,
    pub status: RepositoryStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FinishExecutionReport {
    pub plan: FinishPlanReport,
    pub events: Vec<TaskActionEvent>,
    #[serde(rename = "verificationResults")]
    pub verification_results: Vec<dw_workspace::VerificationResult>,
    #[serde(rename = "gitActions")]
    pub git_actions: Vec<FinishGitAction>,
    #[serde(rename = "pullRequests")]
    pub pull_requests: Vec<FinishPullRequestResult>,
    #[serde(rename = "workItemUpdates")]
    pub work_item_updates: Vec<FinishWorkItemStateUpdate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FinishGitAction {
    pub repository: WorkspaceRepositoryName,
    pub operation: GitOperation,
    pub path: RepositoryPath,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FinishPullRequestResult {
    pub repository: WorkspaceRepositoryName,
    pub action: FinishPullRequestAction,
    pub url: Option<String>,
    #[serde(rename = "pullRequestId")]
    pub pull_request_id: Option<i64>,
    #[serde(rename = "skipReason")]
    pub skip_reason: Option<FinishPullRequestSkipReason>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FinishPullRequestAction {
    Created,
    Existing,
    Skipped,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FinishPullRequestSkipReason {
    MissingAdoRepository,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FinishWorkItemStateUpdate {
    pub id: WorkItemId,
    pub label: String,
    pub kind: Option<String>,
    #[serde(rename = "currentState")]
    pub current_state: Option<String>,
    #[serde(rename = "targetState")]
    pub target_state: Option<String>,
    pub changed: bool,
    pub outcome: FinishWorkItemStateOutcome,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FinishWorkItemStateOutcome {
    UnsupportedWorkItemType,
    AlreadyInTargetState,
    Updated,
}

pub fn finish_plan(args: FinishArgs) -> Result<FinishPlanReport> {
    let root = resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        args.workspace.as_ref().map(WorkspacePath::as_str),
        args.r#continue,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let (manifest, targets, handoff) = plan_task_finish(&projects, &workspace)?;
    let project_config = resolve_project(&projects, manifest.project.as_str());
    let targets = targets
        .into_iter()
        .map(|target| {
            let status = repository_status(target.path.as_str());
            FinishTargetStatus { target, status }
        })
        .collect::<Vec<_>>();
    let handoff_summaries = targets
        .iter()
        .filter_map(|target| {
            read_handoff_summary(
                Path::new(workspace.as_str()),
                target.target.repository.as_str(),
            )
            .ok()
        })
        .collect::<Vec<WorkspaceHandoffSummary>>();
    let changed_repositories = targets
        .iter()
        .filter(|target| target.status.is_git_repository && target.status.has_changes)
        .map(|target| target.target.repository.clone())
        .collect::<Vec<_>>();
    let unpushed_repositories = targets
        .iter()
        .filter(|target| target.status.is_git_repository && target.status.has_unpushed)
        .map(|target| target.target.repository.clone())
        .collect::<Vec<_>>();
    let actionable_repositories = if changed_repositories.is_empty() {
        unpushed_repositories.clone()
    } else {
        changed_repositories.clone()
    };
    let pull_request_candidates = if args.create_pr {
        let status_refs = targets
            .iter()
            .map(|target| (&target.target, target.status.clone()))
            .collect::<Vec<_>>();
        select_pull_request_candidates(
            &status_refs,
            &actionable_repositories,
            project_config.as_ref(),
        )
    } else {
        Vec::new()
    };

    Ok(FinishPlanReport {
        root: DevWorkflowRoot::from(root),
        workspace,
        commit_message: build_commit_message(&manifest, args.message.as_deref()),
        manifest,
        targets,
        handoff,
        handoff_summaries,
        create_pr: args.create_pr,
        ready: args.ready,
        skip_ado: args.skip_ado,
        changed_repositories,
        unpushed_repositories,
        actionable_repositories,
        pull_request_candidates,
    })
}

pub async fn execute_finish(
    plan: FinishPlanReport,
    args: &FinishArgs,
) -> Result<FinishExecutionReport> {
    execute_finish_with_events(plan, args, |_| {}).await
}

pub async fn execute_finish_with_events(
    plan: FinishPlanReport,
    args: &FinishArgs,
    mut emit: impl FnMut(TaskActionEvent),
) -> Result<FinishExecutionReport> {
    if plan.create_pr && plan.skip_ado {
        anyhow::bail!("La création de PR ne peut pas être combinée avec le mode sans ADO.");
    }
    if !plan.handoff.is_valid {
        anyhow::bail!(
            "task finish bloqué: handoff invalide. Corriger ou compléter les handoffs avant push."
        );
    }

    let projects = load_projects_config(plan.root.as_str());
    let workflow = load_workflow_config(plan.root.as_str());
    let finish_options = task_finish_options(&workflow);
    let mut events = Vec::new();
    let mut verification_results = Vec::new();
    let mut git_actions = Vec::new();
    let mut pull_requests = Vec::new();
    let mut work_item_updates = Vec::new();

    if !args.skip_verify && finish_options.run_verification {
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::VerifyingFinish {
                pull_request_candidate_count: plan.pull_request_candidates.len(),
            },
        );
        verification_results = run_verification(&finish_options, &plan.pull_request_candidates);
        ensure_verification_passed(&verification_results)?;
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::FinishVerificationCompleted,
        );
    }

    let changed = changed_targets(&plan);
    let unpushed = unpushed_targets(&plan);
    if !changed.is_empty() {
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::RunningGitOperation {
                operation: GitOperation::CommitAndPush,
                repository_count: changed.len(),
            },
        );
        for target in changed {
            push_event(
                &mut events,
                &mut emit,
                TaskActionEvent::RunningRepositoryGitOperation {
                    repository: target.target.repository.clone(),
                    operation: GitOperation::CommitAndPush,
                },
            );
            commit_repository(target.target.path.as_str(), &plan.commit_message)?;
            push_repository(
                target.target.path.as_str(),
                plan.manifest.branch_name.as_str(),
            )?;
            git_actions.push(FinishGitAction {
                repository: target.target.repository.clone(),
                operation: GitOperation::CommitAndPush,
                path: target.target.path.clone(),
            });
        }
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::GitOperationCompleted {
                operation: GitOperation::CommitAndPush,
            },
        );
    } else if !unpushed.is_empty() {
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::RunningGitOperation {
                operation: GitOperation::Push,
                repository_count: unpushed.len(),
            },
        );
        for target in unpushed {
            push_event(
                &mut events,
                &mut emit,
                TaskActionEvent::RunningRepositoryGitOperation {
                    repository: target.target.repository.clone(),
                    operation: GitOperation::Push,
                },
            );
            push_repository(
                target.target.path.as_str(),
                plan.manifest.branch_name.as_str(),
            )?;
            git_actions.push(FinishGitAction {
                repository: target.target.repository.clone(),
                operation: GitOperation::Push,
                path: target.target.path.clone(),
            });
        }
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::GitOperationCompleted {
                operation: GitOperation::Push,
            },
        );
    }

    if !plan.create_pr {
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::SkippingPullRequestCreation,
        );
        return Ok(FinishExecutionReport {
            plan,
            events,
            verification_results,
            git_actions,
            pull_requests,
            work_item_updates,
        });
    }

    push_event(
        &mut events,
        &mut emit,
        TaskActionEvent::AuthenticatingAdoForPullRequests {
            pull_request_candidate_count: plan.pull_request_candidates.len(),
        },
    );
    let mut options = resolve_ado_options(&projects, &workflow, plan.manifest.project.as_str())?;
    if options.project.trim().is_empty() {
        options.project = plan.manifest.project.to_string();
    }
    let token = require_token(load_auth_options(Some(plan.root.as_str()))?).await?;
    let source_ref = format!("refs/heads/{}", plan.manifest.branch_name);
    let task_plan = read_plan(Path::new(plan.workspace.as_str()));

    for candidate in &plan.pull_request_candidates {
        let Some(ado_repository) = candidate.ado_repository.as_ref() else {
            pull_requests.push(FinishPullRequestResult {
                repository: WorkspaceRepositoryName::from(candidate.repository.clone()),
                action: FinishPullRequestAction::Skipped,
                url: None,
                pull_request_id: None,
                skip_reason: Some(FinishPullRequestSkipReason::MissingAdoRepository),
            });
            continue;
        };
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::CheckingActivePullRequest {
                repository: candidate.repository.clone().into(),
            },
        );
        let options_for_find = options.clone();
        let repository_for_find = ado_repository.clone();
        let source_ref_for_find = source_ref.clone();
        let token_for_find = token.clone();
        if let Some(existing) = run_blocking_ado(move || {
            try_find_active_pull_request_authenticated(
                &options_for_find,
                &repository_for_find,
                &source_ref_for_find,
                &token_for_find,
            )
        })
        .await?
        {
            pull_requests.push(FinishPullRequestResult {
                repository: WorkspaceRepositoryName::from(candidate.repository.clone()),
                action: FinishPullRequestAction::Existing,
                url: existing.url,
                pull_request_id: Some(existing.pull_request_id),
                skip_reason: None,
            });
            continue;
        }
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::CreatingPullRequest {
                repository: candidate.repository.clone().into(),
            },
        );
        let handoff_summary =
            read_handoff_summary(Path::new(plan.workspace.as_str()), &candidate.repository)?;
        let input = CreatePullRequestInput {
            repository: ado_repository.clone(),
            source_ref_name: source_ref.clone(),
            target_ref_name: format!("refs/heads/{}", candidate.target_branch),
            title: pull_request_title(&plan.manifest),
            description: pull_request_description(
                &plan.manifest,
                candidate,
                &task_plan,
                &verification_results,
                &handoff_summary,
            ),
            is_draft: !plan.ready,
            work_item_ids: plan
                .manifest
                .all_known_work_item_ids()
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        };
        let options_for_create = options.clone();
        let token_for_create = token.clone();
        let created = run_blocking_ado(move || {
            create_pull_request_authenticated(&options_for_create, &input, &token_for_create)
        })
        .await?;
        if let Some(pull_request_id) = created.pull_request_id {
            for id in plan.manifest.all_known_work_item_ids() {
                let options_for_link = options.clone();
                let token_for_link = token.clone();
                let repository_for_link = ado_repository.clone();
                let id_for_link = id.clone();
                if let Err(error) = run_blocking_ado(move || {
                    link_work_item_to_pull_request_authenticated(
                        &options_for_link,
                        &repository_for_link,
                        pull_request_id,
                        id_for_link.as_str(),
                        &token_for_link,
                    )
                })
                .await
                {
                    push_event(
                        &mut events,
                        &mut emit,
                        TaskActionEvent::PullRequestWorkItemLinkSkipped {
                            work_item_id: id,
                            error: error.to_string(),
                        },
                    );
                }
            }
        }
        pull_requests.push(created_pr_result(&candidate.repository, created));
    }

    if finish_options.update_work_item_state {
        let ids = plan.manifest.all_known_work_item_ids();
        push_event(
            &mut events,
            &mut emit,
            TaskActionEvent::UpdatingFinishWorkItemStates {
                work_item_ids: ids.clone(),
            },
        );
        for id in ids {
            let options_for_fetch = options.clone();
            let token_for_fetch = token.clone();
            let id_for_fetch = id.clone();
            let item = run_blocking_ado(move || {
                get_work_item_snapshot_authenticated(
                    &options_for_fetch,
                    id_for_fetch.as_str(),
                    &token_for_fetch,
                )
            })
            .await?;
            let state = finish_state(
                item.kind.as_deref().or(plan
                    .manifest
                    .work_item_type
                    .as_ref()
                    .map(|kind| kind.as_str())),
                &finish_options,
            );
            let label = work_item_label(&item);
            let Some(state) = state else {
                work_item_updates.push(FinishWorkItemStateUpdate {
                    id: WorkItemId::from(item.id),
                    label,
                    kind: item.kind.clone(),
                    current_state: item.state,
                    target_state: None,
                    changed: false,
                    outcome: FinishWorkItemStateOutcome::UnsupportedWorkItemType,
                });
                continue;
            };
            if item
                .state
                .as_deref()
                .is_some_and(|current| current.eq_ignore_ascii_case(&state))
            {
                work_item_updates.push(FinishWorkItemStateUpdate {
                    id: WorkItemId::from(item.id),
                    label,
                    kind: item.kind.clone(),
                    current_state: item.state,
                    target_state: Some(state.clone()),
                    changed: false,
                    outcome: FinishWorkItemStateOutcome::AlreadyInTargetState,
                });
                continue;
            }
            let options_for_update = options.clone();
            let token_for_update = token.clone();
            let id_for_update = id.clone();
            let state_for_update = state.clone();
            run_blocking_ado(move || {
                update_work_item_state_authenticated(
                    &options_for_update,
                    id_for_update.as_str(),
                    &state_for_update,
                    "task finish: PR ouverte",
                    &token_for_update,
                )
            })
            .await?;
            work_item_updates.push(FinishWorkItemStateUpdate {
                id: WorkItemId::from(item.id),
                label,
                kind: item.kind.clone(),
                current_state: item.state,
                target_state: Some(state.clone()),
                changed: true,
                outcome: FinishWorkItemStateOutcome::Updated,
            });
        }
    }

    Ok(FinishExecutionReport {
        plan,
        events,
        verification_results,
        git_actions,
        pull_requests,
        work_item_updates,
    })
}

pub fn changed_targets(plan: &FinishPlanReport) -> Vec<&FinishTargetStatus> {
    plan.targets
        .iter()
        .filter(|target| target.status.is_git_repository && target.status.has_changes)
        .collect()
}

pub fn unpushed_targets(plan: &FinishPlanReport) -> Vec<&FinishTargetStatus> {
    plan.targets
        .iter()
        .filter(|target| target.status.is_git_repository && target.status.has_unpushed)
        .collect()
}

pub fn finish_has_work(plan: &FinishPlanReport) -> bool {
    !plan.changed_repositories.is_empty()
        || !plan.unpushed_repositories.is_empty()
        || !plan.pull_request_candidates.is_empty()
}

fn created_pr_result(
    repository: &str,
    created: PullRequestCreateResult,
) -> FinishPullRequestResult {
    FinishPullRequestResult {
        repository: repository.into(),
        action: FinishPullRequestAction::Created,
        url: created.url,
        pull_request_id: created.pull_request_id,
        skip_reason: None,
    }
}

fn push_event(
    events: &mut Vec<TaskActionEvent>,
    emit: &mut impl FnMut(TaskActionEvent),
    event: TaskActionEvent,
) {
    emit(event.clone());
    events.push(event);
}

fn work_item_label(item: &dw_ado::WorkItemSnapshot) -> String {
    format!(
        "#{}{}{}",
        item.id,
        item.kind
            .as_ref()
            .map(|kind| format!(" [{kind}]"))
            .unwrap_or_default(),
        item.title
            .as_ref()
            .map(|title| format!(" {title}"))
            .unwrap_or_default()
    )
}
