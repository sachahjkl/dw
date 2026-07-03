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
use dw_workspace::{
    build_commit_message, ensure_verification_passed, finish_state, plan_task_finish,
    pull_request_description, pull_request_title, read_handoff_summary, read_plan,
    resolve_workspace_for_workspace_command, run_verification, select_pull_request_candidates,
    task_finish_options,
};
use std::path::Path;

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
        create_pr,
        ready,
        skip_verify,
        skip_ado,
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
        println!("Workspace: {workspace}");
        println!("Branche: {}", manifest.branch_name);
        for (target, status) in &statuses {
            print_repository_status(&target.repository, status);
        }
        println!();
        println!(
            "Handoff validation: {}",
            if handoff.is_valid { "OK" } else { "KO" }
        );
        for item in &handoff.items {
            println!("- [{}] {}: {}", item.status, item.repository, item.message);
        }
        if !changed.is_empty() {
            println!();
            println!("Message: {commit_message}");
        }
        if create_pr {
            println!();
            if pull_request_candidates.is_empty() {
                println!("PR: aucun depot candidat detecte.");
            } else {
                for candidate in &pull_request_candidates {
                    println!(
                        "PR: {} -> {}",
                        candidate.repository, candidate.target_branch
                    );
                }
            }
        }
    }

    if changed.is_empty() && unpushed.is_empty() && pull_request_candidates.is_empty() {
        if !json {
            println!();
            println!("Rien a terminer.");
        }
        return Ok(());
    }
    if !execute {
        if !json {
            println!();
            println!(
                "{}",
                if !create_pr {
                    if changed.is_empty() {
                        "Dry-run uniquement. Relancer avec --execute --skip-ado pour pousser."
                    } else {
                        "Dry-run uniquement. Relancer avec --execute --skip-ado pour committer/pousser."
                    }
                } else {
                    "Dry-run uniquement. Relancer avec --execute pour pousser/creer PR."
                }
            );
        }
        return Ok(());
    }
    if create_pr && skip_ado {
        return Err(anyhow::anyhow!(
            "--create-pr ne peut pas etre combine avec --skip-ado."
        ));
    }

    if !handoff.is_valid {
        return Err(anyhow::anyhow!(
            "task finish bloque: handoff invalide. Corriger ou completer les handoffs avant push."
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
            println!("Commits/push termines.");
        }
    } else {
        for (target, _) in &unpushed {
            push_repository(&target.path, &manifest.branch_name)?;
        }
        if !json {
            println!("Push termine.");
        }
    }
    if !create_pr {
        if !json {
            println!("PR non creee. Relancer avec --create-pr pour ouvrir les PR ADO.");
        }
        return Ok(());
    }

    if !changed.is_empty() && !json {
        println!("Commits/push termines. Creation PR en cours.");
    } else if changed.is_empty() && unpushed.is_empty() && !json {
        println!("Aucun commit local a pousser. Verification PR en cours.");
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
            println!(
                "PR ignoree pour {}: azureDevOpsRepository manquant.",
                candidate.repository
            );
            continue;
        };
        if let Some(existing) = try_find_active_pull_request_authenticated(
            &options,
            ado_repository,
            &source_ref,
            &token,
        )? {
            println!(
                "PR deja ouverte pour {}: {}",
                candidate.repository,
                existing.url.as_deref().unwrap_or("(url non retournee)")
            );
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
                    println!(
                        "Lien PR/work item deja demande a la creation, lien explicite ignore pour #{}: {}",
                        id, error
                    );
                }
            }
        }
        println!(
            "PR creee pour {}: {}",
            candidate.repository,
            created.url.as_deref().unwrap_or("(url non retournee)")
        );
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
                println!(
                    "ADO item {}: etat inchange ({}).",
                    label,
                    item.kind.as_deref().unwrap_or("type inconnu")
                );
                continue;
            };
            if item
                .state
                .as_deref()
                .is_some_and(|current| current.eq_ignore_ascii_case(&state))
            {
                println!("ADO item {label}: deja en etat {state}.");
                continue;
            }
            update_work_item_state_authenticated(
                &options,
                &id,
                &state,
                "dw task finish: PR ouverte",
                &token,
            )?;
            println!("ADO item {label}: etat -> {state}");
        }
    }

    Ok(())
}

fn print_repository_status(repository: &str, status: &dw_git::RepositoryStatus) {
    println!();
    println!("[{repository}] {}", status.path);
    if !status.is_git_repository {
        println!("Pas un repo Git utilisable.");
    } else if status.has_changes {
        println!("Changements detectes:");
    } else if status.has_unpushed {
        println!("Commits non pousses.");
    } else {
        println!("Aucun changement.");
    }
    if !status.detail.trim().is_empty() {
        println!("{}", status.detail);
    }
}
