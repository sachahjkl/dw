mod finish;

use crate::cli::TaskCommand;
use crate::handlers::resolve_ado_options;
use crate::simple_handlers::load_auth_options;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    create_child_task, env_pat, get_work_item_snapshots, get_work_item_snapshots_authenticated,
    query_assigned_work_items,
};
use dw_agent::{AgentOpenRequest, build_open_launch};
use dw_config::{load_projects_config, load_workflow_config, resolve_project, resolve_root};
use dw_git::{
    WorktreePrepareRequest, commit_repository, prepare_worktree, repository_status,
    update_repository, worktree_prune, worktree_remove,
};
use dw_workspace::{
    TaskStartRequest, build_commit_message, build_handoff_validation_report,
    build_preflight_report_from_ai_context_files, display_work_items, execute_add_child_task,
    execute_task_add_repo, execute_task_rename, execute_task_start,
    execute_task_start_with_work_items, execute_task_sync, execute_task_teardown,
    execute_work_item_update, parse_work_item_ids as parse_workspace_work_item_ids,
    plan_add_work_item_snapshots, plan_add_work_items, plan_remove_work_items, plan_task_add_repo,
    plan_task_commit, plan_task_prune, plan_task_rename, plan_task_repo_latest, plan_task_start,
    plan_task_teardown, read_manifest_path, requires_child_tasks, resolve_open_target,
    resolve_workspace, resolve_workspace_for_workspace_command, task_current, task_list,
    task_status,
};
use inquire::{MultiSelect, Select, Text};
use std::io::IsTerminal;
use std::path::Path;

pub(crate) fn handle_task(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Status { root } => {
            let root = resolve_root(root.as_deref());
            let items = task_status(&root);
            println!("Root: {}", root);
            println!("Workspaces detectes:");
            if items.is_empty() {
                println!("  Aucun workspace task trouve.");
            } else {
                for item in items {
                    println!("  {}", item);
                }
            }
        }
        TaskCommand::List {
            root,
            project,
            work_item,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let items = task_list(&root, project.as_deref(), work_item.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else if items.is_empty() {
                println!("Aucun workspace task trouve.");
            } else {
                println!("Project  WorkItems  Created     Branch");
                for item in items {
                    println!(
                        "{:<8} {:<8} {}  {}",
                        item.project,
                        item.display_work_items,
                        created_date(&item.created_at),
                        item.branch_name
                    );
                    println!("  {}", item.path);
                }
            }
        }
        TaskCommand::Current { json } => {
            let current = std::env::current_dir()?.display().to_string();
            let item = task_current(&current)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&item)?);
            } else {
                println!("Workspace: {}", item.workspace);
                println!("Project: {}", item.project);
                println!(
                    "Work items: {}",
                    format_current_work_items(&item.work_items)
                );
                println!("Branch: {}", item.branch);
                println!("Repos: {}", item.repositories.join(", "));
            }
        }
        TaskCommand::Open {
            workspace,
            project,
            work_item,
            positional_work_item,
            r#continue,
            repo,
            agent,
            json,
            root,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = if workspace.is_none() && !r#continue && std::io::stdin().is_terminal()
            {
                interactive_workspace_selection(
                    &root,
                    project.as_deref(),
                    work_item.as_deref().or(positional_work_item.as_deref()),
                )?
            } else {
                resolve_workspace(
                    &root,
                    workspace.as_deref(),
                    project.as_deref(),
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                    r#continue,
                )?
            };
            let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
            let projects = load_projects_config(&root);
            let workflow = load_workflow_config(&root);
            let project_config = resolve_project(&projects, &manifest.project);
            let target = resolve_open_target(
                &workspace,
                &manifest,
                project_config.as_ref(),
                repo.as_deref(),
            )?;
            let selected_agent = agent
                .clone()
                .or_else(|| {
                    project_config.as_ref().and_then(|project| {
                        project.agent.as_ref().map(|agent| agent.default.clone())
                    })
                })
                .or_else(|| workflow.agent.as_ref().map(|agent| agent.default.clone()));
            let launch = build_open_launch(
                selected_agent.as_deref(),
                &AgentOpenRequest {
                    root: root.clone(),
                    workspace: target.clone(),
                    r#continue,
                },
            )?;

            if json {
                println!("{}", serde_json::to_string_pretty(&launch)?);
            } else {
                let mut command = std::process::Command::new(&launch.file_name);
                command
                    .args(&launch.arguments)
                    .current_dir(&launch.working_directory);
                for (key, value) in &launch.environment {
                    command.env(key, value);
                }
                let status = command.status()?;
                if !status.success() {
                    return Err(anyhow::anyhow!("agent exited with status {status}"));
                }
            }
        }
        TaskCommand::Start {
            work_item_id,
            root,
            project,
            task,
            type_name,
            only,
            slug,
            skip_ado,
            json,
            execute,
        } => {
            let root = resolve_root(root.as_deref());
            let projects = load_projects_config(&root);
            let workflow = load_workflow_config(&root);
            let project = interactive_project(project, &projects);
            let work_item_id = interactive_work_item(
                work_item_id,
                &root,
                &projects,
                &workflow,
                project.as_deref(),
                skip_ado,
            )?;
            let only = interactive_repositories(only, &projects, project.as_deref());
            let plan = plan_task_start(TaskStartRequest {
                root: &root,
                projects: &projects,
                work_item_id: &work_item_id,
                project: project.as_deref(),
                task_id: task.as_deref(),
                type_name: type_name.as_deref(),
                only: only.as_deref(),
                slug: slug.as_deref(),
            })?;

            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else if execute {
                let manifest = if skip_ado {
                    execute_task_start(&plan, None, None, None)?
                } else {
                    let mut ado_options = resolve_project(&projects, &plan.project)
                            .and_then(|project| project.azure_dev_ops)
                            .ok_or_else(|| anyhow::anyhow!("Configuration azureDevOps manquante dans projects.json pour {}. Ajouter --skip-ado pour un start offline.", plan.project))?;
                    if ado_options.project.trim().is_empty() {
                        ado_options.project = plan.project.clone();
                    }
                    let token = env_pat()?;
                    let snapshots =
                        get_work_item_snapshots(&ado_options, &plan.work_item_ids, &token)?;
                    let work_items = if snapshots.is_empty() {
                        plan.work_item_ids
                            .iter()
                            .map(|id| dw_workspace::WorkspaceWorkItem {
                                id: id.clone(),
                                kind: None,
                                title: None,
                                state: None,
                            })
                            .collect()
                    } else {
                        snapshots
                            .into_iter()
                            .map(|snapshot| dw_workspace::WorkspaceWorkItem {
                                id: snapshot.id,
                                kind: snapshot.kind,
                                title: snapshot.title,
                                state: snapshot.state,
                            })
                            .collect()
                    };
                    execute_task_start_with_work_items(&plan, work_items)?
                };
                write_agent_configs(&plan.workspace, &manifest)?;
                println!("Workspace cree: {}", plan.workspace);
                println!("Branche cible: {}", plan.branch_name);
                println!("Repos: {}", plan.repositories.join(", "));
            } else {
                println!("Project: {}", plan.project);
                println!("Work items: {}", plan.work_item_ids.join(", "));
                println!("Slug: {}", plan.slug);
                println!("Branche cible: {}", plan.branch_name);
                println!("Workspace cible: {}", plan.workspace);
                println!("Repos: {}", plan.repositories.join(", "));
            }
        }
        TaskCommand::Preflight {
            workspace,
            ai_context_file,
            json,
        } => {
            let files = if ai_context_file.is_empty() {
                discover_ai_context_files(&workspace)
            } else {
                ai_context_file
            };

            if files.is_empty() {
                return Err(anyhow::anyhow!(
                    "Aucun fichier ai-context detecte. Fournir --ai-context-file ou placer des fichiers ai-context*.json dans le workspace."
                ));
            }

            let report = build_preflight_report_from_ai_context_files(&workspace, &files)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Preflight workspace: {}", report.workspace);
                println!("Projet: {}", report.project);
                println!(
                    "Work items: {}",
                    report
                        .work_item_ids
                        .iter()
                        .map(|id| format!("#{id}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                println!();
                if report.issues.is_empty() {
                    println!("Aucun warning ni blocage detecte.");
                } else {
                    for issue in &report.issues {
                        println!("- [{}] {}: {}", issue.severity, issue.code, issue.message);
                        if let Some(details) = &issue.details {
                            println!("  {}", details);
                        }
                    }
                    if report.has_blocking_issues {
                        println!();
                        println!(
                            "Blocages detectes: demander confirmation utilisateur avant de forcer l'implementation."
                        );
                    }
                }
            }
        }
        TaskCommand::Sync {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace(
                &root,
                workspace.as_deref(),
                project.as_deref(),
                work_item.as_deref(),
                positional_work_item.as_deref(),
                r#continue,
            )?;
            let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
            let projects = load_projects_config(&root);
            let mut options = resolve_project(&projects, &manifest.project)
                .and_then(|project| project.azure_dev_ops)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Configuration azureDevOps manquante dans projects.json pour {}.",
                        manifest.project
                    )
                })?;
            if options.project.trim().is_empty() {
                options.project = manifest.project.clone();
            }
            let token = env_pat()?;
            let ids = manifest
                .parent_work_items()
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>();
            let snapshots = get_work_item_snapshots(&options, &ids, &token)?;
            let updated = execute_task_sync(&workspace, &snapshots)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&updated)?);
            } else {
                println!("Workspace synchronise: {workspace}");
                for item in updated.parent_work_items() {
                    println!(
                        "ADO item {}: {} / {} / {}",
                        item.id,
                        item.kind.unwrap_or_else(|| "?".into()),
                        item.state.unwrap_or_else(|| "?".into()),
                        item.title.unwrap_or_else(|| "(sans titre)".into())
                    );
                }
            }
        }
        TaskCommand::Rename {
            slug,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            json,
            execute,
            positional_work_item,
        } => {
            let root = resolve_root(root.as_deref());
            let projects = load_projects_config(&root);
            let workspace = resolve_workspace(
                &root,
                workspace.as_deref(),
                project.as_deref(),
                work_item.as_deref(),
                positional_work_item.as_deref(),
                r#continue,
            )?;
            let (manifest, plan) = plan_task_rename(&root, &projects, &workspace, &slug)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("Rename dry-run:");
                println!("- slug: {} -> {}", plan.old_slug, plan.new_slug);
                println!("- branch: {} -> {}", plan.old_branch, plan.new_branch);
                println!("- workspace: {} -> {}", plan.workspace, plan.new_workspace);
                if execute {
                    let _updated = execute_task_rename(&manifest, &plan)?;
                    println!("Workspace renomme: {}", plan.new_workspace);
                } else {
                    println!("Relancer avec --execute pour appliquer.");
                }
            }
        }
        TaskCommand::RepoLatest {
            workspace,
            r#continue,
            only,
            root,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace_for_workspace_command(
                &root,
                workspace.as_deref(),
                r#continue,
                &std::env::current_dir()?.display().to_string(),
            )?;
            let projects = load_projects_config(&root);
            let (manifest, targets) =
                plan_task_repo_latest(&root, &projects, &workspace, only.as_deref())?;

            if json {
                println!("{}", serde_json::to_string_pretty(&targets)?);
            } else {
                println!("Workspace: {}", workspace);
                println!("Branche: {}", manifest.branch_name);
                for target in &targets {
                    println!("Repo {}: sync latest...", target.repository);
                    update_repository(&target.repository_path, &target.default_branch)?;
                }
                println!("Repos synchronises avec la remote.");
            }
        }
        TaskCommand::Commit {
            workspace,
            r#continue,
            root,
            execute,
            message,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace_for_workspace_command(
                &root,
                workspace.as_deref(),
                r#continue,
                &std::env::current_dir()?.display().to_string(),
            )?;
            let projects = load_projects_config(&root);
            let (manifest, targets) = plan_task_commit(&projects, &workspace)?;
            let statuses = targets
                .iter()
                .map(|target| (target, repository_status(&target.path)))
                .collect::<Vec<_>>();
            let changed = statuses
                .iter()
                .filter(|(_, status)| status.is_git_repository && status.has_changes)
                .collect::<Vec<_>>();
            let commit_message = build_commit_message(&manifest, message.as_deref());

            if json {
                let report = serde_json::json!({
                    "workspace": workspace,
                    "branch": manifest.branch_name,
                    "message": commit_message,
                    "targets": statuses.iter().map(|(target, status)| serde_json::json!({
                        "repository": target.repository,
                        "path": status.path,
                        "isGitRepository": status.is_git_repository,
                        "hasChanges": status.has_changes,
                        "hasUnpushed": status.has_unpushed,
                        "detail": status.detail,
                    })).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Workspace: {workspace}");
                println!("Branche: {}", manifest.branch_name);
                for (target, status) in &statuses {
                    println!();
                    println!("[{}] {}", target.repository, status.path);
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
                if changed.is_empty() {
                    println!();
                    println!("Rien a committer.");
                } else {
                    println!();
                    println!("Message: {commit_message}");
                }
            }

            if changed.is_empty() || !execute {
                if !changed.is_empty() && !json {
                    println!("Dry-run uniquement. Relancer avec --execute pour committer.");
                }
                return Ok(());
            }

            for (target, _) in changed {
                commit_repository(&target.path, &commit_message)?;
            }
            if !json {
                println!("Commits termines. Aucun push ni PR creee.");
            }
        }
        TaskCommand::AddWorkItem {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            skip_ado,
            type_name,
            title,
            state,
            execute,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace(
                &root,
                workspace.as_deref(),
                project.as_deref(),
                work_item.as_deref(),
                positional_work_item.as_deref(),
                r#continue,
            )?;
            let current_manifest = read_manifest_path(
                &Path::new(&workspace)
                    .join("task.json")
                    .display()
                    .to_string(),
            )?;
            let requested_ids = parse_workspace_work_item_ids(&work_item_ids);
            let missing_ids = requested_ids
                .iter()
                .filter(|id| !current_manifest.matches_work_item(id))
                .cloned()
                .collect::<Vec<_>>();
            if missing_ids.is_empty() {
                if !json {
                    println!("Tous les work items demandes sont deja presents dans le workspace.");
                }
                return Ok(());
            }
            let (manifest, plan) = if skip_ado {
                plan_add_work_items(
                    &root,
                    &workspace,
                    &work_item_ids,
                    type_name.as_deref(),
                    title.as_deref(),
                    state.as_deref(),
                )?
            } else {
                let projects = load_projects_config(&root);
                let workflow = load_workflow_config(&root);
                let mut options =
                    resolve_ado_options(&projects, &workflow, &current_manifest.project)?;
                if options.project.trim().is_empty() {
                    options.project = current_manifest.project.clone();
                }
                let token = require_token(load_auth_options(Some(&root))?)?;
                let snapshots =
                    get_work_item_snapshots_authenticated(&options, &missing_ids, &token)?;
                if snapshots.len() != missing_ids.len() {
                    let found = snapshots
                        .iter()
                        .map(|snapshot| snapshot.id.clone())
                        .collect::<Vec<_>>();
                    let unresolved = missing_ids
                        .iter()
                        .filter(|id| {
                            !found
                                .iter()
                                .any(|found_id| found_id.eq_ignore_ascii_case(id))
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    return Err(anyhow::anyhow!(
                        "Work items ADO introuvables ou inaccessibles: {}",
                        unresolved.join(", ")
                    ));
                }
                let final_items = snapshots
                    .iter()
                    .filter(|snapshot| {
                        dw_workspace::is_final_state(
                            snapshot.kind.as_deref(),
                            snapshot.state.as_deref(),
                        )
                    })
                    .collect::<Vec<_>>();
                if !final_items.is_empty() {
                    let labels = final_items
                        .iter()
                        .map(|item| {
                            format!(
                                "#{} ({})",
                                item.id,
                                item.state.as_deref().unwrap_or("etat inconnu")
                            )
                        })
                        .collect::<Vec<_>>();
                    return Err(anyhow::anyhow!(
                        "Impossible d'ajouter des work items en etat final: {}",
                        labels.join(", ")
                    ));
                }
                plan_add_work_item_snapshots(&root, &workspace, &snapshots)?
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_work_item_update_plan("Add work-item", &plan);
                if !skip_ado {
                    println!("Work items ADO resolus:");
                    println!("{}", display_work_items(&plan.work_items, true));
                }
            }
            if execute {
                let (updated, new_workspace) = execute_work_item_update(&manifest, &plan)?;
                write_agent_configs(&new_workspace, &updated)?;
                if !json {
                    println!("Workspace mis a jour: {new_workspace}");
                }
            } else if !json {
                println!("Relancer avec --execute pour appliquer.");
            }
        }
        TaskCommand::RemoveWorkItem {
            work_item_ids,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace(
                &root,
                workspace.as_deref(),
                project.as_deref(),
                work_item.as_deref(),
                positional_work_item.as_deref(),
                r#continue,
            )?;
            let (manifest, plan) = plan_remove_work_items(&root, &workspace, &work_item_ids)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_work_item_update_plan("Remove work-item", &plan);
            }
            if execute {
                let (updated, new_workspace) = execute_work_item_update(&manifest, &plan)?;
                write_agent_configs(&new_workspace, &updated)?;
                if !json {
                    println!("Workspace mis a jour: {new_workspace}");
                }
            } else if !json {
                println!("Relancer avec --execute pour appliquer.");
            }
        }
        TaskCommand::AddRepo {
            repo,
            workspace,
            root,
            execute,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace_for_workspace_command(
                &root,
                workspace.as_deref(),
                false,
                &std::env::current_dir()?.display().to_string(),
            )?;
            let projects = load_projects_config(&root);
            let (manifest, plan) = plan_task_add_repo(&root, &projects, &workspace, &repo)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                println!("Add repo dry-run:");
                println!("- workspace: {}", plan.workspace);
                println!("- repo: {}", plan.repository);
                println!("- worktree: {}", plan.worktree_path);
                println!("- branche: {}", plan.branch_name);
                println!(
                    "- anchor: {}/repositories/{}",
                    plan.project_root, plan.anchor_name
                );
            }

            if !execute {
                if !json {
                    println!("Relancer avec --execute pour appliquer.");
                }
                return Ok(());
            }

            let result = prepare_worktree(&WorktreePrepareRequest {
                project_root: plan.project_root.clone(),
                repository: plan.repository.clone(),
                url: plan.url.clone(),
                default_branch: plan.default_branch.clone(),
                anchor_name: plan.anchor_name.clone(),
                branch_name: plan.branch_name.clone(),
                worktree_path: plan.worktree_path.clone(),
            })?;
            let updated = execute_task_add_repo(&manifest, &plan)?;
            write_agent_configs(&workspace, &updated)?;
            if !json {
                println!(
                    "Repo ajoute: {} - {} - {}",
                    result.repository, result.status, result.message
                );
            }
        }
        TaskCommand::CreateChildTask {
            repo,
            title,
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace(
                &root,
                workspace.as_deref(),
                project.as_deref(),
                work_item.as_deref(),
                positional_work_item.as_deref(),
                r#continue,
            )?;
            let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
            let parent = manifest.parent_work_items()[0].clone();
            if !requires_child_tasks(parent.kind.as_deref()) {
                return Err(anyhow::anyhow!(
                    "Cette commande est reservee aux User Story et Anomalie."
                ));
            }
            let projects = load_projects_config(&root);
            let workflow = load_workflow_config(&root);
            let mut options = resolve_ado_options(&projects, &workflow, &manifest.project)?;
            if options.project.trim().is_empty() {
                options.project = manifest.project.clone();
            }
            let token = env_pat()?;
            let task_title = child_task_title(&repo, &title);
            let result = create_child_task(
                &options,
                &dw_ado::WorkItemSnapshot {
                    id: parent.id.clone(),
                    kind: parent.kind.clone(),
                    state: parent.state.clone(),
                    title: parent.title.clone(),
                    url: None,
                },
                &repo,
                &task_title,
                "dw task create-child-task",
                &token,
            )?;
            let updated =
                execute_add_child_task(&workspace, &repo, &result.id, Some(result.title.clone()))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&updated)?);
            } else {
                println!(
                    "Sous-tache enregistree dans le workspace: {} -> #{} {}",
                    repo, result.id, result.title
                );
            }
        }
        TaskCommand::Finish {
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
        } => finish::handle(finish::FinishArgs {
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
        })?,
        TaskCommand::HandoffValidate { workspace, json } => {
            let report = build_handoff_validation_report(&workspace)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Handoff validation: {}", report.workspace);
                println!("Projet: {}", report.project);
                println!();
                for item in &report.items {
                    println!("- [{}] {}: {}", item.status, item.repository, item.message);
                    if item.valid {
                        println!(
                            "  done={} decisions={} risks={} blockers={} follow_up={}",
                            item.done_count,
                            item.decision_count,
                            item.risk_count,
                            item.blocker_count,
                            item.follow_up_count
                        );
                    }
                }

                if !report.is_valid {
                    println!();
                    println!(
                        "Validation handoff echouee: completer/corriger les handoffs avant task finish."
                    );
                }
            }
        }
        TaskCommand::Teardown {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            positional_work_item,
            execute,
            yes,
            json,
        } => {
            let root = resolve_root(root.as_deref());
            let workspace = resolve_workspace(
                &root,
                workspace.as_deref(),
                project.as_deref(),
                work_item.as_deref(),
                positional_work_item.as_deref(),
                r#continue,
            )?;
            let projects = load_projects_config(&root);
            let (_manifest, steps) = plan_task_teardown(&root, &projects, &workspace)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&steps)?);
            } else {
                println!("Workspace: {workspace}");
                println!(
                    "{}",
                    if execute {
                        "Teardown execute:"
                    } else {
                        "Teardown dry-run:"
                    }
                );
                for step in &steps {
                    println!("- [{}] {}: {}", step.repository, step.action, step.target);
                }
            }

            if !execute {
                if !json {
                    println!();
                    println!(
                        "Dry-run uniquement. Relancer avec --execute --yes pour supprimer les worktrees et le workspace."
                    );
                }
                return Ok(());
            }

            if !yes {
                return Err(anyhow::anyhow!(
                    "Suppression destructive refusee: ajouter --yes avec --execute."
                ));
            }

            execute_task_teardown(&workspace, &steps, |git_dir, args| match args {
                ["worktree", "remove", "--force", target] => {
                    worktree_remove(git_dir, target).map_err(|error| error.to_string())
                }
                ["worktree", "prune"] => worktree_prune(git_dir).map_err(|error| error.to_string()),
                _ => Err(format!("commande git non supportee: {}", args.join(" "))),
            })?;
            if !json {
                println!("Workspace supprime: {workspace}");
            }
        }
        TaskCommand::Prune {
            root,
            project,
            work_item,
            execute,
            yes,
            no_sync,
            json,
        } => {
            if !no_sync {
                return Err(anyhow::anyhow!(
                    "Rust task prune exige --no-sync tant que le sync ADO reel n'est pas porte."
                ));
            }
            let root = resolve_root(root.as_deref());
            let candidates = plan_task_prune(&root, project.as_deref(), work_item.as_deref());
            if json {
                println!("{}", serde_json::to_string_pretty(&candidates)?);
            } else if candidates.is_empty() {
                println!("Aucun workspace eligible au prune.");
            } else {
                for candidate in &candidates {
                    println!(
                        "{} / {}: {}",
                        candidate.manifest.project,
                        display_work_items(&candidate.manifest.parent_work_items(), true),
                        candidate.path
                    );
                }
            }

            if candidates.is_empty() || !execute {
                if !candidates.is_empty() && !json {
                    println!();
                    println!(
                        "Dry-run uniquement. Relancer avec --execute --yes --no-sync pour supprimer les workspaces eligibles."
                    );
                }
                return Ok(());
            }
            if !yes {
                return Err(anyhow::anyhow!(
                    "Suppression destructive refusee: ajouter --yes avec --execute."
                ));
            }

            let projects = load_projects_config(&root);
            for candidate in candidates {
                let (_manifest, steps) = plan_task_teardown(&root, &projects, &candidate.path)?;
                execute_task_teardown(&candidate.path, &steps, |git_dir, args| match args {
                    ["worktree", "remove", "--force", target] => {
                        worktree_remove(git_dir, target).map_err(|error| error.to_string())
                    }
                    ["worktree", "prune"] => {
                        worktree_prune(git_dir).map_err(|error| error.to_string())
                    }
                    _ => Err(format!("commande git non supportee: {}", args.join(" "))),
                })?;
                if !json {
                    println!("Workspace supprime: {}", candidate.path);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkItemChoice {
    id: String,
    label: String,
}

impl std::fmt::Display for WorkItemChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

fn created_date(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}

pub(crate) struct OpenWorkspaceArgs {
    pub(crate) workspace: Option<String>,
    pub(crate) project: Option<String>,
    pub(crate) work_item: Option<String>,
    pub(crate) positional_work_item: Option<String>,
    pub(crate) r#continue: bool,
    pub(crate) repo: Option<String>,
    pub(crate) agent: Option<String>,
    pub(crate) json: bool,
    pub(crate) root: Option<String>,
}

pub(crate) fn open_workspace(args: OpenWorkspaceArgs) -> Result<()> {
    let OpenWorkspaceArgs {
        workspace,
        project,
        work_item,
        positional_work_item,
        r#continue,
        repo,
        agent,
        json,
        root,
    } = args;
    let root = resolve_root(root.as_deref());
    let workspace = if workspace.is_none() && !r#continue && std::io::stdin().is_terminal() {
        interactive_workspace_selection(
            &root,
            project.as_deref(),
            work_item.as_deref().or(positional_work_item.as_deref()),
        )?
    } else {
        resolve_workspace(
            &root,
            workspace.as_deref(),
            project.as_deref(),
            work_item.as_deref(),
            positional_work_item.as_deref(),
            r#continue,
        )?
    };
    let manifest = read_manifest_path(&format!("{workspace}/task.json"))?;
    let projects = load_projects_config(&root);
    let workflow = load_workflow_config(&root);
    let project_config = resolve_project(&projects, &manifest.project);
    let target = resolve_open_target(
        &workspace,
        &manifest,
        project_config.as_ref(),
        repo.as_deref(),
    )?;
    let selected_agent = agent
        .or_else(|| {
            project_config
                .as_ref()
                .and_then(|project| project.agent.as_ref().map(|agent| agent.default.clone()))
        })
        .or_else(|| workflow.agent.as_ref().map(|agent| agent.default.clone()));
    let launch = build_open_launch(
        selected_agent.as_deref(),
        &AgentOpenRequest {
            root,
            workspace: target,
            r#continue,
        },
    )?;

    if json {
        println!("{}", serde_json::to_string_pretty(&launch)?);
        return Ok(());
    }

    let mut command = std::process::Command::new(&launch.file_name);
    command
        .args(&launch.arguments)
        .current_dir(&launch.working_directory);
    for (key, value) in &launch.environment {
        command.env(key, value);
    }
    let status = command.status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("agent exited with status {status}"));
    }
    Ok(())
}

fn format_current_work_items(items: &[dw_workspace::WorkspaceWorkItem]) -> String {
    items
        .iter()
        .map(|item| {
            let title = item.title.clone().unwrap_or_else(|| "(sans titre)".into());
            format!("#{} {}", item.id, title)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn child_task_title(repository: &str, title: &str) -> String {
    let normalized = repository.to_ascii_lowercase();
    let prefix = match normalized.as_str() {
        "front" => "FRONT",
        "back" => "BACK",
        "sql" | "db" | "database" => "SQL",
        other => other,
    };
    format!("[{}] {}", prefix.to_ascii_uppercase(), title)
}

fn interactive_project(
    project: Option<String>,
    projects: &dw_config::ProjectsConfig,
) -> Option<String> {
    if project.is_some() || !std::io::stdin().is_terminal() {
        return project;
    }

    let options = projects.projects.keys().cloned().collect::<Vec<_>>();
    if options.is_empty() {
        return None;
    }

    Select::new("Projet", options).prompt().ok()
}

fn interactive_work_item(
    work_item_id: Option<String>,
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project: Option<&str>,
    skip_ado: bool,
) -> Result<String> {
    if let Some(work_item_id) = work_item_id.filter(|value| !value.trim().is_empty()) {
        return Ok(work_item_id);
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "work-item-id requis en mode non interactif"
        ));
    }

    if !skip_ado && let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        match interactive_assigned_work_item_selection(root, projects, workflow, project) {
            Ok(Some(selection)) => return Ok(selection),
            Ok(None) => {}
            Err(error) => {
                println!("Selection ADO indisponible: {error}");
                println!("Saisie manuelle du work item.");
            }
        }
    }

    Ok(Text::new("Work item ID").prompt()?)
}

fn interactive_assigned_work_item_selection(
    root: &str,
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project: &str,
) -> Result<Option<String>> {
    let options = resolve_ado_options(projects, workflow, project)?;
    let token = require_token(load_auth_options(Some(root))?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let items = runtime.block_on(query_assigned_work_items(&options, 50, &token))?;
    let items = items
        .into_iter()
        .filter(|item| !dw_workspace::is_final_state(item.kind.as_deref(), item.state.as_deref()))
        .collect::<Vec<_>>();
    let choices = work_item_choices(&items);
    if choices.is_empty() {
        println!("Aucun work item assigne hors etats finaux pour {project}.");
        return Ok(None);
    }

    let selected = MultiSelect::new("Work items assignes", choices).prompt()?;
    if selected.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        selected
            .into_iter()
            .map(|choice| choice.id)
            .collect::<Vec<_>>()
            .join(","),
    ))
}

fn work_item_choices(items: &[dw_ado::WorkItemSnapshot]) -> Vec<WorkItemChoice> {
    items
        .iter()
        .map(|item| WorkItemChoice {
            id: item.id.clone(),
            label: format_work_item_choice(item),
        })
        .collect()
}

fn format_work_item_choice(item: &dw_ado::WorkItemSnapshot) -> String {
    let mut label = format!("#{}", item.id);
    if let Some(kind) = item
        .kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(&format!(" [{kind}]"));
    }
    if let Some(state) = item
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(&format!(" {state}"));
    }
    if let Some(title) = item
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(&format!(" - {title}"));
    }
    label
}

fn interactive_repositories(
    only: Option<String>,
    projects: &dw_config::ProjectsConfig,
    project: Option<&str>,
) -> Option<String> {
    if only.is_some() || !std::io::stdin().is_terminal() {
        return only;
    }

    let project = project?;
    let project_config = resolve_project(projects, project)?;
    let options = project_config
        .repositories
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if options.len() <= 1 {
        return None;
    }

    let selected = MultiSelect::new("Repos", options).prompt().ok()?;
    if selected.is_empty() {
        None
    } else {
        Some(selected.join(","))
    }
}

fn interactive_workspace_selection(
    root: &str,
    project: Option<&str>,
    work_item: Option<&str>,
) -> Result<String> {
    let items = task_list(root, project, work_item);
    if items.is_empty() {
        return Err(anyhow::anyhow!("Aucun workspace task trouve."));
    }
    if items.len() == 1 {
        return Ok(items[0].path.clone());
    }

    let options = items
        .into_iter()
        .map(|item| {
            (
                format!(
                    "{} / {} / {} / {}",
                    item.project, item.display_work_items, item.kind, item.path
                ),
                item.path,
            )
        })
        .collect::<Vec<_>>();
    let labels = options
        .iter()
        .map(|(label, _)| label.clone())
        .collect::<Vec<_>>();
    let selected = Select::new("Workspace", labels).prompt()?;
    options
        .into_iter()
        .find(|(label, _)| *label == selected)
        .map(|(_, path)| path)
        .ok_or_else(|| anyhow::anyhow!("Workspace selection invalide"))
}

fn print_work_item_update_plan(label: &str, plan: &dw_workspace::TaskWorkItemUpdatePlan) {
    println!("{label} dry-run:");
    println!("- branch: {} -> {}", plan.old_branch, plan.new_branch);
    println!("- workspace: {} -> {}", plan.workspace, plan.new_workspace);
    println!(
        "- work items: {}",
        plan.work_items
            .iter()
            .map(|item| format!("#{}", item.id))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

fn write_agent_configs(workspace: &str, manifest: &dw_workspace::WorkspaceManifest) -> Result<()> {
    let config_files = dw_agent::workspace_config_files(&dw_agent::AgentWorkspaceConfigRequest {
        workspace: workspace.into(),
        work_items: manifest
            .parent_work_items()
            .into_iter()
            .map(|item| dw_agent::WorkspaceWorkItemRef {
                id: item.id,
                kind: item.kind,
                title: item.title,
            })
            .collect(),
        project: manifest.project.clone(),
    });
    for file in config_files {
        let path = std::path::Path::new(workspace).join(file.relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.content)?;
    }
    Ok(())
}

fn discover_ai_context_files(workspace: &str) -> Vec<String> {
    let mut files = Vec::new();
    collect_ai_context_files(Path::new(workspace), &mut files);
    files.sort();
    files
}

fn collect_ai_context_files(root: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ai_context_files(&path, files);
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("ai-context") && name.ends_with(".json") {
            files.push(path.display().to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_choices_include_id_type_state_and_title() {
        let items = vec![dw_ado::WorkItemSnapshot {
            id: "53115".into(),
            kind: Some("Bug".into()),
            state: Some("En développement".into()),
            title: Some("Corriger le calcul".into()),
            url: None,
        }];

        let choices = work_item_choices(&items);

        assert_eq!(
            choices,
            vec![WorkItemChoice {
                id: "53115".into(),
                label: "#53115 [Bug] En développement - Corriger le calcul".into()
            }]
        );
    }
}
