use crate::cli::*;
use crate::guide::print_guide;
use crate::version::informational_version;
use anyhow::Result;
use dw_cli_adapter::{
    PromptUi, confirm_risk_prompt_spec, print_json, print_lines, project_prompt_spec,
    repositories_prompt_spec,
};
use dw_core::{
    AdoActionEvent, AdoRepositoryName, Agent, ConfigColorMode, ConfigRootPath, DevWorkflowRoot,
    EnvironmentVariableName, ExecutionMode, ProjectKey, PromptChoiceValue, PromptKind, PromptSpec,
    PullRequestId, SecretKey, TaskId, TaskSlug, WorkItemId, WorkItemTypeName, WorkspacePath,
    WorkspaceRepositoryName,
};
use dw_ui::TerminalTheme;
use inquire::{Confirm, MultiSelect, Password, PasswordDisplayMode, Select, Text};
use std::io::{IsTerminal, Write};
use std::sync::mpsc;
use std::time::Duration;

pub(crate) async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            println!("Dev Workflow {}", informational_version());
        }
        Command::Guide => {
            print_guide(&informational_version());
        }
        Command::Doctor { fix } => {
            let report = dw_doctor::run_doctor(fix)?;
            print_lines(&dw_cli_adapter::render::doctor_report_lines(
                &report,
                &TerminalTheme::stdout_auto(),
            ));
            if !report.passed() {
                return Err(anyhow::anyhow!("doctor a détecté des points à corriger."));
            }
        }
        Command::Init {
            profile,
            root,
            dry_run,
            no_save,
        } => {
            let report = dw_config::command::init(dw_config::command::InitCommandArgs {
                root,
                profile,
                no_save,
                dry_run,
            })?;
            print_lines(&dw_cli_adapter::render::init_report_lines(&report));
        }
        Command::Refresh { root, profile } => {
            let report = dw_config::command::refresh(dw_config::command::RefreshCommandArgs {
                root,
                profile,
            })?;
            print_lines(&dw_cli_adapter::render::refresh_report_lines(&report));
        }
        Command::Tui { root } => dw_tui::run_tui(root)?,
        Command::Agent { command } => handle_agent(command)?,
        Command::Auth { command } => handle_auth(command).await?,
        Command::Completion { command } => handle_completion(command)?,
        Command::Config { command } => handle_config(command)?,

        Command::Ado { command } => handle_ado(command).await?,
        Command::Db { command } => handle_db(command).await?,
        Command::Secret { command } => handle_secret(command)?,
        Command::Upgrade { check, rid } => {
            handle_upgrade_command(check, rid).await?;
        }
        Command::Task { command } => handle_task(command).await?,
    }

    Ok(())
}

async fn handle_upgrade_command(check: bool, rid: Option<String>) -> Result<()> {
    let (sender, receiver) = mpsc::channel();
    let task = tokio::spawn(async move {
        dw_upgrade::handle_upgrade_with_events(check, rid, move |event| {
            let _ = sender.send(event);
        })
        .await
    });
    let interactive = std::io::stderr().is_terminal();
    let theme = TerminalTheme::stdout_auto();
    let frames = ["|", "/", "-", "\\"];
    let mut frame = 0_usize;
    let mut events = Vec::new();
    let mut current = None;

    while !task.is_finished() {
        while let Ok(event) = receiver.try_recv() {
            if !interactive {
                print_lines(&[dw_cli_adapter::render::upgrade_event_line(&event)]);
            }
            current = Some(event.clone());
            events.push(event);
        }
        if interactive {
            render_upgrade_spinner(current.as_ref(), frames[frame % frames.len()], &theme)?;
            frame = frame.wrapping_add(1);
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    while let Ok(event) = receiver.try_recv() {
        if !interactive {
            print_lines(&[dw_cli_adapter::render::upgrade_event_line(&event)]);
        }
        events.push(event);
    }
    if interactive {
        clear_upgrade_spinner()?;
        print_lines(
            &events
                .iter()
                .map(dw_cli_adapter::render::upgrade_event_line)
                .collect::<Vec<_>>(),
        );
    }

    let report = task.await??;
    print_lines(&dw_cli_adapter::render::upgrade_report_lines(&report));
    Ok(())
}

fn render_upgrade_spinner(
    event: Option<&dw_upgrade::UpgradeEvent>,
    frame: &str,
    theme: &TerminalTheme,
) -> Result<()> {
    let message = event
        .map(dw_cli_adapter::render::upgrade_event_line)
        .unwrap_or_else(|| "Upgrade [starting          ] Préparation".into());
    eprint!("\r{} {}", theme.cyan(frame), message);
    std::io::stderr().flush()?;
    Ok(())
}

fn clear_upgrade_spinner() -> Result<()> {
    eprint!("\r\x1b[2K");
    std::io::stderr().flush()?;
    Ok(())
}

async fn handle_auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login { root } => {
            let mode = Select::new(
                "Mode de connexion Azure DevOps",
                dw_ado_commands::auth::auth_login_choices(),
            )
            .prompt()?
            .mode;
            let report =
                dw_ado_commands::auth::login_report(root, mode, |message| print_lines(&[message]))
                    .await?;
            print_lines(&dw_cli_adapter::render::auth_login_lines(&report));
        }
        AuthCommand::Status { root } => {
            let report = dw_ado_commands::auth::status_report(root).await?;
            print_lines(&dw_cli_adapter::render::auth_status_lines(&report));
            if !report.connected {
                std::process::exit(1);
            }
        }
        AuthCommand::Logout { root } => {
            let report = dw_ado_commands::auth::logout_report(root)?;
            print_lines(&dw_cli_adapter::render::auth_logout_lines(&report));
        }
    }
    Ok(())
}

async fn handle_task(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Status { root } => {
            let report = dw_task::open::status_report(root.map(DevWorkflowRoot::from));
            print_lines(&dw_cli_adapter::render::task_status_lines(&report));
        }
        TaskCommand::List {
            root,
            project,
            work_item,
            json,
        } => {
            let report = dw_task::open::list_report(
                root.map(DevWorkflowRoot::from),
                project.map(ProjectKey::from),
                work_item
                    .as_deref()
                    .map(WorkItemId::parse_many)
                    .unwrap_or_default(),
            );
            if json {
                print_json(&report.items)?;
            } else if report.items.is_empty() {
                print_lines(&["Aucun workspace task trouvé.".into()]);
            } else {
                print_lines(&dw_cli_adapter::render::task_list_lines(&report));
            }
        }
        TaskCommand::Current { json } => {
            let report = dw_task::open::current_report()?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_current_lines(&report));
            }
        }
        TaskCommand::Open {
            workspace,
            project,
            work_item,
            pull_request,
            positional_work_item,
            r#continue,
            repo,
            agent,
            json,
            root,
        } => {
            let args = resolve_open_args_interactively(dw_task::open::OpenWorkspaceArgs {
                workspace: workspace.map(WorkspacePath::from),
                project: project.map(ProjectKey::from),
                work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                pull_request: pull_request.map(PullRequestId::from),
                r#continue,
                repo: repo.map(WorkspaceRepositoryName::from),
                agent: agent.map(dw_core::AgentName::from),
                root: root.map(DevWorkflowRoot::from),
            })?;
            let launch = dw_task::open::resolve_open_launch_async(args).await?;
            if json {
                print_json(&launch)?;
            } else {
                dw_task::open::run_external_launch(&launch)?;
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
            with_active_children,
            create_child_tasks,
            json,
            execute,
        } => {
            let args = resolve_start_args_interactively(dw_task::start::StartArgs {
                work_item_ids: work_item_id
                    .as_deref()
                    .map(WorkItemId::parse_many)
                    .unwrap_or_default(),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                task: task.map(TaskId::from),
                type_name: type_name.map(WorkItemTypeName::from),
                repositories: parse_workspace_repository_names(only.as_deref()),
                slug: slug.map(TaskSlug::from),
                skip_ado,
                with_active_children,
                create_child_tasks,
                mode: ExecutionMode::from_execute(execute),
            })
            .await?;
            let report = dw_task::start::start_plan(args.clone()).await?;
            if execute {
                let execution = dw_task::start::execute_start(report.clone(), &args).await?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_start_execution_lines(
                        &execution,
                    ));
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_start_plan_lines(&report));
            }
        }
        TaskCommand::StartPr {
            pull_request_id,
            root,
            project,
            repo,
            type_name,
            slug,
            json,
            execute,
        } => {
            let args = dw_task::start::StartPrArgs {
                pull_request_id: PullRequestId::from(pull_request_id),
                root: root.map(DevWorkflowRoot::from),
                project: ProjectKey::from(project),
                repositories: parse_workspace_repository_names(repo.as_deref()),
                type_name: type_name.map(WorkItemTypeName::from),
                slug: slug.map(TaskSlug::from),
                mode: ExecutionMode::from_execute(execute),
            };
            let report = dw_task::start::start_pr_plan(args.clone()).await?;
            if execute {
                let execution = dw_task::start::execute_start_pr(report.clone(), &args).await?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_start_execution_lines(
                        &execution,
                    ));
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_start_pr_plan_lines(&report));
            }
        }
        TaskCommand::Preflight {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            ai_context_file,
            json,
            positional_work_item,
        } => {
            let report = dw_task::validate::preflight_report(dw_task::validate::PreflightArgs {
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
                ai_context_files: ai_context_file
                    .into_iter()
                    .map(dw_core::AiContextFilePath::from)
                    .collect(),
            })?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_preflight_lines(&report));
            }
        }
        TaskCommand::HandoffValidate {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            json,
            positional_work_item,
        } => {
            let report = dw_task::validate::handoff_validation_report(
                dw_task::validate::HandoffValidateArgs {
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    work_item_ids: parse_workspace_filter_work_item_ids(
                        work_item.as_deref(),
                        positional_work_item.as_deref(),
                    )?,
                    r#continue,
                },
            )?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_handoff_validation_lines(
                    &report,
                ));
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
            let report = dw_task::prune::plan(dw_task::prune::PruneArgs {
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: work_item
                    .as_deref()
                    .map(WorkItemId::parse_many)
                    .unwrap_or_default(),
                mode: ExecutionMode::from_execute(execute),
                yes,
                no_sync,
            })
            .await?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_prune_plan_lines(&report));
            }

            if report.candidates.is_empty() || !execute {
                return Ok(());
            }
            let selected = resolve_prune_selection(&report, yes)?;
            if selected.is_empty() {
                if !json {
                    print_lines(&["Prune annulé.".into()]);
                }
                return Ok(());
            }
            let execution = dw_task::prune::execute(&report.root, selected)?;
            if json {
                print_json(&execution)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_prune_execution_lines(
                    &execution,
                ));
            }
        }
        TaskCommand::RepoLatest {
            workspace,
            r#continue,
            only,
            root,
            json,
        } => {
            let report = dw_task::repo::repo_latest_plan(dw_task::repo::RepoLatestArgs {
                workspace: workspace.map(WorkspacePath::from),
                r#continue,
                repositories: parse_workspace_repository_names(only.as_deref()),
                root: root.map(DevWorkflowRoot::from),
            })?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_repo_latest_plan_lines(
                    &report,
                ));
                let execution = dw_task::repo::execute_repo_latest(&report)?;
                print_lines(&dw_cli_adapter::render::task_repo_latest_execution_lines(
                    &execution,
                ));
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
            let report = dw_task::repo::commit_plan(dw_task::repo::CommitArgs {
                workspace: workspace.map(WorkspacePath::from),
                r#continue,
                root: root.map(DevWorkflowRoot::from),
                mode: dw_core::ExecutionMode::Preview,
                message,
            })?;
            if execute {
                let execution = dw_task::repo::execute_commit(&report)?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_commit_plan_lines(
                        &report, true,
                    ));
                    print_lines(&dw_cli_adapter::render::task_commit_execution_lines(
                        &execution,
                    ));
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_commit_plan_lines(
                    &report, false,
                ));
            }
        }
        TaskCommand::AddRepo {
            repo,
            workspace,
            root,
            execute,
            json,
        } => {
            let repo = resolve_add_repo_selection(repo, workspace.clone(), root.clone())?;
            let report = dw_task::repo::add_repo_plan(dw_task::repo::AddRepoArgs {
                repo,
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                mode: dw_core::ExecutionMode::Preview,
            })?;
            if execute {
                let execution = dw_task::repo::execute_add_repo(&report)?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_add_repo_plan_lines(&report));
                    print_lines(&dw_cli_adapter::render::task_add_repo_execution_lines(
                        &execution,
                    ));
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_add_repo_plan_lines(&report));
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
            let report = dw_task::repo::teardown_plan(dw_task::repo::TeardownArgs {
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
                mode: dw_core::ExecutionMode::Preview,
                yes: false,
            })?;
            if !execute {
                if json {
                    print_json(&report)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_teardown_plan_lines(
                        &report, false,
                    ));
                }
                return Ok(());
            }
            if report.workspace.is_none() {
                if json {
                    print_json(&report)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_teardown_plan_lines(
                        &report, true,
                    ));
                }
                return Ok(());
            }
            if !confirm_teardown(
                yes,
                report
                    .workspace
                    .as_ref()
                    .map(WorkspacePath::as_str)
                    .unwrap_or_default(),
            )? {
                if !json {
                    print_lines(&["Suppression annulée.".into()]);
                }
                return Ok(());
            }
            let execution = dw_task::repo::execute_teardown(&report)?;
            if json {
                print_json(&execution)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_teardown_execution_lines(
                    &execution,
                ));
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
            let report = dw_task::lifecycle::sync_report(dw_task::lifecycle::SyncArgs {
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
            })
            .await?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_sync_lines(&report));
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
            let report = dw_task::lifecycle::rename_plan(dw_task::lifecycle::RenameArgs {
                slug,
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
                mode: dw_core::ExecutionMode::Preview,
            })?;
            if execute {
                let execution = dw_task::lifecycle::execute_rename(&report)?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_rename_execution_lines(
                        &execution,
                    ));
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_rename_plan_lines(&report));
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
            let report = dw_task::lifecycle::create_child_task_report(
                dw_task::lifecycle::CreateChildTaskArgs {
                    repo: dw_core::WorkspaceRepositoryName::from(repo),
                    title,
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    work_item_ids: parse_workspace_filter_work_item_ids(
                        work_item.as_deref(),
                        positional_work_item.as_deref(),
                    )?,
                    r#continue,
                },
            )
            .await?;
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_child_task_lines(&report));
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
            let choices_args = dw_task::work_item::WorkItemChoicesArgs {
                workspace: workspace.clone().map(WorkspacePath::from),
                root: root.clone().map(DevWorkflowRoot::from),
                project: project.clone().map(ProjectKey::from),
                workspace_work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
            };
            let work_item_ids =
                resolve_add_work_item_ids_interactively(work_item_ids, choices_args, skip_ado)
                    .await?;
            let Some(work_item_ids) = work_item_ids else {
                return Ok(());
            };
            let report = dw_task::work_item::add_plan(dw_task::work_item::AddWorkItemArgs {
                work_item_ids,
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                workspace_work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
                skip_ado,
                type_name,
                title,
                state,
                mode: dw_core::ExecutionMode::Preview,
            })
            .await?;
            if execute {
                let execution = dw_task::work_item::execute_update(&report)?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&report));
                    if let Some(execution) = execution {
                        print_lines(&dw_cli_adapter::render::task_work_item_execution_lines(
                            &execution,
                        ));
                    }
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&report));
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
            let choices_args = dw_task::work_item::WorkItemChoicesArgs {
                workspace: workspace.clone().map(WorkspacePath::from),
                root: root.clone().map(DevWorkflowRoot::from),
                project: project.clone().map(ProjectKey::from),
                workspace_work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
            };
            let work_item_ids =
                resolve_remove_work_item_ids_interactively(work_item_ids, choices_args)?;
            let Some(work_item_ids) = work_item_ids else {
                return Ok(());
            };
            let report = dw_task::work_item::remove_plan(dw_task::work_item::RemoveWorkItemArgs {
                work_item_ids,
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                workspace_work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
                mode: dw_core::ExecutionMode::Preview,
            })?;
            if execute {
                let execution = dw_task::work_item::execute_update(&report)?;
                if json {
                    print_json(&execution)?;
                } else {
                    print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&report));
                    if let Some(execution) = execution {
                        print_lines(&dw_cli_adapter::render::task_work_item_execution_lines(
                            &execution,
                        ));
                    }
                }
            } else if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&report));
            }
        }
        TaskCommand::Finish {
            workspace,
            r#continue,
            root,
            execute,
            yes,
            message,
            mut create_pr,
            mut ready,
            skip_verify,
            mut skip_ado,
            json,
        } => {
            if should_prompt_finish_mode(execute, json, create_pr, ready, skip_ado)
                && let Some(mode) = select_finish_mode()?
            {
                apply_finish_mode(mode, &mut create_pr, &mut ready, &mut skip_ado);
            }
            let args = dw_task::finish::FinishArgs {
                workspace: workspace.map(WorkspacePath::from),
                r#continue,
                root: root.map(DevWorkflowRoot::from),
                mode: dw_core::ExecutionMode::Preview,
                yes: false,
                message,
                create_pr,
                ready,
                skip_verify,
                skip_ado,
            };
            let plan = dw_task::finish::finish_plan(args.clone())?;
            if json && !execute {
                print_json(&plan)?;
            } else if !json {
                print_lines(&dw_cli_adapter::render::task_finish_plan_lines(&plan));
            }
            if !dw_task::finish::finish_has_work(&plan) {
                if !json {
                    print_lines(&[String::new(), "Rien à terminer.".into()]);
                }
                return Ok(());
            }
            if !execute {
                if !json {
                    print_lines(&[
                        String::new(),
                        dw_cli_adapter::render::task_finish_dry_run_hint(
                            plan.changed_repositories.is_empty(),
                            plan.create_pr,
                        )
                        .into(),
                    ]);
                }
                return Ok(());
            }
            confirm_finish(
                yes,
                &finish_confirmation_prompt(
                    plan.workspace.as_str(),
                    !plan.changed_repositories.is_empty(),
                    !plan.unpushed_repositories.is_empty(),
                    plan.create_pr,
                    plan.skip_ado,
                ),
            )?;
            let execution = dw_task::finish::execute_finish(
                plan,
                &dw_task::finish::FinishArgs {
                    mode: ExecutionMode::from_execute(execute),
                    yes,
                    ..args
                },
            )
            .await?;
            if json {
                print_json(&execution)?;
            } else {
                print_lines(&dw_cli_adapter::render::task_finish_execution_lines(
                    &execution,
                ));
            }
        }
    }
    Ok(())
}

async fn handle_ado(command: AdoCommand) -> Result<()> {
    match command {
        AdoCommand::Assigned {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        } => {
            let project = resolve_ado_project_interactively(root.clone(), project, "ado assigned")?;
            let mut report = dw_ado_commands::commands::assigned::report_with_events(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: root.map(DevWorkflowRoot::from),
                    project: Some(project),
                    top,
                    all,
                    group_by_parent,
                },
                |event| {
                    if !json {
                        print_ado_action_event(event);
                    }
                },
            )
            .await?;
            if json {
                if report.group_by_parent {
                    print_json(&report.groups)?;
                } else {
                    print_json(&report.items)?;
                }
            } else {
                report.events.clear();
                print_lines(&dw_cli_adapter::render::ado_assigned_lines(
                    &report,
                    &TerminalTheme::stdout_auto(),
                ));
            }
        }
        AdoCommand::Prs {
            root,
            project,
            repo,
            json,
        } => {
            let report =
                dw_ado_commands::commands::prs::report(dw_ado_commands::commands::prs::PrsArgs {
                    root: root.map(DevWorkflowRoot::from),
                    project: ProjectKey::from(project),
                    repo: repo.map(AdoRepositoryName::from),
                })
                .await?;
            if json {
                print_json(&report.items)?;
            } else {
                print_lines(&dw_cli_adapter::render::ado_prs_lines(&report));
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
            let source = if from_git {
                dw_ado_commands::commands::changelog::ChangelogSource::GitRange {
                    from: ids,
                    to: git_to,
                }
            } else if from_pr {
                dw_ado_commands::commands::changelog::ChangelogSource::PullRequests(
                    PullRequestId::parse_many(&ids),
                )
            } else {
                dw_ado_commands::commands::changelog::ChangelogSource::WorkItems(
                    WorkItemId::parse_many(&ids),
                )
            };
            let mut report = dw_ado_commands::commands::changelog::report_with_events(
                dw_ado_commands::commands::changelog::ChangelogArgs {
                    source,
                    root,
                    project: project.map(ProjectKey::from),
                    repo: repo.map(AdoRepositoryName::from),
                    group_by_parent,
                    format,
                    table,
                    ids_only,
                },
                print_ado_action_event,
            )
            .await?;
            report.events.clear();
            print_lines(&dw_cli_adapter::render::ado_changelog_lines(
                &report,
                &TerminalTheme::stdout_auto(),
            ));
        }
        AdoCommand::SetState {
            id,
            root,
            project,
            state,
            history,
            yes,
            json,
        } => {
            if json && !yes {
                anyhow::bail!("ado set-state --json requiert --yes pour rester déterministe.");
            }
            let plan = dw_ado_commands::commands::set_state::plan(
                dw_ado_commands::commands::set_state::SetStateArgs {
                    ids: WorkItemId::parse_many(&id),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    state: dw_core::WorkItemState::parse(state)?,
                    history: history.map(dw_core::WorkItemHistoryComment::from),
                    yes: false,
                },
            )?;
            confirm_ado_set_state(yes, &plan)?;
            let mut execution =
                dw_ado_commands::commands::set_state::execute_with_events(plan, |event| {
                    if !json {
                        print_ado_action_event(event);
                    }
                })
                .await?;
            if json {
                print_json(&execution)?;
            } else {
                execution.events.clear();
                print_lines(&dw_cli_adapter::render::ado_set_state_execution_lines(
                    &execution,
                ));
            }
        }
        AdoCommand::WorkItem {
            id,
            root,
            project,
            json,
        } => {
            let mut report = dw_ado_commands::commands::work_item::report_with_events(
                dw_ado_commands::commands::work_item::WorkItemArgs {
                    ids: WorkItemId::parse_many(&id),
                    root,
                    project: project.map(ProjectKey::from),
                },
                |event| {
                    if !json {
                        print_ado_action_event(event);
                    }
                },
            )
            .await?;
            if json {
                print_json(&report.items)?;
            } else {
                report.events.clear();
                print_lines(&dw_cli_adapter::render::ado_work_item_lines(
                    &report,
                    &TerminalTheme::stdout_auto(),
                ));
            }
        }
        AdoCommand::Context {
            id,
            root,
            project,
            summary,
            comments,
            json,
        } => {
            let mut report = dw_ado_commands::commands::context::context_report_with_events(
                dw_ado_commands::commands::context::ContextArgs {
                    ids: WorkItemId::parse_many(&id),
                    root,
                    project: project.map(ProjectKey::from),
                    summary,
                    comments,
                    mode: if json {
                        dw_ado_commands::commands::context::ContextMode::Expanded
                    } else {
                        dw_ado_commands::commands::context::ContextMode::AiContext
                    },
                },
                |event| {
                    if !json {
                        print_ado_action_event(event);
                    }
                },
            )
            .await?;
            if json {
                print_json(&report.expanded)?;
            } else {
                report.events.clear();
                print_lines(&dw_cli_adapter::render::ado_context_lines(
                    &report,
                    &TerminalTheme::stdout_auto(),
                ));
            }
        }
        AdoCommand::AiContext {
            root,
            organization,
            project,
            id,
            summary,
            comments,
            include_comments,
        } => {
            let report = dw_ado_commands::commands::context::ai_context_report_with_events(
                dw_ado_commands::commands::context::AiContextArgs {
                    root,
                    organization,
                    project: project.map(ProjectKey::from),
                    ids: WorkItemId::parse_many(&id),
                    summary,
                    comments,
                    include_comments,
                },
                |_| {},
            )
            .await?;
            print_json(&report.items)?;
        }
    }
    Ok(())
}

fn print_ado_action_event(event: AdoActionEvent) {
    print_lines(&[dw_cli_adapter::render::ado_action_event_line(&event)]);
}

fn add_work_item_choices_loading_line() -> String {
    "Chargement des work items ADO à ajouter...".into()
}

fn assigned_work_items_loading_line(project: &str) -> String {
    format!("Chargement des work items assignés pour le projet {project}...")
}

fn resolve_ado_project_interactively(
    root: Option<String>,
    project: Option<String>,
    command_name: &str,
) -> Result<ProjectKey> {
    if let Some(project) = project.filter(|value| !value.trim().is_empty()) {
        return Ok(ProjectKey::from(project));
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("{command_name} requiert --project configuré en mode non-interactif.");
    }

    let root = dw_config::resolve_root(root.as_deref());
    let projects = dw_config::load_projects_config(&root);
    let choices = dw_config::project_choices(&projects);
    if choices.is_empty() {
        anyhow::bail!(
            "Aucun projet configuré dans projects.json. Exécuter dw init ou compléter config/projects.json."
        );
    }
    let mut prompt = InquirePrompt;
    prompt
        .select_value(&project_prompt_spec(
            "ado-project",
            "Projet Azure DevOps",
            &choices,
        ))
        .map(|value| ProjectKey::from(value.to_string()))
}

fn confirm_ado_set_state(
    yes: bool,
    plan: &dw_ado_commands::commands::set_state::SetStatePlanReport,
) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Changement d'état ADO refusé: ajouter --yes avec ado set-state.");
    }
    let prompt = format!(
        "Mettre {} work item(s) du projet {} en état `{}` ?\n{}",
        plan.ids.len(),
        plan.project,
        plan.state,
        plan.ids
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut prompt_ui = InquirePrompt;
    if prompt_ui.confirm(&confirm_risk_prompt_spec("ado-set-state", prompt), false)? {
        Ok(())
    } else {
        anyhow::bail!("Mise à jour ADO annulée.")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinishMode {
    PushOnly,
    DraftPr,
    ReadyPr,
    KeepFlags,
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

fn parse_workspace_repository_names(value: Option<&str>) -> Vec<WorkspaceRepositoryName> {
    value
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(WorkspaceRepositoryName::from)
        .collect()
}

fn parse_workspace_filter_work_item_ids(
    option: Option<&str>,
    positional: Option<&str>,
) -> Result<Vec<WorkItemId>> {
    let option = option.filter(|value| !value.trim().is_empty());
    let positional = positional.filter(|value| !value.trim().is_empty());
    if option.is_some() && positional.is_some() {
        anyhow::bail!("Work item fourni à la fois en option et en positionnel.");
    }
    Ok(option
        .or(positional)
        .map(WorkItemId::parse_many)
        .unwrap_or_default())
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

fn select_finish_mode() -> Result<Option<FinishMode>> {
    Ok(Select::new("Mode de finalisation", finish_mode_choices())
        .prompt_skippable()?
        .map(|label| finish_mode_from_label(&label)))
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

fn confirm_finish(yes: bool, prompt: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Finalisation destructive refusée: ajouter --yes avec --execute.");
    }
    let mut prompt_ui = InquirePrompt;
    if prompt_ui.confirm(&confirm_risk_prompt_spec("task-finish-mode", prompt), false)? {
        Ok(())
    } else {
        anyhow::bail!("Finalisation annulée.")
    }
}

fn resolve_prune_selection(
    report: &dw_task::prune::PrunePlanReport,
    yes: bool,
) -> Result<Vec<dw_workspace::WorkspaceSummary>> {
    #[derive(Clone)]
    struct PruneSelectionChoice {
        index: usize,
        label: String,
    }

    impl std::fmt::Display for PruneSelectionChoice {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(&self.label)
        }
    }

    if yes {
        return Ok(report.candidates.clone());
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Suppression destructive refusée: ajouter --yes avec --execute.");
    }

    let choices = report
        .candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| PruneSelectionChoice {
            index,
            label: dw_task::prune::prune_candidate_choice(candidate),
        })
        .collect::<Vec<_>>();
    let selected_choices = MultiSelect::new("Workspaces à supprimer", choices)
        .prompt_skippable()?
        .unwrap_or_default();

    let selected = selected_choices
        .into_iter()
        .map(|choice| report.candidates[choice.index].clone())
        .collect();
    Ok(selected)
}

fn resolve_add_repo_selection(
    repo: Option<String>,
    workspace: Option<String>,
    root: Option<String>,
) -> Result<WorkspaceRepositoryName> {
    if let Some(repo) = repo.filter(|value| !value.trim().is_empty()) {
        return Ok(WorkspaceRepositoryName::from(repo));
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Repository manquant. Fournir `dw task add-repo <repo>`.");
    }

    let report = dw_task::repo::add_repo_choices(dw_task::repo::AddRepoChoicesArgs {
        workspace: workspace.map(WorkspacePath::from),
        root: root.map(DevWorkflowRoot::from),
    })?;
    let selected = Select::new("Repository à ajouter", report.choices)
        .prompt_skippable()?
        .ok_or_else(|| anyhow::anyhow!("Aucun repository configuré à ajouter."))?;
    Ok(selected)
}

fn confirm_teardown(yes: bool, workspace: &str) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Suppression destructive refusée: ajouter --yes avec --execute.");
    }
    Confirm::new(&format!(
        "Supprimer ce workspace et ses worktrees ?\n{workspace}"
    ))
    .with_default(false)
    .prompt()
    .map_err(Into::into)
}

async fn resolve_add_work_item_ids_interactively(
    explicit: Option<String>,
    choices_args: dw_task::work_item::WorkItemChoicesArgs,
    skip_ado: bool,
) -> Result<Option<Vec<WorkItemId>>> {
    if let Some(ids) = explicit.filter(|ids| !ids.trim().is_empty()) {
        return Ok(Some(WorkItemId::parse_many(&ids)));
    }
    if skip_ado || !std::io::stdin().is_terminal() {
        anyhow::bail!("Work items à ajouter manquants. Fournir `dw task add-work-item <ids>`.");
    }

    print_lines(&[add_work_item_choices_loading_line()]);
    let report = dw_task::work_item::add_work_item_choices_report(choices_args).await?;
    if report.choices.is_empty() {
        print_lines(&[format!(
            "Aucun work item assigné disponible à ajouter pour le projet {}.",
            report.project
        )]);
        return Ok(None);
    }
    select_work_item_ids("Work items à ajouter", &report.choices)
}

fn resolve_remove_work_item_ids_interactively(
    explicit: Option<String>,
    choices_args: dw_task::work_item::WorkItemChoicesArgs,
) -> Result<Option<Vec<WorkItemId>>> {
    if let Some(ids) = explicit.filter(|ids| !ids.trim().is_empty()) {
        return Ok(Some(WorkItemId::parse_many(&ids)));
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Work items à retirer manquants. Fournir `dw task remove-work-item <ids>`.");
    }

    let report = dw_task::work_item::removable_work_item_choices_report(choices_args)?;
    if report.choices.is_empty() {
        print_lines(&["Aucun work item disponible à retirer.".into()]);
        return Ok(None);
    }
    select_work_item_ids("Work items à retirer", &report.choices)
}

fn select_work_item_ids(
    prompt: &str,
    choices: &[dw_workspace::WorkspaceWorkItem],
) -> Result<Option<Vec<WorkItemId>>> {
    let labels = choices
        .iter()
        .map(dw_task::work_item::work_item_choice_label)
        .collect::<Vec<_>>();
    let Some(selected) = MultiSelect::new(prompt, labels).prompt_skippable()? else {
        anyhow::bail!("{prompt} manquants.");
    };
    if selected.is_empty() {
        print_lines(&["Aucun work item sélectionné.".into()]);
        return Ok(None);
    }
    Ok(Some(
        selected
            .iter()
            .map(|label| dw_task::work_item::work_item_id_from_choice(label))
            .collect::<Vec<_>>(),
    ))
}

fn resolve_open_args_interactively(
    mut args: dw_task::open::OpenWorkspaceArgs,
) -> Result<dw_task::open::OpenWorkspaceArgs> {
    if args.workspace.is_some()
        || args.pull_request.is_some()
        || args.r#continue
        || !std::io::stdin().is_terminal()
    {
        return Ok(args);
    }

    let root = dw_config::resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let work_item = args.work_item_ids.first().map(WorkItemId::as_str);
    let items = dw_workspace::task_list(
        &root,
        args.project.as_ref().map(ProjectKey::as_str),
        work_item,
    );
    if items.is_empty() {
        anyhow::bail!("Aucun workspace task trouvé.");
    }
    if items.len() == 1 {
        args.workspace = Some(WorkspacePath::from(items[0].path.clone()));
        return Ok(args);
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
    let selected = Select::new("Workspace", labels)
        .prompt_skippable()?
        .ok_or_else(|| anyhow::anyhow!("Sélection workspace annulée."))?;
    args.workspace = options
        .into_iter()
        .find(|(label, _)| *label == selected)
        .map(|(_, path)| WorkspacePath::from(path));
    if args.workspace.is_none() {
        anyhow::bail!("Sélection workspace invalide");
    }
    Ok(args)
}

async fn resolve_start_args_interactively(
    mut args: dw_task::start::StartArgs,
) -> Result<dw_task::start::StartArgs> {
    if args.project.is_none() && std::io::stdin().is_terminal() {
        let root = dw_config::resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
        let projects = dw_config::load_projects_config(&root);
        let choices = dw_config::project_choices(&projects);
        if !choices.is_empty() {
            let mut prompt = InquirePrompt;
            args.project = prompt
                .select_value(&project_prompt_spec("project", "Projet", &choices))
                .map(|value| ProjectKey::from(value.to_string()))
                .ok();
        }
    }

    if args.work_item_ids.is_empty() {
        if !std::io::stdin().is_terminal() {
            anyhow::bail!(
                "work-item-id requis en mode non interactif. Fournir `dw task start <id>`."
            );
        }
        args.work_item_ids = vec![resolve_start_work_item_id_interactively(&args).await?];
    }

    if args.repositories.is_empty() && std::io::stdin().is_terminal() {
        let Some(project) = args.project.as_ref() else {
            return Ok(args);
        };
        let root = dw_config::resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
        let projects = dw_config::load_projects_config(&root);
        let Some(project_config) = dw_config::resolve_project(&projects, project.as_str()) else {
            return Ok(args);
        };
        let repositories = project_config
            .repositories
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        if repositories.len() > 1 {
            let mut prompt = InquirePrompt;
            let selected = prompt.multiselect_values(&repositories_prompt_spec(repositories))?;
            if !selected.is_empty() {
                args.repositories = selected
                    .into_iter()
                    .map(|value| WorkspaceRepositoryName::from(value.to_string()))
                    .collect();
            }
        }
    }

    Ok(args)
}

async fn resolve_start_work_item_id_interactively(
    args: &dw_task::start::StartArgs,
) -> Result<WorkItemId> {
    if args.skip_ado {
        return Ok(WorkItemId::from(Text::new("Work item ID").prompt()?));
    }

    let Some(project) = args.project.clone() else {
        return Ok(WorkItemId::from(Text::new("Work item ID").prompt()?));
    };
    print_lines(&[assigned_work_items_loading_line(project.as_str())]);
    let report = dw_ado_commands::commands::assigned::report(
        dw_ado_commands::commands::assigned::AssignedArgs {
            root: args.root.clone(),
            project: Some(project),
            top: 50,
            all: false,
            group_by_parent: false,
        },
    )
    .await?;
    let mut prompt = InquirePrompt;
    resolve_start_work_item_id_from_report(&report, &mut prompt)
}

fn resolve_start_work_item_id_from_report(
    report: &dw_ado_commands::commands::assigned::AssignedReport,
    prompt: &mut impl PromptUi,
) -> Result<WorkItemId> {
    if report.items.is_empty() {
        print_lines(&[dw_ado_commands::commands::assigned::empty_assigned_message(false).into()]);
        return prompt
            .text_value(&PromptSpec::text("work-item-id", "Work item ID"))
            .map(WorkItemId::from);
    }

    let selected = prompt.select_value(
        &dw_ado_commands::commands::assigned::assigned_work_item_prompt_spec(&report.items),
    )?;
    if selected.as_str() == dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_VALUE {
        prompt
            .text_value(&PromptSpec::text("work-item-id", "Work item ID"))
            .map(WorkItemId::from)
    } else {
        Ok(WorkItemId::from(selected.as_str()))
    }
}

struct InquirePrompt;

impl PromptUi for InquirePrompt {
    fn select_value(&mut self, spec: &PromptSpec) -> Result<PromptChoiceValue> {
        prompt_select_value(spec)
    }

    fn multiselect_values(&mut self, spec: &PromptSpec) -> Result<Vec<PromptChoiceValue>> {
        if spec.kind != PromptKind::MultiSelect {
            anyhow::bail!("PromptSpec `{}` n'est pas un multiselect.", spec.id);
        }
        let labels = spec
            .choices
            .iter()
            .map(|choice| choice.label.clone())
            .collect::<Vec<_>>();
        let selected = MultiSelect::new(&spec.label, labels)
            .with_help_message(spec.help.as_deref().unwrap_or(""))
            .prompt()?;
        selected
            .iter()
            .map(|label| prompt_choice_value_from_label(spec, label))
            .collect()
    }

    fn confirm(&mut self, spec: &PromptSpec, default: bool) -> Result<bool> {
        if spec.kind != PromptKind::Confirm {
            anyhow::bail!("PromptSpec `{}` n'est pas une confirmation.", spec.id);
        }
        Ok(Confirm::new(&spec.label).with_default(default).prompt()?)
    }

    fn text_value(&mut self, spec: &PromptSpec) -> Result<String> {
        if spec.kind != PromptKind::Text {
            anyhow::bail!("PromptSpec `{}` n'est pas un champ texte.", spec.id);
        }
        Ok(Text::new(&spec.label).prompt()?)
    }
}

fn prompt_select_value(spec: &PromptSpec) -> Result<PromptChoiceValue> {
    if spec.kind != PromptKind::Select {
        anyhow::bail!("PromptSpec `{}` n'est pas un select.", spec.id);
    }
    let choices = spec
        .choices
        .iter()
        .map(|choice| choice.label.clone())
        .collect::<Vec<_>>();
    let selected = Select::new(&spec.label, choices)
        .with_help_message(spec.help.as_deref().unwrap_or(""))
        .prompt_skippable()?
        .ok_or_else(|| anyhow::anyhow!("Sélection annulée: {}", spec.label))?;
    prompt_choice_value_from_label(spec, &selected)
}

fn prompt_choice_value_from_label(spec: &PromptSpec, selected: &str) -> Result<PromptChoiceValue> {
    spec.choices
        .iter()
        .find(|choice| choice.label == selected)
        .map(|choice| choice.value.clone())
        .ok_or_else(|| anyhow::anyhow!("Sélection invalide: {}", spec.label))
}

async fn handle_db(command: DbCommand) -> Result<()> {
    match command {
        DbCommand::Guard { sql } => {
            let result = dw_db::commands::guard(dw_db::commands::GuardArgs { sql });
            print_lines(&dw_cli_adapter::render::db_guard_lines(
                &result,
                &TerminalTheme::stdout_auto(),
            ));
        }
        DbCommand::Schema {
            project,
            database,
            env,
            json,
        } => {
            let result = dw_db::commands::schema(dw_db::commands::SchemaArgs {
                project,
                database,
                env,
            })
            .await?;
            print_db_result(&result, json)?;
        }
        DbCommand::Describe {
            table,
            project,
            database,
            env,
            json,
        } => {
            let result = dw_db::commands::describe(dw_db::commands::DescribeArgs {
                table,
                project,
                database,
                env,
            })
            .await?;
            if let Some(result) = result {
                print_db_result(&result, json)?;
            }
        }
        DbCommand::Query {
            sql,
            project,
            database,
            env,
            max_rows,
            json,
            sql_parts,
        } => {
            let result = dw_db::commands::query(dw_db::commands::QueryArgs {
                sql: resolve_query_sql(sql, sql_parts)?,
                project,
                database,
                env,
                max_rows,
            })
            .await?;
            print_db_result(&result, json)?;
        }
    }
    Ok(())
}

fn resolve_query_sql(sql: Option<String>, sql_parts: Vec<String>) -> Result<String> {
    let sql = sql
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let positional = sql_parts.join(" ");
    let positional = positional.trim();

    match (sql, positional.is_empty()) {
        (Some(_), false) => {
            anyhow::bail!(
                "Utiliser soit l'option SQL, soit la requête positionnelle, pas les deux."
            )
        }
        (Some(sql), true) => Ok(sql),
        (None, false) => Ok(positional.to_string()),
        (None, true) => anyhow::bail!(
            "Requête SQL manquante. Fournir l'option SQL ou une requête positionnelle."
        ),
    }
}

fn print_db_result(result: &dw_db::QueryResult, json: bool) -> Result<()> {
    if json {
        print_json(result)?;
    } else if std::io::stdout().is_terminal() {
        println!(
            "{}",
            dw_cli_adapter::render::db_query_table(result, &TerminalTheme::stdout_auto())
        );
    } else {
        println!("{}", dw_db::render_query_result_tsv(result));
    }
    Ok(())
}

fn handle_secret(command: SecretCommand) -> Result<()> {
    match command {
        SecretCommand::Set {
            key,
            value,
            from_env,
        } => {
            let secret = match (value, from_env) {
                (Some(secret), None) => secret,
                (None, Some(name)) => {
                    dw_secret::secret_from_env(&EnvironmentVariableName::from(name))?
                }
                (None, None) if std::io::stdin().is_terminal() => Password::new("Secret")
                    .with_display_mode(PasswordDisplayMode::Hidden)
                    .without_confirmation()
                    .prompt()?,
                (None, None) => {
                    return Err(anyhow::anyhow!(
                        "secret set requiert --value ou --from-env en mode non interactif"
                    ));
                }
                (Some(_), Some(_)) => unreachable!("clap rejects --value with --from-env"),
            };
            let report = dw_secret::command::set_secret(&SecretKey::from(key), &secret)?;
            print_lines(&dw_cli_adapter::render::secret_set_lines(&report));
        }
        SecretCommand::Get { key } => {
            let report = dw_secret::command::get_secret(&SecretKey::from(key))?;
            print_lines(&dw_cli_adapter::render::secret_get_lines(&report));
        }
        SecretCommand::Delete { key } => {
            let report = dw_secret::command::delete_secret_key(&SecretKey::from(key))?;
            print_lines(&dw_cli_adapter::render::secret_delete_lines(&report));
        }
    }
    Ok(())
}

fn handle_agent(command: AgentCommand) -> Result<()> {
    match command {
        AgentCommand::Context => {
            let root = dw_config::resolve_root(None);
            println!("{}", dw_agent::agent_context(&root));
        }
        AgentCommand::Open {
            workspace,
            root,
            project,
            work_item,
            r#continue,
            repo,
            agent,
            positional_work_item,
        } => {
            let launch = dw_task::open::resolve_open_launch(resolve_open_args_interactively(
                dw_task::open::OpenWorkspaceArgs {
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    work_item_ids: parse_workspace_filter_work_item_ids(
                        work_item.as_deref(),
                        positional_work_item.as_deref(),
                    )?,
                    pull_request: None,
                    r#continue,
                    repo: repo.map(WorkspaceRepositoryName::from),
                    agent: agent.map(dw_core::AgentName::from),
                },
            )?)?;
            dw_task::open::run_external_launch(&launch)?;
        }
        AgentCommand::Config { root } | AgentCommand::Show { root } => {
            let root = dw_config::resolve_root(root.as_deref());
            let root = DevWorkflowRoot::from(root);
            let agent = dw_config::default_agent(&root);
            print_lines(&dw_cli_adapter::render::agent_config_lines(
                &root,
                &agent,
                &TerminalTheme::stdout_auto(),
            ));
        }
        AgentCommand::SetDefault { root, agent } => {
            let root = dw_config::resolve_root(root.as_deref());
            let root = DevWorkflowRoot::from(root);
            let agent = dw_config::set_default_agent(&root, agent.parse::<Agent>()?)?;
            print_lines(&dw_cli_adapter::render::agent_config_updated_lines(
                &root,
                &agent,
                &TerminalTheme::stdout_auto(),
            ));
        }
        AgentCommand::Doctor { agent } => {
            let report = dw_agent::command::agent_doctor(
                agent.as_deref().map(str::parse::<Agent>).transpose()?,
            )?;
            print_lines(&dw_cli_adapter::render::agent_doctor_lines(
                &report,
                &TerminalTheme::stdout_auto(),
            ));
        }
    }
    Ok(())
}

fn handle_config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show { root, json } => {
            let root = root.map(DevWorkflowRoot::from);
            let report = dw_config::command::show(root.as_ref());
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::config_show_lines(
                    &report,
                    &TerminalTheme::stdout_auto(),
                ));
            }
        }
        ConfigCommand::Doctor { root, json } => {
            let root = root.map(DevWorkflowRoot::from);
            let report = dw_config::command::doctor(root.as_ref());
            if json {
                print_json(&report)?;
            } else {
                print_lines(&dw_cli_adapter::render::config_doctor_lines(
                    &report,
                    &TerminalTheme::stdout_auto(),
                ));
            }
            if !report.passed {
                std::process::exit(1);
            }
        }
        ConfigCommand::SetRoot { path } => {
            let report = dw_config::command::set_root(&ConfigRootPath::from(path))?;
            print_lines(&[
                "Configuration mise à jour".into(),
                format!("Root      : {}", report.value),
            ]);
        }
        ConfigCommand::SetColor { mode } => {
            let report = dw_config::command::set_color(&mode.parse::<ConfigColorMode>()?)?;
            print_lines(&[
                "Configuration mise à jour".into(),
                format!("Couleur   : {}", report.value),
            ]);
        }
    }
    Ok(())
}

fn handle_completion(command: CompletionCommand) -> Result<()> {
    match command {
        CompletionCommand::Show => dw_completion::print_completion_show(),
        CompletionCommand::Generate { shell } => {
            let mut command = Cli::localized_command();
            dw_completion::generate_completion(shell, &mut command);
        }
        CompletionCommand::Install { shell } => dw_completion::print_completion_install(shell),
        CompletionCommand::Complete { format, words } => {
            dw_completion::print_completion_complete(format, words)?
        }
    }
    Ok(())
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    struct FakePrompt {
        selected: PromptChoiceValue,
        text: String,
        selected_specs: Vec<dw_core::PromptId>,
        text_specs: Vec<dw_core::PromptId>,
    }

    impl PromptUi for FakePrompt {
        fn select_value(&mut self, spec: &PromptSpec) -> Result<PromptChoiceValue> {
            self.selected_specs.push(spec.id.clone());
            Ok(self.selected.clone())
        }

        fn multiselect_values(&mut self, spec: &PromptSpec) -> Result<Vec<PromptChoiceValue>> {
            self.selected_specs.push(spec.id.clone());
            Ok(vec![self.selected.clone()])
        }

        fn confirm(&mut self, spec: &PromptSpec, _default: bool) -> Result<bool> {
            self.selected_specs.push(spec.id.clone());
            Ok(true)
        }

        fn text_value(&mut self, spec: &PromptSpec) -> Result<String> {
            self.text_specs.push(spec.id.clone());
            Ok(self.text.clone())
        }
    }

    #[test]
    fn prompt_choice_value_returns_typed_value_not_label_text() {
        let spec = PromptSpec::select(
            "assigned-work-item",
            "Work item Azure DevOps",
            vec![dw_core::PromptChoice::new(
                "55264",
                "#55264 [Task] (Actif) Transmission automatique",
            )],
        );

        let value =
            prompt_choice_value_from_label(&spec, "#55264 [Task] (Actif) Transmission automatique")
                .expect("choice should resolve");

        assert_eq!(value.as_str(), "55264");
    }

    #[test]
    fn task_start_interactive_prompt_uses_assigned_select_value() {
        let report = assigned_report(vec![dw_ado::WorkItemSnapshot {
            id: "55264".into(),
            kind: Some("Task".into()),
            state: Some("Actif".into()),
            title: Some("Transmission automatique".into()),
            url: None,
        }]);
        let mut prompt = FakePrompt {
            selected: PromptChoiceValue::from("55264"),
            text: "ignored".into(),
            selected_specs: Vec::new(),
            text_specs: Vec::new(),
        };

        let value = resolve_start_work_item_id_from_report(&report, &mut prompt)
            .expect("assigned select should resolve");

        assert_eq!(value, WorkItemId::from("55264"));
        assert_eq!(
            prompt.selected_specs,
            [dw_core::PromptId::from("assigned-work-item")]
        );
        assert!(prompt.text_specs.is_empty());
    }

    #[test]
    fn task_start_interactive_prompt_keeps_manual_id_fallback_in_select() {
        let report = assigned_report(vec![dw_ado::WorkItemSnapshot {
            id: "55264".into(),
            kind: Some("Task".into()),
            state: Some("Actif".into()),
            title: Some("Transmission automatique".into()),
            url: None,
        }]);
        let mut prompt = FakePrompt {
            selected: PromptChoiceValue::from(
                dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_VALUE,
            ),
            text: "99999".into(),
            selected_specs: Vec::new(),
            text_specs: Vec::new(),
        };

        let value = resolve_start_work_item_id_from_report(&report, &mut prompt)
            .expect("manual fallback should resolve");

        assert_eq!(value, WorkItemId::from("99999"));
        assert_eq!(
            prompt.selected_specs,
            [dw_core::PromptId::from("assigned-work-item")]
        );
        assert_eq!(prompt.text_specs, [dw_core::PromptId::from("work-item-id")]);
    }

    #[test]
    fn interactive_ado_loads_have_visible_loading_lines() {
        assert_eq!(
            assigned_work_items_loading_line("ha"),
            "Chargement des work items assignés pour le projet ha..."
        );
        assert_eq!(
            add_work_item_choices_loading_line(),
            "Chargement des work items ADO à ajouter..."
        );
    }

    fn assigned_report(
        items: Vec<dw_ado::WorkItemSnapshot>,
    ) -> dw_ado_commands::commands::assigned::AssignedReport {
        dw_ado_commands::commands::assigned::AssignedReport {
            root: "/tmp/dw".into(),
            project: "ha".into(),
            top: 50,
            include_final_states: false,
            group_by_parent: false,
            items,
            groups: Vec::new(),
            events: Vec::new(),
        }
    }
}
