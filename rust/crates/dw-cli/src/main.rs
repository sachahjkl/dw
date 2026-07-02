use anyhow::Result;
use clap::{Parser, Subcommand};
use dw_ado::{
    AuthSource, AzureDevOpsOptions, detect_env_auth, expanded_work_item_url, work_item_comments_url,
};
use dw_agent::{AgentOpenRequest, build_open_launch};
use dw_config::{
    config_show, load_projects_config, load_workflow_config, resolve_project, resolve_root,
};
use dw_contracts::Phase0Status;
use dw_db::validate_read_only_sql;
use dw_git::update_repository;
use dw_workspace::{
    TaskStartRequest, build_handoff_validation_report,
    build_preflight_report_from_ai_context_files, execute_task_rename, execute_task_start,
    plan_task_rename, plan_task_repo_latest, plan_task_start, read_manifest_path,
    resolve_open_target, resolve_workspace, resolve_workspace_for_workspace_command, task_current,
    task_list, task_status,
};
use inquire::{MultiSelect, Select, Text};
use std::io::IsTerminal;
use std::path::Path;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Version => {
            println!("dw 0.1.0-bootstrap");
        }
        Command::Config { command } => match command {
            ConfigCommand::Show { root, json } => {
                let report = config_show(root.as_deref());
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("Root: {}", report.root);
                    println!("Color: {}", report.color);
                }
            }
        },
        Command::Phase0 { command } => match command {
            Phase0Command::Status => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&Phase0Status::current())?
                );
            }
        },
        Command::Ado { command } => match command {
            AdoCommand::AuthEnv => {
                let status = detect_env_auth();
                match status.source {
                    AuthSource::EnvironmentPat => {
                        println!(
                            "Auth detectee via {}.",
                            status.variable_name.unwrap_or("variable inconnue")
                        );
                    }
                    AuthSource::Missing => {
                        println!(
                            "Aucune auth detectee. Definir DW_ADO_TOKEN ou AZURE_DEVOPS_EXT_PAT."
                        );
                    }
                }
            }
            AdoCommand::ExpandedWorkItem {
                organization,
                project,
                work_item,
            } => {
                let options = AzureDevOpsOptions {
                    organization,
                    project,
                };
                println!("Expanded: {}", expanded_work_item_url(&options, &work_item));
                println!(
                    "Comments: {}",
                    work_item_comments_url(&options, &work_item, 200)
                );
                println!("Note: le fetch HTTP reel n'est pas encore implemente dans ce bootstrap.");
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
        },
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
                if !skip_ado {
                    return Err(anyhow::anyhow!(
                        "Rust task start exige encore --skip-ado tant que les flux ADO ne sont pas portes."
                    ));
                }

                let root = resolve_root(root.as_deref());
                let projects = load_projects_config(&root);
                let project = interactive_project(project, &projects);
                let work_item_id = interactive_work_item(work_item_id)?;
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
                    let manifest = execute_task_start(&plan, None, None, None)?;
                    let config_files =
                        dw_agent::workspace_config_files(&dw_agent::AgentWorkspaceConfigRequest {
                            workspace: plan.workspace.clone(),
                            work_items: manifest
                                .parent_work_items()
                                .into_iter()
                                .map(|item| dw_agent::WorkspaceWorkItemRef {
                                    id: item.id,
                                    kind: item.kind,
                                    title: item.title,
                                })
                                .collect(),
                            project: plan.project.clone(),
                        });
                    for file in config_files {
                        let path = std::path::Path::new(&plan.workspace).join(file.relative_path);
                        if let Some(parent) = path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(path, file.content)?;
                    }
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
        },
        Command::UiDemo => {
            println!("{}", dw_ui::banner("dw Rust rewrite"));
        }
    }

    Ok(())
}

#[derive(Debug, Parser)]
#[command(name = "dw")]
#[command(about = "Bootstrap du rewrite Rust pour dw")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Version,
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Phase0 {
        #[command(subcommand)]
        command: Phase0Command,
    },
    Ado {
        #[command(subcommand)]
        command: AdoCommand,
    },
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    UiDemo,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Show {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Phase0Command {
    Status,
}

#[derive(Debug, Subcommand)]
enum AdoCommand {
    AuthEnv,
    ExpandedWorkItem {
        #[arg(long)]
        organization: String,
        #[arg(long)]
        project: String,
        #[arg(long = "work-item")]
        work_item: String,
    },
}

#[derive(Debug, Subcommand)]
enum DbCommand {
    Guard {
        #[arg(long)]
        sql: String,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    Status {
        #[arg(long)]
        root: Option<String>,
    },
    List {
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Current {
        #[arg(long)]
        json: bool,
    },
    Open {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue")]
        r#continue: bool,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        json: bool,
        positional_work_item: Option<String>,
    },
    Start {
        work_item_id: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "task")]
        task: Option<String>,
        #[arg(long = "type")]
        type_name: Option<String>,
        #[arg(long = "only")]
        only: Option<String>,
        #[arg(long)]
        slug: Option<String>,
        #[arg(long = "skip-ado")]
        skip_ado: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        execute: bool,
    },
    Preflight {
        #[arg(long)]
        workspace: String,
        #[arg(long = "ai-context-file")]
        ai_context_file: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    Rename {
        slug: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "work-item")]
        work_item: Option<String>,
        #[arg(long = "continue")]
        r#continue: bool,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        execute: bool,
        positional_work_item: Option<String>,
    },
    RepoLatest {
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long = "continue")]
        r#continue: bool,
        #[arg(long = "only")]
        only: Option<String>,
        #[arg(long)]
        root: Option<String>,
        #[arg(long)]
        json: bool,
    },
    HandoffValidate {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        json: bool,
    },
}

fn created_date(value: &str) -> &str {
    value.get(..10).unwrap_or(value)
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

fn interactive_work_item(work_item_id: Option<String>) -> Result<String> {
    if let Some(work_item_id) = work_item_id.filter(|value| !value.trim().is_empty()) {
        return Ok(work_item_id);
    }

    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "work-item-id requis en mode non interactif"
        ));
    }

    Ok(Text::new("Work item ID").prompt()?)
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
