use crate::{load_auth_options, resolve_ado_options};
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    CreatePullRequestInput, create_pull_request_authenticated,
    get_work_item_snapshot_authenticated, link_work_item_to_pull_request_authenticated,
    try_find_active_pull_request_authenticated, update_work_item_state_authenticated,
};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_git::{commit_repository, push_repository, repository_status};
use dw_ui::{confirm_when_interactive, select_optional};
use dw_workspace::{
    WorkspaceHandoffSummary, build_commit_message, ensure_verification_passed, finish_state,
    plan_task_finish, pull_request_description, pull_request_title, read_handoff_summary,
    read_plan, resolve_workspace_for_workspace_command, run_verification,
    select_pull_request_candidates, task_finish_options,
};
use std::path::Path;

use self::render::{FinishSummary, finish_dry_run_hint, finish_summary_lines};
use crate::render::{print_styled, print_styled_lines};

mod render;

pub struct FinishArgs {
    pub workspace: Option<String>,
    pub r#continue: bool,
    pub root: Option<String>,
    pub execute: bool,
    pub message: Option<String>,
    pub create_pr: bool,
    pub ready: bool,
    pub skip_verify: bool,
    pub skip_ado: bool,
    pub json: bool,
}

pub fn handle(args: FinishArgs) -> Result<()> {
    let FinishArgs {
        workspace,
        r#continue,
        root,
        execute,
        message,
        mut create_pr,
        mut ready,
        skip_verify,
        mut skip_ado,
        json,
    } = args;

    let root = resolve_root(root.as_deref());
    let workspace = resolve_workspace_for_workspace_command(
        &root,
        workspace.as_deref(),
        r#continue,
        &std::env::current_dir()?.display().to_string(),
    )?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let (manifest, targets, handoff) = plan_task_finish(&projects, &workspace)?;
    let project_config = resolve_project(&projects, &manifest.project);
    let statuses = targets
        .iter()
        .map(|target| (target, repository_status(&target.path)))
        .collect::<Vec<_>>();
    let handoff_summaries = targets
        .iter()
        .filter_map(|target| read_handoff_summary(Path::new(&workspace), &target.repository).ok())
        .collect::<Vec<WorkspaceHandoffSummary>>();
    let changed = statuses
        .iter()
        .filter(|(_, status)| status.is_git_repository && status.has_changes)
        .collect::<Vec<_>>();
    let unpushed = statuses
        .iter()
        .filter(|(_, status)| status.is_git_repository && status.has_unpushed)
        .collect::<Vec<_>>();
    let actionable_repositories = if changed.is_empty() {
        unpushed
            .iter()
            .map(|(target, _)| target.repository.clone())
            .collect::<Vec<_>>()
    } else {
        changed
            .iter()
            .map(|(target, _)| target.repository.clone())
            .collect::<Vec<_>>()
    };

    if should_prompt_finish_mode(execute, json, create_pr, ready, skip_ado)
        && let Some(mode) = select_finish_mode()?
    {
        apply_finish_mode(mode, &mut create_pr, &mut ready, &mut skip_ado);
    }

    let pull_request_candidates = if create_pr {
        select_pull_request_candidates(&statuses, &actionable_repositories, project_config.as_ref())
    } else {
        Vec::new()
    };
    let commit_message = build_commit_message(&manifest, message.as_deref());

    if json {
        let report = serde_json::json!({
            "workspace": workspace,
            "branch": manifest.branch_name,
            "message": commit_message,
            "handoff": handoff,
            "targets": statuses.iter().map(|(target, status)| serde_json::json!({
                "repository": target.repository,
                "path": status.path,
                "isGitRepository": status.is_git_repository,
                "hasChanges": status.has_changes,
                "hasUnpushed": status.has_unpushed,
                "detail": status.detail,
            })).collect::<Vec<_>>(),
            "pullRequestCandidates": pull_request_candidates,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_styled_lines(&finish_summary_lines(FinishSummary {
            workspace: &workspace,
            branch_name: &manifest.branch_name,
            statuses: &statuses,
            handoff: &handoff,
            handoff_summaries: &handoff_summaries,
            commit_message: &commit_message,
            has_changes: !changed.is_empty(),
            create_pr,
            pull_request_candidates: &pull_request_candidates,
        }));
    }

    if changed.is_empty() && unpushed.is_empty() && pull_request_candidates.is_empty() {
        if !json {
            print_styled("");
            print_styled("Rien à terminer.");
        }
        return Ok(());
    }
    if !execute {
        if !json {
            print_styled("");
            print_styled(finish_dry_run_hint(changed.is_empty(), create_pr));
        }
        return Ok(());
    }
    if create_pr && skip_ado {
        return Err(anyhow::anyhow!(
            "--create-pr ne peut pas être combiné avec --skip-ado."
        ));
    }
    if !confirm_when_interactive(&finish_confirmation_prompt(
        &workspace,
        !changed.is_empty(),
        !unpushed.is_empty(),
        create_pr,
        skip_ado,
    ))? {
        if !json {
            print_styled("Finalisation annulée.");
        }
        return Ok(());
    }

    if !handoff.is_valid {
        return Err(anyhow::anyhow!(
            "task finish bloqué: handoff invalide. Corriger ou compléter les handoffs avant push."
        ));
    }
    let finish_options = task_finish_options(&workflow);
    let verification_results = if !skip_verify && finish_options.run_verification {
        let actionable_candidates = select_pull_request_candidates(
            &statuses,
            &actionable_repositories,
            project_config.as_ref(),
        );
        let results = run_verification(&finish_options, &actionable_candidates);
        ensure_verification_passed(&results)?;
        results
    } else {
        Vec::new()
    };
    if !changed.is_empty() {
        for (target, _) in &changed {
            commit_repository(&target.path, &commit_message)?;
            push_repository(&target.path, &manifest.branch_name)?;
        }
        if !json {
            print_styled("Commits/push terminés.");
        }
    } else {
        for (target, _) in &unpushed {
            push_repository(&target.path, &manifest.branch_name)?;
        }
        if !json {
            print_styled("Push terminé.");
        }
    }
    if !create_pr {
        if !json {
            print_styled("PR non créée. Relancer avec --create-pr pour ouvrir les PR ADO.");
        }
        return Ok(());
    }

    if !changed.is_empty() && !json {
        print_styled("Commits/push terminés. Création PR en cours.");
    } else if changed.is_empty() && unpushed.is_empty() && !json {
        print_styled("Aucun commit local à pousser. Vérification PR en cours.");
    }

    let mut options = resolve_ado_options(&projects, &workflow, &manifest.project)?;
    if options.project.trim().is_empty() {
        options.project = manifest.project.clone();
    }
    let token = require_token(load_auth_options(Some(&root))?)?;
    let source_ref = format!("refs/heads/{}", manifest.branch_name);
    let plan = read_plan(Path::new(&workspace));

    for candidate in &pull_request_candidates {
        let Some(ado_repository) = candidate.ado_repository.as_ref() else {
            print_styled(&format!(
                "PR ignorée pour {}: azureDevOpsRepository manquant.",
                candidate.repository
            ));
            continue;
        };
        if let Some(existing) = try_find_active_pull_request_authenticated(
            &options,
            ado_repository,
            &source_ref,
            &token,
        )? {
            print_styled(&format!(
                "PR déjà ouverte pour {}: {}",
                candidate.repository,
                existing.url.as_deref().unwrap_or("(url non retournée)")
            ));
            continue;
        }
        let handoff_summary = read_handoff_summary(Path::new(&workspace), &candidate.repository)?;
        let input = CreatePullRequestInput {
            repository: ado_repository.clone(),
            source_ref_name: source_ref.clone(),
            target_ref_name: format!("refs/heads/{}", candidate.target_branch),
            title: pull_request_title(&manifest),
            description: pull_request_description(
                &manifest,
                candidate,
                &plan,
                &verification_results,
                &handoff_summary,
            ),
            is_draft: !ready,
            work_item_ids: manifest.all_known_work_item_ids(),
        };
        let created = create_pull_request_authenticated(&options, &input, &token)?;
        if let Some(pull_request_id) = created.pull_request_id {
            for id in manifest.all_known_work_item_ids() {
                if let Err(error) = link_work_item_to_pull_request_authenticated(
                    &options,
                    ado_repository,
                    pull_request_id,
                    &id,
                    &token,
                ) {
                    print_styled(&format!(
                        "Lien PR/work item déjà demandé à la création, lien explicite ignoré pour #{}: {}",
                        id, error
                    ));
                }
            }
        }
        print_styled(&format!(
            "PR créée pour {}: {}",
            candidate.repository,
            created.url.as_deref().unwrap_or("(url non retournée)")
        ));
    }

    if finish_options.update_work_item_state {
        for id in manifest.all_known_work_item_ids() {
            let item = get_work_item_snapshot_authenticated(&options, &id, &token)?;
            let state = finish_state(
                item.kind.as_deref().or(manifest.work_item_type.as_deref()),
                &finish_options,
            );
            let label = format!(
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
            );
            let Some(state) = state else {
                print_styled(&format!(
                    "ADO item {}: état inchangé ({}).",
                    label,
                    item.kind.as_deref().unwrap_or("type inconnu")
                ));
                continue;
            };
            if item
                .state
                .as_deref()
                .is_some_and(|current| current.eq_ignore_ascii_case(&state))
            {
                print_styled(&format!("ADO item {label}: déjà en état {state}."));
                continue;
            }
            update_work_item_state_authenticated(
                &options,
                &id,
                &state,
                "dw task finish: PR ouverte",
                &token,
            )?;
            print_styled(&format!("ADO item {label}: état -> {state}"));
        }
    }

    Ok(())
}

fn finish_confirmation_prompt(
    workspace: &str,
    has_changes: bool,
    has_unpushed: bool,
    create_pr: bool,
    skip_ado: bool,
) -> String {
    let mut actions = Vec::new();
    if has_changes {
        actions.push("commit");
    }
    if has_changes || has_unpushed {
        actions.push("push");
    }
    if create_pr {
        actions.push("PR ADO");
    } else if skip_ado {
        actions.push("sans ADO");
    }

    format!(
        "Exécuter la finalisation ({}) ?\n{}",
        actions.join(" + "),
        workspace
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinishMode {
    PushOnly,
    DraftPr,
    ReadyPr,
    KeepFlags,
}

fn should_prompt_finish_mode(
    execute: bool,
    json: bool,
    create_pr: bool,
    ready: bool,
    skip_ado: bool,
) -> bool {
    execute && !json && !create_pr && !ready && !skip_ado
}

fn finish_mode_choices() -> Vec<String> {
    vec![
        "Push uniquement, sans ADO".to_string(),
        "Push + PR ADO draft".to_string(),
        "Push + PR ADO ready".to_string(),
        "Garder les flags actuels".to_string(),
    ]
}

fn finish_mode_from_label(label: &str) -> FinishMode {
    match label {
        "Push + PR ADO draft" => FinishMode::DraftPr,
        "Push + PR ADO ready" => FinishMode::ReadyPr,
        "Garder les flags actuels" => FinishMode::KeepFlags,
        _ => FinishMode::PushOnly,
    }
}

fn select_finish_mode() -> Result<Option<FinishMode>> {
    Ok(
        select_optional("Mode de finalisation", finish_mode_choices())?
            .map(|label| finish_mode_from_label(&label)),
    )
}

fn apply_finish_mode(
    mode: FinishMode,
    create_pr: &mut bool,
    ready: &mut bool,
    skip_ado: &mut bool,
) {
    match mode {
        FinishMode::PushOnly => {
            *create_pr = false;
            *ready = false;
            *skip_ado = true;
        }
        FinishMode::DraftPr => {
            *create_pr = true;
            *ready = false;
            *skip_ado = false;
        }
        FinishMode::ReadyPr => {
            *create_pr = true;
            *ready = true;
            *skip_ado = false;
        }
        FinishMode::KeepFlags => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FinishMode, apply_finish_mode, finish_confirmation_prompt, finish_mode_choices,
        finish_mode_from_label, should_prompt_finish_mode,
    };

    #[test]
    fn finish_confirmation_prompt_summarizes_actions() {
        assert_eq!(
            finish_confirmation_prompt("/tmp/ws", true, false, true, false),
            "Exécuter la finalisation (commit + push + PR ADO) ?\n/tmp/ws"
        );
        assert_eq!(
            finish_confirmation_prompt("/tmp/ws", false, true, false, true),
            "Exécuter la finalisation (push + sans ADO) ?\n/tmp/ws"
        );
    }

    #[test]
    fn finish_mode_prompt_only_when_flags_leave_it_ambiguous() {
        assert!(should_prompt_finish_mode(true, false, false, false, false));
        assert!(!should_prompt_finish_mode(
            false, false, false, false, false
        ));
        assert!(!should_prompt_finish_mode(true, true, false, false, false));
        assert!(!should_prompt_finish_mode(true, false, true, false, false));
        assert!(!should_prompt_finish_mode(true, false, false, true, false));
        assert!(!should_prompt_finish_mode(true, false, false, false, true));
    }

    #[test]
    fn finish_mode_choices_map_to_modes() {
        let choices = finish_mode_choices();
        assert_eq!(finish_mode_from_label(&choices[0]), FinishMode::PushOnly);
        assert_eq!(finish_mode_from_label(&choices[1]), FinishMode::DraftPr);
        assert_eq!(finish_mode_from_label(&choices[2]), FinishMode::ReadyPr);
        assert_eq!(finish_mode_from_label(&choices[3]), FinishMode::KeepFlags);
    }

    #[test]
    fn finish_modes_apply_flags() {
        let (mut create_pr, mut ready, mut skip_ado) = (false, false, false);
        apply_finish_mode(
            FinishMode::ReadyPr,
            &mut create_pr,
            &mut ready,
            &mut skip_ado,
        );
        assert!(create_pr);
        assert!(ready);
        assert!(!skip_ado);

        apply_finish_mode(
            FinishMode::PushOnly,
            &mut create_pr,
            &mut ready,
            &mut skip_ado,
        );
        assert!(!create_pr);
        assert!(!ready);
        assert!(skip_ado);
    }
}
