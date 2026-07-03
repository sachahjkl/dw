use crate::cli::*;
use crate::simple_handlers::{
    handle_auth, handle_completion, handle_config, handle_secret, handle_upgrade, load_auth_options,
};
use crate::support::{unsupported_command, unsupported_command_with_args};
use crate::version::informational_version;
use anyhow::Result;
use dw_ado::auth::require_token;
use dw_ado::{
    AzureDevOpsOptions, create_child_task, default_api_version, env_pat,
    extract_work_item_ids_from_commit_messages, get_ai_context,
    get_work_item_ids_from_pull_requests, get_work_item_snapshots, group_work_items_by_parent,
    load_changelog_items, parse_changelog_format, query_assigned_work_items,
    query_work_item_snapshots, render_flat_changelog, render_grouped_changelog,
};
use dw_agent::{AgentOpenRequest, agent_context, build_open_launch};
use dw_config::{
    InitRequest, RefreshRequest, default_agent, init_root, load_projects_config,
    load_workflow_config, refresh_root, resolve_project, resolve_root, set_default_agent,
};
use dw_db::{
    DatabaseSelection, describe_table_sql, query_sql_server, render_query_result_tsv,
    resolve_connection as resolve_db_connection, schema_sql, validate_read_only_sql,
};
use dw_git::{
    WorktreePrepareRequest, commit_repository, prepare_worktree, push_repository,
    repository_status, update_repository, worktree_prune, worktree_remove,
};
use dw_workspace::{
    TaskStartRequest, build_commit_message, build_handoff_validation_report,
    build_preflight_report_from_ai_context_files, display_work_items, execute_add_child_task,
    execute_task_add_repo, execute_task_rename, execute_task_start,
    execute_task_start_with_work_items, execute_task_sync, execute_task_teardown,
    execute_work_item_update, plan_add_work_items, plan_remove_work_items, plan_task_add_repo,
    plan_task_commit, plan_task_finish, plan_task_prune, plan_task_rename, plan_task_repo_latest,
    plan_task_start, plan_task_teardown, read_manifest_path, requires_child_tasks,
    resolve_open_target, resolve_workspace, resolve_workspace_for_workspace_command, task_current,
    task_list, task_status,
};
use inquire::{MultiSelect, Select, Text};
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command as ProcessCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectChoice {
    key: String,
    label: String,
}

impl std::fmt::Display for ProjectChoice {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
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

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            println!("dw {}", informational_version());
        }
        Command::Guide => {
            println!(
                "dw - Dev Workflow {}\nDemarrer avec `dw init`, puis `dw task start <work-item-id>`.",
                informational_version()
            );
        }
        Command::Doctor { fix } => unsupported_command("doctor", fix)?,
        Command::Init {
            profile,
            root,
            dry_run,
            no_save,
        } => {
            let report = init_root(InitRequest {
                root,
                profile,
                no_save,
                dry_run,
            })?;
            if report.dry_run {
                println!("Dry-run init DevWorkflow: {}", report.root);
                println!("Profil: {}", report.profile);
                for path in &report.planned_paths {
                    println!("  would create/write: {path}");
                }
                if report.no_save {
                    println!("  would not modify user settings (--no-save).");
                } else {
                    println!("  would save user root: {}", report.root);
                }
            } else {
                println!("Root DevWorkflow initialise: {}", report.root);
                println!("Profil: {}", report.profile);
                if report.no_save {
                    println!("Settings utilisateur non modifies (--no-save).");
                }
                println!("Prochaine etape conseillee: dw doctor");
            }
        }
        Command::Refresh { root, profile } => {
            let root = resolve_root(root.as_deref());
            let report = refresh_root(RefreshRequest {
                root,
                profile: Some(profile),
            })?;
            println!("Root rafraichi: {}", report.root);
            println!("Profil: {}", report.profile);
            println!("Schemas et contextes agents regeneres.");
            println!(
                "Fichiers utilisateurs preserves: projects.json, workflow.json, databases.json, plan.md."
            );
        }
        Command::Agent { command } => match command {
            AgentCommand::Context => {
                let root = resolve_root(None);
                println!("{}", agent_context(&root));
            }
            AgentCommand::Open {
                workspace,
                project,
                work_item,
                positional_work_item,
                r#continue,
                repo,
                agent,
                root,
            } => open_workspace(OpenWorkspaceArgs {
                workspace,
                project,
                work_item,
                positional_work_item,
                r#continue,
                repo,
                agent,
                json: false,
                root,
            })?,
            AgentCommand::Config { root } | AgentCommand::Show { root } => {
                let root = resolve_root(root.as_deref());
                println!("Agent par defaut: {}", default_agent(&root));
            }
            AgentCommand::SetDefault { root, agent } => {
                let root = resolve_root(root.as_deref());
                let agent = set_default_agent(&root, &agent)?;
                println!("Agent par defaut: {agent}");
            }
            AgentCommand::Doctor { agent } => {
                unsupported_command_with_args("agent doctor", &[("agent", agent.as_deref())])?
            }
        },
        Command::Auth { command } => handle_auth(command)?,
        Command::Completion { command } => handle_completion(command)?,
        Command::Config { command } => handle_config(command)?,
        Command::Ado { command } => match command {
            AdoCommand::Assigned {
                root,
                project,
                top,
                all,
                group_by_parent,
                json,
            } => {
                let root = resolve_root(root.as_deref());
                let projects = load_projects_config(&root);
                let project_key =
                    resolve_project_key_or_prompt(project, &projects, "ado assigned")?;
                let workflow = load_workflow_config(&root);
                let options = resolve_ado_options(&projects, &workflow, &project_key)?;
                let token = require_token(load_auth_options(Some(&root))?)?;
                let runtime = tokio::runtime::Runtime::new()?;
                let items = runtime.block_on(query_assigned_work_items(
                    &options,
                    top.try_into().unwrap_or(20),
                    &token,
                ))?;
                let items = items
                    .into_iter()
                    .filter(|item| {
                        all || !dw_workspace::is_final_state(
                            item.kind.as_deref(),
                            item.state.as_deref(),
                        )
                    })
                    .collect::<Vec<_>>();
                if group_by_parent {
                    print_assigned_items_grouped(
                        &options,
                        &items,
                        &token,
                        &project_key,
                        all,
                        json,
                    )?;
                } else {
                    print_assigned_items(&items, &project_key, all, json)?;
                }
            }
            AdoCommand::Changelog {
                ids,
                root,
                project,
                from_pr,
                from_git,
                repo,
                group_by_parent,
                format,
                table,
                ids_only,
                git_to,
            } => {
                if from_pr && from_git {
                    return Err(anyhow::anyhow!(
                        "Choisir soit --from-pr, soit --from-git, pas les deux."
                    ));
                }
                let output_format = parse_changelog_format(format.as_deref())?;
                if table && output_format != dw_ado::ChangelogFormat::Markdown {
                    return Err(anyhow::anyhow!(
                        "L'option --table est uniquement disponible avec --format markdown."
                    ));
                }
                if ids_only && table {
                    return Err(anyhow::anyhow!(
                        "Les options --ids-only et --table ne peuvent pas etre combinees."
                    ));
                }

                let root = resolve_root(root.as_deref());
                let project_key = project.ok_or_else(|| {
                    anyhow::anyhow!("ado changelog requiert --project configure.")
                })?;
                let projects = load_projects_config(&root);
                let workflow = load_workflow_config(&root);
                let options = resolve_ado_options(&projects, &workflow, &project_key)?;
                let token = require_token(load_auth_options(Some(&root))?)?;

                let work_item_ids = if from_git {
                    extract_work_item_ids_from_git_range(&ids, git_to.as_deref())?
                } else {
                    let project_config = resolve_project(&projects, &project_key);
                    let repositories =
                        resolve_ado_repositories(project_config.as_ref(), repo.as_deref());
                    get_work_item_ids_from_pull_requests(&options, &repositories, &ids, &token)?
                };

                if work_item_ids.is_empty() {
                    println!(
                        "{}",
                        if from_git {
                            "Aucun work item detecte dans les messages de commit de la plage git."
                        } else {
                            "Aucun work item detecte pour les pull requests donnees."
                        }
                    );
                    return Ok(());
                }

                if ids_only {
                    println!("{}", work_item_ids.join(" "));
                    return Ok(());
                }

                let mut items = load_changelog_items(&options, &work_item_ids, &token)?;
                if items.is_empty() {
                    println!("Aucun work item resolu dans Azure DevOps.");
                    return Ok(());
                }

                if group_by_parent {
                    let groups = group_work_items_by_parent(&options, &items, &token)?;
                    println!(
                        "{}",
                        render_grouped_changelog(&groups, output_format, &options, table)
                    );
                } else {
                    items.sort_by(|left, right| left.id.cmp(&right.id));
                    println!(
                        "{}",
                        render_flat_changelog(&items, output_format, &options, table)
                    );
                }
            }
            AdoCommand::WorkItem {
                id,
                root,
                project,
                json,
            } => {
                let root = resolve_root(root.as_deref());
                let project_key = project.ok_or_else(|| {
                    anyhow::anyhow!("ado work-item requiert --project configure.")
                })?;
                let projects = load_projects_config(&root);
                let workflow = load_workflow_config(&root);
                let options = resolve_ado_options(&projects, &workflow, &project_key)?;
                let token = require_token(load_auth_options(Some(&root))?)?;
                let ids = parse_work_item_ids(&id)?;
                let runtime = tokio::runtime::Runtime::new()?;
                let items = runtime.block_on(query_work_item_snapshots(&options, &ids, &token))?;
                print_work_item_snapshots(&items, &project_key, json)?;
            }
            AdoCommand::Context {
                id,
                root,
                project,
                summary,
                comments,
                json,
            } => {
                let root = resolve_root(root.as_deref());
                let project_key = project
                    .ok_or_else(|| anyhow::anyhow!("ado context requiert --project configure."))?;
                let projects = load_projects_config(&root);
                let workflow = load_workflow_config(&root);
                let options = resolve_ado_options(&projects, &workflow, &project_key)?;
                let token = require_token(load_auth_options(Some(&root))?)?;
                let ids = parse_work_item_ids_as_strings(&id)?;
                if json {
                    let payloads = ids
                        .iter()
                        .map(|item_id| dw_ado::get_work_item_expanded(&options, item_id, &token))
                        .collect::<Result<Vec<_>, _>>()?;
                    println!("{}", serde_json::to_string_pretty(&payloads)?);
                } else {
                    let items = ids
                        .iter()
                        .map(|item_id| dw_ado::get_ai_context(&options, item_id, summary, &token))
                        .collect::<Result<Vec<_>, _>>()?;
                    print_context_items(&items, comments, &project_key);
                }
            }
            AdoCommand::AiContext {
                root,
                organization,
                project,
                id,
                summary,
                comments: _,
                include_comments,
            } => {
                let root = resolve_root(root.as_deref());
                let options = match (organization, project) {
                    (Some(organization), Some(project)) => AzureDevOpsOptions {
                        organization,
                        project,
                        api_version: default_api_version(),
                    },
                    (None, Some(project)) => {
                        let projects = load_projects_config(&root);
                        let workflow = load_workflow_config(&root);
                        resolve_ado_options(&projects, &workflow, &project)?
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "ado ai-context requiert --project configure ou --organization avec --project."
                        ));
                    }
                };
                let token = require_token(load_auth_options(Some(&root))?)?;
                let contexts = parse_work_item_ids_as_strings(&id)?
                    .iter()
                    .map(|item_id| get_ai_context(&options, item_id, summary, &token))
                    .map(|context| {
                        context.map(|context| {
                            if include_comments {
                                context
                            } else {
                                dw_contracts::AdoAiContextItem {
                                    comments: vec![],
                                    ..context
                                }
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                println!("{}", serde_json::to_string_pretty(&contexts)?);
            }
        },
        Command::Db { command } => match command {
            DbCommand::Guard { sql } => {
                let result = validate_read_only_sql(&sql);
                if result.is_allowed {
                    println!("SQL autorisee.");
                } else {
                    println!(
                        "SQL bloquee: {}",
                        result.reason.unwrap_or_else(|| "raison inconnue".into())
                    );
                }
            }
            DbCommand::Schema { project, json } => {
                let result =
                    execute_db_query(project.as_deref(), None, None, schema_sql(), Some(0))?;
                print_db_result(&result, json)?;
            }
            DbCommand::Describe {
                project,
                database,
                table,
                json,
            } => {
                let sql = describe_table_sql(&table);
                let result =
                    execute_db_query(project.as_deref(), database.as_deref(), None, &sql, Some(0))?;
                print_db_result(&result, json)?;
            }
            DbCommand::Query {
                project,
                database,
                env,
                sql,
                json,
            } => {
                let result = execute_db_query(
                    project.as_deref(),
                    database.as_deref(),
                    env.as_deref(),
                    &sql,
                    None,
                )?;
                print_db_result(&result, json)?;
            }
        },
        Command::Secret { command } => handle_secret(command)?,
        Command::Upgrade { check, rid } => handle_upgrade(check, rid)?,
        Command::Task { command } => match command {
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
                let workspace =
                    if workspace.is_none() && !r#continue && std::io::stdin().is_terminal() {
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
                if !skip_ado {
                    return Err(anyhow::anyhow!(
                        "Rust task add-work-item exige --skip-ado tant que le client ADO reel n'est pas porte."
                    ));
                }
                let root = resolve_root(root.as_deref());
                let workspace = resolve_workspace(
                    &root,
                    workspace.as_deref(),
                    project.as_deref(),
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                    r#continue,
                )?;
                let (manifest, plan) = plan_add_work_items(
                    &root,
                    &workspace,
                    &work_item_ids,
                    type_name.as_deref(),
                    title.as_deref(),
                    state.as_deref(),
                )?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&plan)?);
                } else {
                    print_work_item_update_plan("Add work-item", &plan);
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
                let updated = execute_add_child_task(
                    &workspace,
                    &repo,
                    &result.id,
                    Some(result.title.clone()),
                )?;
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
                ready: _,
                skip_verify: _,
                skip_ado,
                json,
            } => {
                if create_pr {
                    return Err(anyhow::anyhow!(
                        "Rust task finish ne cree pas encore les PR ADO. Relancer sans --create-pr ou utiliser le binaire .NET pendant le port ADO."
                    ));
                }
                if !skip_ado {
                    return Err(anyhow::anyhow!(
                        "Rust task finish exige --skip-ado tant que les transitions ADO de fin ne sont pas portees."
                    ));
                }
                let root = resolve_root(root.as_deref());
                let workspace = resolve_workspace_for_workspace_command(
                    &root,
                    workspace.as_deref(),
                    r#continue,
                    &std::env::current_dir()?.display().to_string(),
                )?;
                let projects = load_projects_config(&root);
                let (manifest, targets, handoff) = plan_task_finish(&projects, &workspace)?;
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
                }

                if changed.is_empty() && unpushed.is_empty() {
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
                            if changed.is_empty() {
                                "Dry-run uniquement. Relancer avec --execute --skip-ado pour pousser."
                            } else {
                                "Dry-run uniquement. Relancer avec --execute --skip-ado pour committer/pousser."
                            }
                        );
                    }
                    return Ok(());
                }

                if !handoff.is_valid {
                    return Err(anyhow::anyhow!(
                        "task finish bloque: handoff invalide. Corriger ou completer les handoffs avant push."
                    ));
                }
                if !changed.is_empty() {
                    for (target, _) in changed {
                        commit_repository(&target.path, &commit_message)?;
                        push_repository(&target.path, &manifest.branch_name)?;
                    }
                    if !json {
                        println!("Commits/push termines.");
                    }
                } else {
                    for (target, _) in unpushed {
                        push_repository(&target.path, &manifest.branch_name)?;
                    }
                    if !json {
                        println!("Push termine.");
                    }
                }
                if !json {
                    println!(
                        "PR non creee. Relancer plus tard avec --create-pr quand ADO Rust sera porte."
                    );
                }
            }
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
                    ["worktree", "prune"] => {
                        worktree_prune(git_dir).map_err(|error| error.to_string())
                    }
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
        },
    }

    Ok(())
}

fn created_date(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
}

struct OpenWorkspaceArgs {
    workspace: Option<String>,
    project: Option<String>,
    work_item: Option<String>,
    positional_work_item: Option<String>,
    r#continue: bool,
    repo: Option<String>,
    agent: Option<String>,
    json: bool,
    root: Option<String>,
}

fn open_workspace(args: OpenWorkspaceArgs) -> Result<()> {
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

fn resolve_ado_options(
    projects: &dw_config::ProjectsConfig,
    workflow: &dw_config::WorkflowConfig,
    project_key: &str,
) -> Result<AzureDevOpsOptions> {
    let workflow_options = workflow
        .azure_dev_ops
        .clone()
        .and_then(|value| serde_json::from_value::<AzureDevOpsOptions>(value).ok());
    let project_options =
        resolve_project(projects, project_key).and_then(|project| project.azure_dev_ops);

    match (workflow_options, project_options) {
        (Some(workflow), Some(project)) => Ok(AzureDevOpsOptions {
            organization: if project.organization.trim().is_empty() {
                workflow.organization
            } else {
                project.organization
            },
            project: if project.project.trim().is_empty() {
                workflow.project
            } else {
                project.project
            },
            api_version: if project.api_version.trim().is_empty() {
                workflow.api_version
            } else {
                project.api_version
            },
        }),
        (Some(options), None) | (None, Some(options)) => Ok(options),
        (None, None) => Err(anyhow::anyhow!(
            "Configuration azureDevOps manquante pour {}.",
            project_key
        )),
    }
}

fn execute_db_query(
    project: Option<&str>,
    database: Option<&str>,
    env: Option<&str>,
    sql: &str,
    max_rows_override: Option<usize>,
) -> Result<dw_db::QueryResult> {
    let guard = validate_read_only_sql(sql);
    if !guard.is_allowed {
        return Err(anyhow::anyhow!(
            "Requete bloquee: {}",
            guard.reason.unwrap_or_else(|| "raison inconnue".into())
        ));
    }
    let project = project.unwrap_or("default");
    let database = database.or(env).unwrap_or("dev");
    let root = resolve_root(None);
    let config = dw_config::load_databases_config(&root);
    let resolved = resolve_db_connection(&config, DatabaseSelection { project, database })
        .map_err(anyhow::Error::msg)?;
    query_sql_server(
        &resolved.connection,
        &resolved.defaults,
        sql,
        max_rows_override,
    )
    .map_err(anyhow::Error::msg)
}

fn print_db_result(result: &dw_db::QueryResult, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        println!("{}", render_query_result_tsv(result));
    }
    Ok(())
}

fn resolve_project_key_or_prompt(
    project: Option<String>,
    projects: &dw_config::ProjectsConfig,
    command_name: &str,
) -> Result<String> {
    if let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        return Ok(project);
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "{command_name} requiert --project configure en mode non-interactif."
        ));
    }

    let choices = project_choices(projects);
    if choices.is_empty() {
        return Err(anyhow::anyhow!(
            "Aucun projet configure dans projects.json. Executer dw init ou completer config/projects.json."
        ));
    }

    let selected = Select::new("Projet Azure DevOps", choices).prompt()?;
    Ok(selected.key)
}

fn project_choices(projects: &dw_config::ProjectsConfig) -> Vec<ProjectChoice> {
    projects
        .projects
        .keys()
        .map(|key| {
            let display_name = resolve_project(projects, key)
                .map(|project| project.display_name)
                .filter(|display_name| !display_name.trim().is_empty());
            ProjectChoice {
                key: key.clone(),
                label: match display_name {
                    Some(display_name) if display_name != *key => format!("{key} - {display_name}"),
                    _ => key.clone(),
                },
            }
        })
        .collect::<Vec<_>>()
}

fn resolve_ado_repositories(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: Option<&str>,
) -> Vec<String> {
    if let Some(repository) = repository.filter(|value| !value.trim().is_empty()) {
        return repository
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|repo| resolve_ado_repository(project_config, repo))
            .fold(Vec::new(), |mut repos, repo| {
                if !repos
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&repo))
                {
                    repos.push(repo);
                }
                repos
            });
    }

    project_config
        .map(|project| {
            project
                .repositories
                .keys()
                .filter_map(|key| dw_config::repository_config(project, key))
                .filter_map(|repo| repo.azure_dev_ops_repository)
                .filter(|repo| !repo.trim().is_empty())
                .fold(Vec::new(), |mut repos, repo| {
                    if !repos
                        .iter()
                        .any(|existing: &String| existing.eq_ignore_ascii_case(&repo))
                    {
                        repos.push(repo);
                    }
                    repos
                })
        })
        .unwrap_or_default()
}

fn resolve_ado_repository(
    project_config: Option<&dw_config::ProjectConfig>,
    repository: &str,
) -> String {
    project_config
        .and_then(|project| dw_config::repository_config(project, repository))
        .and_then(|repo| repo.azure_dev_ops_repository)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| repository.to_string())
}

fn extract_work_item_ids_from_git_range(from: &str, to: Option<&str>) -> Result<Vec<String>> {
    let to = to.filter(|value| !value.trim().is_empty()).ok_or_else(|| {
        anyhow::anyhow!("Le mode --from-git attend 2 refs git: source et target.")
    })?;
    let output = ProcessCommand::new("git")
        .args(["log", "--format=%B%x1e", &format!("{from}..{to}")])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = [stderr.trim(), stdout.trim()]
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or("erreur inconnue");
        return Err(anyhow::anyhow!("git log a echoue: {message}"));
    }
    Ok(extract_work_item_ids_from_commit_messages(
        &String::from_utf8_lossy(&output.stdout),
    ))
}

fn print_assigned_items(
    items: &[dw_ado::WorkItemSnapshot],
    project: &str,
    include_final_states: bool,
    json: bool,
) -> Result<()> {
    if items.is_empty() {
        println!(
            "{}",
            if include_final_states {
                "Aucun work item assigne."
            } else {
                "Aucun work item assigne hors etats finaux."
            }
        );
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
        return Ok(());
    }

    for item in items {
        println!(
            "#{} [{}] {} - {}",
            item.id,
            item.kind.as_deref().unwrap_or("inconnu"),
            item.state.as_deref().unwrap_or("inconnu"),
            item.title.as_deref().unwrap_or("inconnu")
        );
        println!("  Start: dw task start {} --project {}", item.id, project);
    }
    Ok(())
}

fn print_assigned_items_grouped(
    options: &AzureDevOpsOptions,
    items: &[dw_ado::WorkItemSnapshot],
    token: &dw_ado::auth::AdoToken,
    project: &str,
    include_final_states: bool,
    json: bool,
) -> Result<()> {
    if items.is_empty() {
        println!(
            "{}",
            if include_final_states {
                "Aucun work item assigne."
            } else {
                "Aucun work item assigne hors etats finaux."
            }
        );
        return Ok(());
    }

    let groups = group_work_items_by_parent(options, items, token)?;
    if json {
        let payload = groups
            .iter()
            .map(|group| {
                serde_json::json!({
                    "parent": group.parent,
                    "items": group.items,
                    "suggestedStartCommand": format!(
                        "dw task start {} --project {}",
                        suggested_start_ids(&group.parent, &group.items),
                        project
                    )
                })
            })
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string(&payload)?);
        return Ok(());
    }

    for group in groups {
        println!(
            "#{} [{}] {} - {}",
            group.parent.id,
            group.parent.kind.as_deref().unwrap_or("(inconnu)"),
            group.parent.state.as_deref().unwrap_or("(inconnu)"),
            group.parent.title.as_deref().unwrap_or("(sans titre)")
        );
        if !group.items.is_empty() {
            println!(
                "  Start: dw task start {} --project {}",
                suggested_start_ids(&group.parent, &group.items),
                project
            );
        }
        for item in group.items {
            println!(
                "  - #{} [{}] {} - {}",
                item.id,
                item.kind.as_deref().unwrap_or("(inconnu)"),
                item.state.as_deref().unwrap_or("(inconnu)"),
                item.title.as_deref().unwrap_or("(sans titre)")
            );
        }
        println!();
    }
    Ok(())
}

fn suggested_start_ids(
    parent: &dw_ado::WorkItemSnapshot,
    children: &[dw_ado::WorkItemSnapshot],
) -> String {
    let mut ids = vec![parent.id.clone()];
    for child in children {
        if !ids.iter().any(|id| id.eq_ignore_ascii_case(&child.id)) {
            ids.push(child.id.clone());
        }
    }
    ids.join(",")
}

fn parse_work_item_ids(raw: &str) -> Result<Vec<i32>> {
    let mut ids = Vec::new();
    for part in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let id = part
            .parse::<i32>()
            .map_err(|_| anyhow::anyhow!("Work item invalide: {part}"))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return Err(anyhow::anyhow!("Au moins un work item est requis."));
    }
    Ok(ids)
}

fn parse_work_item_ids_as_strings(raw: &str) -> Result<Vec<String>> {
    Ok(parse_work_item_ids(raw)?
        .into_iter()
        .map(|id| id.to_string())
        .collect())
}

fn print_work_item_snapshots(
    items: &[dw_ado::WorkItemSnapshot],
    project: &str,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
        return Ok(());
    }

    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            println!();
            println!("---");
        }
        println!("#{}", item.id);
        println!("Type: {}", item.kind.as_deref().unwrap_or("inconnu"));
        println!("Etat: {}", item.state.as_deref().unwrap_or("inconnu"));
        println!("Titre: {}", item.title.as_deref().unwrap_or("inconnu"));
        println!();
        println!(
            "Contexte complet: dw ado context {} --project {}",
            item.id, project
        );
    }
    Ok(())
}

fn print_context_items(
    items: &[dw_contracts::AdoAiContextItem],
    comment_limit: i32,
    project: &str,
) {
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            println!();
            println!("---");
            println!();
        }

        println!("#{}", item.work_item.id);
        println!(
            "Type: {}",
            item.work_item.kind.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Etat: {}",
            item.work_item.state.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Titre: {}",
            item.work_item.title.as_deref().unwrap_or("inconnu")
        );
        println!(
            "Assigne a: {}",
            item.work_item
                .assigned_to
                .as_deref()
                .unwrap_or("non assigne")
        );

        if let Some(description) = &item.content.description
            && !description.trim().is_empty()
        {
            println!();
            println!("Description:");
            println!("{}", description.trim());
        }

        if !item.relations.is_empty() {
            println!();
            println!("Relations:");
            for relation in &item.relations {
                println!(
                    "- {} {}",
                    relation.kind,
                    relation
                        .work_item_id
                        .as_deref()
                        .or(relation.url.as_deref())
                        .unwrap_or("")
                );
            }
        }

        if comment_limit != 0 && !item.comments.is_empty() {
            println!();
            println!("Commentaires:");
            for comment in item.comments.iter().take(comment_limit.max(0) as usize) {
                println!(
                    "- {}: {}",
                    comment.author.as_deref().unwrap_or("inconnu"),
                    comment.text.as_deref().unwrap_or("").trim()
                );
            }
        }

        println!();
        println!(
            "AI context: dw ado ai-context {} --project {}",
            item.work_item.id, project
        );
    }
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
    fn project_choices_keep_config_order_and_include_display_name() {
        let projects: dw_config::ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "zz": { "displayName": "Projet Z", "repositories": {} },
    "ha": { "displayName": "HOMMAGE AGENCE", "repositories": {} }
  }
}"#,
        )
        .expect("projects config should parse");

        let choices = project_choices(&projects);

        assert_eq!(
            choices,
            vec![
                ProjectChoice {
                    key: "zz".into(),
                    label: "zz - Projet Z".into()
                },
                ProjectChoice {
                    key: "ha".into(),
                    label: "ha - HOMMAGE AGENCE".into()
                }
            ]
        );
    }

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
