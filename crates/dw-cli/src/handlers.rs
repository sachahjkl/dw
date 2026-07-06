use crate::cli::*;
use crate::version::informational_version;
use anyhow::Result;
use dw_cli_adapter::{
    PromptUi, confirm_risk_prompt_spec, print_ado_action_output, print_db_action_output,
    print_json, print_lines, project_prompt_spec, repositories_prompt_spec,
};
use dw_core::{
    AdoActionEvent, AdoRepositoryName, Agent, ConfigColorMode, ConfigRootPath, DevWorkflowRoot,
    DwActionEvent, EnvironmentVariableName, ExecutionMode, ExternalLaunchPlan, GitRevision,
    ProjectKey, PromptChoiceValue, PromptKind, PromptSpec, PullRequestId, SecretKey, SecretValue,
    TaskId, TaskSlug, WorkItemId, WorkItemTypeName, WorkspacePath, WorkspaceRepositoryName,
};
use dw_ui::TerminalTheme;
use inquire::{Confirm, MultiSelect, Password, PasswordDisplayMode, Select, Text};
use std::io::{IsTerminal, Write};
use std::time::Duration;

pub(crate) async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Version => {
            run_cli_action(dw_app::DwActionRequest::Version).await?;
        }
        Command::Guide => {
            print_lines(&dw_cli_adapter::render::guide_lines(
                &informational_version(),
                &TerminalTheme::stdout_auto(),
            ));
        }
        Command::Doctor { fix } => {
            let result = run_cli_action(dw_app::DwActionRequest::Doctor { fix }).await?;
            if let dw_app::DwActionResult::Doctor(report) = result
                && !report.passed()
            {
                return Err(anyhow::anyhow!("doctor a détecté des points à corriger."));
            }
        }
        Command::Init {
            profile,
            root,
            dry_run,
            no_save,
        } => {
            run_cli_action(dw_app::DwActionRequest::ConfigInit(
                dw_config::command::InitCommandArgs {
                    root,
                    profile,
                    no_save,
                    dry_run,
                },
            ))
            .await?;
        }
        Command::Refresh { root, profile } => {
            run_cli_action(dw_app::DwActionRequest::Refresh(
                dw_config::command::RefreshCommandArgs { root, profile },
            ))
            .await?;
        }
        Command::Tui { root } => dw_tui::run_tui(root).await?,
        Command::Agent { command } => handle_agent(command).await?,
        Command::Auth { command } => handle_auth(command).await?,
        Command::Completion { command } => handle_completion(command)?,
        Command::Config { command } => handle_config(command).await?,

        Command::Ado { command } => handle_ado(command).await?,
        Command::Db { command } => handle_db(command).await?,
        Command::Secret { command } => handle_secret(command).await?,
        Command::Upgrade { check, rid } => {
            handle_upgrade_command(check, rid).await?;
        }
        Command::Task { command } => handle_task(command).await?,
    }

    Ok(())
}

async fn run_cli_action(request: dw_app::DwActionRequest) -> Result<dw_app::DwActionResult> {
    let result = execute_cli_action(request).await?;
    print_lines(&dw_cli_adapter::render::action_result_lines(
        &result,
        &TerminalTheme::stdout_auto(),
    ));
    Ok(result)
}

async fn execute_cli_action(request: dw_app::DwActionRequest) -> Result<dw_app::DwActionResult> {
    execute_cli_action_with_event_output(request, true).await
}

async fn execute_cli_action_with_event_output(
    request: dw_app::DwActionRequest,
    print_events: bool,
) -> Result<dw_app::DwActionResult> {
    let action = dw_app::spawn_action(request);
    let result = action.result;
    let mut events = action.events;
    while let Some(event) = events.recv().await {
        if print_events {
            print_cli_action_event(&event);
        }
    }
    result.await?
}

fn print_cli_action_event(event: &DwActionEvent) {
    match event {
        DwActionEvent::Ado(event) => print_ado_action_event(event.clone()),
        DwActionEvent::Task(event) => {
            print_lines(&[dw_cli_adapter::render::task_action_event_line(event)]);
        }
        DwActionEvent::Upgrade(event) => print_upgrade_event_line(event),
        _ => {}
    }
}

fn run_external_launch_plan(launch: &ExternalLaunchPlan) -> Result<()> {
    let status = dw_process::status(
        launch.program_as_str(),
        launch.argument_strings(),
        launch.working_directory.as_ref().map(|path| path.as_str()),
        launch.environment_strings(),
    )?;
    if !status.success() {
        anyhow::bail!("agent exited with status {status}");
    }
    Ok(())
}

async fn handle_upgrade_command(check: bool, rid: Option<String>) -> Result<()> {
    let rid = rid.map(dw_core::RuntimeIdentifier::from);
    let action = dw_app::spawn_action(dw_app::DwActionRequest::Upgrade { check, rid });
    let result = action.result;
    let mut events = action.events;
    let interactive = std::io::stderr().is_terminal();
    let theme = TerminalTheme::stdout_auto();
    let frames = ["|", "/", "-", "\\"];
    let mut frame = 0_usize;
    let mut seen_events = Vec::new();
    let mut current = None;

    while !result.is_finished() {
        while let Ok(event) = events.try_recv() {
            let DwActionEvent::Upgrade(event) = event else {
                continue;
            };
            if !interactive {
                print_upgrade_event_line(&event);
            }
            current = Some(event.clone());
            if !dw_cli_adapter::render::upgrade_event_is_transient(&event) {
                seen_events.push(event);
            }
        }
        if interactive {
            write_upgrade_spinner_frame(current.as_ref(), frames[frame % frames.len()], &theme)?;
            frame = frame.wrapping_add(1);
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    while let Ok(event) = events.try_recv() {
        let DwActionEvent::Upgrade(event) = event else {
            continue;
        };
        if !interactive {
            print_upgrade_event_line(&event);
        }
        if !dw_cli_adapter::render::upgrade_event_is_transient(&event) {
            seen_events.push(event);
        }
    }
    if interactive {
        write_upgrade_spinner_clear()?;
        print_lines(
            &seen_events
                .iter()
                .map(dw_cli_adapter::render::upgrade_event_line)
                .collect::<Vec<_>>(),
        );
    }

    let report = match result.await?? {
        dw_app::DwActionResult::Upgrade(dw_app::UpgradeActionResult::Report(report)) => report,
        result => anyhow::bail!("Résultat upgrade inattendu: {result:?}"),
    };
    print_lines(&dw_cli_adapter::render::upgrade_report_lines(&report));
    Ok(())
}

fn print_upgrade_event_line(event: &dw_core::UpgradeActionEvent) {
    if !dw_cli_adapter::render::upgrade_event_is_transient(event) {
        print_lines(&[dw_cli_adapter::render::upgrade_event_line(event)]);
    }
}

fn write_upgrade_spinner_frame(
    event: Option<&dw_core::UpgradeActionEvent>,
    frame: &str,
    theme: &TerminalTheme,
) -> Result<()> {
    eprint!(
        "{}",
        dw_cli_adapter::render::upgrade_spinner_frame(event, frame, theme)
    );
    std::io::stderr().flush()?;
    Ok(())
}

fn write_upgrade_spinner_clear() -> Result<()> {
    eprint!(
        "{}",
        dw_cli_adapter::render::upgrade_spinner_clear_sequence()
    );
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
            run_cli_action(dw_app::DwActionRequest::AdoAuthLogin {
                root: root.map(DevWorkflowRoot::from),
                mode,
            })
            .await?;
        }
        AuthCommand::Status { root } => {
            let result = execute_cli_action(dw_app::DwActionRequest::AdoAuthStatus {
                root: root.map(DevWorkflowRoot::from),
            })
            .await?;
            match result {
                dw_app::DwActionResult::Ado(dw_app::AdoActionResult::AuthStatus(report)) => {
                    print_lines(&dw_cli_adapter::render::auth_status_lines(&report));
                    if !report.connected {
                        std::process::exit(1);
                    }
                }
                result => anyhow::bail!("Résultat auth status inattendu: {result:?}"),
            }
        }
        AuthCommand::Logout { root } => {
            run_cli_action(dw_app::DwActionRequest::AdoAuthLogout {
                root: root.map(DevWorkflowRoot::from),
            })
            .await?;
        }
    }
    Ok(())
}

async fn handle_task(command: TaskCommand) -> Result<()> {
    match command {
        TaskCommand::Status { root } => {
            run_cli_action(dw_app::DwActionRequest::TaskStatus {
                root: root.map(DevWorkflowRoot::from),
            })
            .await?;
        }
        TaskCommand::List {
            root,
            project,
            work_item,
            json,
        } => {
            let request = dw_app::DwActionRequest::TaskList {
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: work_item
                    .as_deref()
                    .map(WorkItemId::parse_many)
                    .unwrap_or_default(),
            };
            if json {
                let result = execute_cli_action(request).await?;
                match result {
                    dw_app::DwActionResult::Task(result) => match result.as_ref() {
                        dw_app::TaskActionResult::List(report) => print_json(&report.items)?,
                        result => anyhow::bail!("Résultat task list inattendu: {result:?}"),
                    },
                    result => anyhow::bail!("Résultat task list inattendu: {result:?}"),
                }
            } else {
                run_cli_action(request).await?;
            }
        }
        TaskCommand::Current { json } => {
            if json {
                let result = execute_cli_action(dw_app::DwActionRequest::TaskCurrent).await?;
                match result {
                    dw_app::DwActionResult::Task(result) => match result.as_ref() {
                        dw_app::TaskActionResult::Current(report) => print_json(report)?,
                        result => anyhow::bail!("Résultat task current inattendu: {result:?}"),
                    },
                    result => anyhow::bail!("Résultat task current inattendu: {result:?}"),
                }
            } else {
                run_cli_action(dw_app::DwActionRequest::TaskCurrent).await?;
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
                agent: agent.map(parse_agent).transpose()?,
                root: root.map(DevWorkflowRoot::from),
            })?;
            let launch = execute_task_open_cli_action(args).await?;
            if json {
                print_json(&launch)?;
            } else {
                run_external_launch_plan(&launch)?;
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
            match *execute_task_cli_action_with_event_output(
                dw_app::DwActionRequest::TaskStart(args),
                !json,
            )
            .await?
            {
                dw_app::TaskActionResult::StartExecution(execution) => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_start_execution_lines(
                            &execution,
                        ));
                    }
                }
                dw_app::TaskActionResult::StartPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_start_plan_lines(&report));
                    }
                }
                result => anyhow::bail!("Résultat task start inattendu: {result:?}"),
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
            match *execute_task_cli_action_with_event_output(
                dw_app::DwActionRequest::TaskStartPr(args),
                !json,
            )
            .await?
            {
                dw_app::TaskActionResult::StartExecution(execution) => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_start_execution_lines(
                            &execution,
                        ));
                    }
                }
                dw_app::TaskActionResult::StartPrPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_start_pr_plan_lines(&report));
                    }
                }
                result => anyhow::bail!("Résultat task start-pr inattendu: {result:?}"),
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
            let report = match *execute_task_cli_action(dw_app::DwActionRequest::TaskPreflight(
                dw_task::validate::PreflightArgs {
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
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::Preflight(report) => report,
                result => anyhow::bail!("Résultat task preflight inattendu: {result:?}"),
            };
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
            let report =
                match *execute_task_cli_action(dw_app::DwActionRequest::TaskHandoffValidate(
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
                ))
                .await?
                {
                    dw_app::TaskActionResult::HandoffValidate(report) => report,
                    result => anyhow::bail!("Résultat task handoff validate inattendu: {result:?}"),
                };
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
            let args = dw_task::prune::PruneArgs {
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: work_item
                    .as_deref()
                    .map(WorkItemId::parse_many)
                    .unwrap_or_default(),
                selected_workspaces: None,
                mode: ExecutionMode::Preview,
                yes,
                no_sync,
            };
            let report =
                match *execute_task_cli_action(dw_app::DwActionRequest::TaskPrune(args.clone()))
                    .await?
                {
                    dw_app::TaskActionResult::PrunePlan(plan) => plan,
                    result => anyhow::bail!("Résultat task prune preview inattendu: {result:?}"),
                };
            if json && !execute {
                print_json(&report)?;
            } else if !json {
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
            let selected_workspaces = selected
                .into_iter()
                .map(|candidate| candidate.path)
                .collect();
            let execution = match *execute_task_cli_action(dw_app::DwActionRequest::TaskPrune(
                dw_task::prune::PruneArgs {
                    selected_workspaces: Some(selected_workspaces),
                    mode: ExecutionMode::from_execute(execute),
                    ..args
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::PruneExecution(execution) => execution,
                result => anyhow::bail!("Résultat task prune execute inattendu: {result:?}"),
            };
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
            let mode = if json {
                ExecutionMode::Preview
            } else {
                ExecutionMode::Execute
            };
            match *execute_task_cli_action(dw_app::DwActionRequest::TaskRepoLatest(
                dw_task::repo::RepoLatestArgs {
                    workspace: workspace.map(WorkspacePath::from),
                    r#continue,
                    repositories: parse_workspace_repository_names(only.as_deref()),
                    root: root.map(DevWorkflowRoot::from),
                    mode,
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::RepoLatestPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_repo_latest_plan_lines(
                            &report,
                        ));
                    }
                }
                dw_app::TaskActionResult::RepoLatestExecution { plan, execution } => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_repo_latest_plan_lines(&plan));
                        print_lines(&dw_cli_adapter::render::task_repo_latest_execution_lines(
                            &execution,
                        ));
                    }
                }
                result => anyhow::bail!("Résultat task repo-latest inattendu: {result:?}"),
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
            match *execute_task_cli_action(dw_app::DwActionRequest::TaskCommit(
                dw_task::repo::CommitArgs {
                    workspace: workspace.map(WorkspacePath::from),
                    r#continue,
                    root: root.map(DevWorkflowRoot::from),
                    mode: ExecutionMode::from_execute(execute),
                    message: message.map(dw_core::CommitMessage::from),
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::CommitExecution { plan, execution } => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_commit_plan_lines(&plan, true));
                        print_lines(&dw_cli_adapter::render::task_commit_execution_lines(
                            &execution,
                        ));
                    }
                }
                dw_app::TaskActionResult::CommitPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_commit_plan_lines(
                            &report, false,
                        ));
                    }
                }
                result => anyhow::bail!("Résultat task commit inattendu: {result:?}"),
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
            match *execute_task_cli_action(dw_app::DwActionRequest::TaskAddRepo(
                dw_task::repo::AddRepoArgs {
                    repo,
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    mode: ExecutionMode::from_execute(execute),
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::AddRepoExecution { plan, execution } => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_add_repo_plan_lines(&plan));
                        print_lines(&dw_cli_adapter::render::task_add_repo_execution_lines(
                            &execution,
                        ));
                    }
                }
                dw_app::TaskActionResult::AddRepoPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_add_repo_plan_lines(&report));
                    }
                }
                result => anyhow::bail!("Résultat task add-repo inattendu: {result:?}"),
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
            let args = dw_task::repo::TeardownArgs {
                workspace: workspace.map(WorkspacePath::from),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                work_item_ids: parse_workspace_filter_work_item_ids(
                    work_item.as_deref(),
                    positional_work_item.as_deref(),
                )?,
                r#continue,
                mode: ExecutionMode::Preview,
                yes: false,
            };
            let report =
                match *execute_task_cli_action(dw_app::DwActionRequest::TaskTeardown(args.clone()))
                    .await?
                {
                    dw_app::TaskActionResult::TeardownPlan { plan, .. } => plan,
                    result => anyhow::bail!("Résultat task teardown preview inattendu: {result:?}"),
                };
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
            let execution = match *execute_task_cli_action(dw_app::DwActionRequest::TaskTeardown(
                dw_task::repo::TeardownArgs {
                    mode: ExecutionMode::from_execute(execute),
                    yes,
                    ..args
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::TeardownExecution(execution) => execution,
                result => anyhow::bail!("Résultat task teardown execute inattendu: {result:?}"),
            };
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
            let report = match *execute_task_cli_action(dw_app::DwActionRequest::TaskSync(
                dw_task::lifecycle::SyncArgs {
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    work_item_ids: parse_workspace_filter_work_item_ids(
                        work_item.as_deref(),
                        positional_work_item.as_deref(),
                    )?,
                    r#continue,
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::Sync(report) => report,
                result => anyhow::bail!("Résultat task sync inattendu: {result:?}"),
            };
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
            match *execute_task_cli_action(dw_app::DwActionRequest::TaskRename(
                dw_task::lifecycle::RenameArgs {
                    slug: TaskSlug::from(slug),
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    work_item_ids: parse_workspace_filter_work_item_ids(
                        work_item.as_deref(),
                        positional_work_item.as_deref(),
                    )?,
                    r#continue,
                    mode: ExecutionMode::from_execute(execute),
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::RenameExecution(execution) => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_rename_execution_lines(
                            &execution,
                        ));
                    }
                }
                dw_app::TaskActionResult::RenamePlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_rename_plan_lines(&report));
                    }
                }
                result => anyhow::bail!("Résultat task rename inattendu: {result:?}"),
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
            let report = match *execute_task_cli_action(
                dw_app::DwActionRequest::TaskCreateChildTask(
                    dw_task::lifecycle::CreateChildTaskArgs {
                        repo: dw_core::WorkspaceRepositoryName::from(repo),
                        title: dw_core::WorkItemTitle::from(title),
                        workspace: workspace.map(WorkspacePath::from),
                        root: root.map(DevWorkflowRoot::from),
                        project: project.map(ProjectKey::from),
                        work_item_ids: parse_workspace_filter_work_item_ids(
                            work_item.as_deref(),
                            positional_work_item.as_deref(),
                        )?,
                        r#continue,
                    },
                ),
            )
            .await?
            {
                dw_app::TaskActionResult::CreateChildTask(report) => report,
                result => anyhow::bail!("Résultat task create-child-task inattendu: {result:?}"),
            };
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
            match *execute_task_cli_action(dw_app::DwActionRequest::TaskAddWorkItem(
                dw_task::work_item::AddWorkItemArgs {
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
                    type_name: type_name.map(WorkItemTypeName::from),
                    title: title.map(dw_core::WorkItemTitle::from),
                    state: state.map(dw_core::WorkItemState::from),
                    mode: ExecutionMode::from_execute(execute),
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::WorkItemExecution { plan, execution } => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&plan));
                        if let Some(execution) = execution {
                            print_lines(&dw_cli_adapter::render::task_work_item_execution_lines(
                                &execution,
                            ));
                        }
                    }
                }
                dw_app::TaskActionResult::WorkItemPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&report));
                    }
                }
                result => anyhow::bail!("Résultat task add-work-item inattendu: {result:?}"),
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
            match *execute_task_cli_action(dw_app::DwActionRequest::TaskRemoveWorkItem(
                dw_task::work_item::RemoveWorkItemArgs {
                    work_item_ids,
                    workspace: workspace.map(WorkspacePath::from),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                    workspace_work_item_ids: parse_workspace_filter_work_item_ids(
                        work_item.as_deref(),
                        positional_work_item.as_deref(),
                    )?,
                    r#continue,
                    mode: ExecutionMode::from_execute(execute),
                },
            ))
            .await?
            {
                dw_app::TaskActionResult::WorkItemExecution { plan, execution } => {
                    if json {
                        print_json(&execution)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&plan));
                        if let Some(execution) = execution {
                            print_lines(&dw_cli_adapter::render::task_work_item_execution_lines(
                                &execution,
                            ));
                        }
                    }
                }
                dw_app::TaskActionResult::WorkItemPlan(report) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::task_work_item_plan_lines(&report));
                    }
                }
                result => anyhow::bail!("Résultat task remove-work-item inattendu: {result:?}"),
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
                mode: ExecutionMode::Preview,
                yes: false,
                message: message.map(dw_core::CommitMessage::from),
                create_pr,
                ready,
                skip_verify,
                skip_ado,
            };
            let plan = match *execute_task_cli_action_with_event_output(
                dw_app::DwActionRequest::TaskFinish(args.clone()),
                !json,
            )
            .await?
            {
                dw_app::TaskActionResult::FinishPlan(plan) => plan,
                result => anyhow::bail!("Résultat task finish preview inattendu: {result:?}"),
            };
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
            let execution = match *execute_task_cli_action_with_event_output(
                dw_app::DwActionRequest::TaskFinish(dw_task::finish::FinishArgs {
                    mode: ExecutionMode::from_execute(execute),
                    yes,
                    ..args
                }),
                !json,
            )
            .await?
            {
                dw_app::TaskActionResult::FinishExecution(execution) => execution,
                result => anyhow::bail!("Résultat task finish execute inattendu: {result:?}"),
            };
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

async fn execute_task_cli_action(
    request: dw_app::DwActionRequest,
) -> Result<Box<dw_app::TaskActionResult>> {
    execute_task_cli_action_with_event_output(request, true).await
}

async fn execute_task_cli_action_with_event_output(
    request: dw_app::DwActionRequest,
    print_events: bool,
) -> Result<Box<dw_app::TaskActionResult>> {
    match execute_cli_action_with_event_output(request, print_events).await? {
        dw_app::DwActionResult::Task(result) => Ok(result),
        result => anyhow::bail!("Résultat task inattendu: {result:?}"),
    }
}

async fn execute_task_open_cli_action(
    args: dw_task::open::OpenWorkspaceArgs,
) -> Result<ExternalLaunchPlan> {
    match *execute_task_cli_action(dw_app::DwActionRequest::TaskOpen(args)).await? {
        dw_app::TaskActionResult::Open(plan) => Ok(plan),
        result => anyhow::bail!("Résultat task open inattendu: {result:?}"),
    }
}

async fn handle_ado(command: AdoCommand) -> Result<()> {
    let (request, json_projection, print_events) = match command {
        AdoCommand::Assigned {
            root,
            project,
            top,
            all,
            group_by_parent,
            json,
        } => {
            let project = resolve_ado_project_interactively(root.clone(), project, "ado assigned")?;
            (
                dw_app::DwActionRequest::AdoAssigned(
                    dw_ado_commands::commands::assigned::AssignedArgs {
                        root: root.map(DevWorkflowRoot::from),
                        project: Some(project),
                        top,
                        all,
                        group_by_parent,
                    },
                ),
                json.then_some(dw_cli_adapter::render::AdoActionJsonProjection::Assigned),
                !json,
            )
        }
        AdoCommand::Prs {
            root,
            project,
            repo,
            json,
        } => (
            dw_app::DwActionRequest::AdoPrs(dw_ado_commands::commands::prs::PrsArgs {
                root: root.map(DevWorkflowRoot::from),
                project: ProjectKey::from(project),
                repo: repo.map(AdoRepositoryName::from),
            }),
            json.then_some(dw_cli_adapter::render::AdoActionJsonProjection::PullRequests),
            !json,
        ),
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
                let git_to = git_to.ok_or_else(|| {
                    anyhow::anyhow!("ado changelog --from-git requiert --git-to.")
                })?;
                dw_ado_commands::commands::changelog::ChangelogSource::GitRange(
                    dw_git::GitRevisionRange::new(
                        GitRevision::from(ids),
                        GitRevision::from(git_to),
                    ),
                )
            } else if from_pr {
                dw_ado_commands::commands::changelog::ChangelogSource::PullRequests(
                    PullRequestId::parse_many(&ids),
                )
            } else {
                dw_ado_commands::commands::changelog::ChangelogSource::WorkItems(
                    WorkItemId::parse_many(&ids),
                )
            };
            (
                dw_app::DwActionRequest::AdoChangelog(
                    dw_ado_commands::commands::changelog::ChangelogArgs {
                        source,
                        root: root.map(DevWorkflowRoot::from),
                        project: project.map(ProjectKey::from),
                        repo: repo.map(AdoRepositoryName::from),
                        group_by_parent,
                        format: format
                            .as_deref()
                            .map(str::parse)
                            .transpose()?
                            .unwrap_or_default(),
                        table,
                        ids_only,
                    },
                ),
                None,
                true,
            )
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
            let args = dw_ado_commands::commands::set_state::SetStateArgs {
                ids: WorkItemId::parse_many(&id),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                state: dw_core::WorkItemState::parse(state)?,
                history: history.map(dw_core::WorkItemHistoryComment::from),
                yes,
            };
            let plan = execute_ado_set_state_plan(args).await?;
            confirm_ado_set_state(yes, &plan)?;
            (
                dw_app::DwActionRequest::AdoSetStateExecute(plan),
                json.then_some(dw_cli_adapter::render::AdoActionJsonProjection::SetState),
                !json,
            )
        }
        AdoCommand::WorkItem {
            id,
            root,
            project,
            json,
        } => (
            dw_app::DwActionRequest::AdoWorkItem(
                dw_ado_commands::commands::work_item::WorkItemArgs {
                    ids: WorkItemId::parse_many(&id),
                    root: root.map(DevWorkflowRoot::from),
                    project: project.map(ProjectKey::from),
                },
            ),
            json.then_some(dw_cli_adapter::render::AdoActionJsonProjection::WorkItems),
            !json,
        ),
        AdoCommand::Context {
            id,
            root,
            project,
            summary,
            comments,
            json,
        } => (
            dw_app::DwActionRequest::AdoContext(dw_ado_commands::commands::context::ContextArgs {
                ids: WorkItemId::parse_many(&id),
                root: root.map(DevWorkflowRoot::from),
                project: project.map(ProjectKey::from),
                summary,
                comments,
                mode: if json {
                    dw_ado_commands::commands::context::ContextMode::Expanded
                } else {
                    dw_ado_commands::commands::context::ContextMode::AiContext
                },
            }),
            json.then_some(dw_cli_adapter::render::AdoActionJsonProjection::ContextExpanded),
            !json,
        ),
        AdoCommand::AiContext {
            root,
            organization,
            project,
            id,
            summary,
            comments,
            include_comments,
        } => (
            dw_app::DwActionRequest::AdoAiContext(
                dw_ado_commands::commands::context::AiContextArgs {
                    root: root.map(DevWorkflowRoot::from),
                    organization,
                    project: project.map(ProjectKey::from),
                    ids: WorkItemId::parse_many(&id),
                    summary,
                    comments,
                    include_comments,
                },
            ),
            Some(dw_cli_adapter::render::AdoActionJsonProjection::AiContextItems),
            false,
        ),
    };

    let result = execute_ado_cli_action(request, print_events).await?;
    let dw_app::DwActionResult::Ado(result) = result else {
        anyhow::bail!("Résultat ADO inattendu: {result:?}");
    };
    let output = dw_cli_adapter::render::ado_action_output(
        &result,
        json_projection,
        &TerminalTheme::stdout_auto(),
    )?;
    print_ado_action_output(&output);
    Ok(())
}

async fn execute_ado_set_state_plan(
    args: dw_ado_commands::commands::set_state::SetStateArgs,
) -> Result<dw_ado_commands::commands::set_state::SetStatePlanReport> {
    let result =
        execute_ado_cli_action(dw_app::DwActionRequest::AdoSetStatePlan(args), false).await?;
    match result {
        dw_app::DwActionResult::Ado(dw_app::AdoActionResult::SetStatePlan(plan)) => Ok(plan),
        result => anyhow::bail!("Plan ADO set-state inattendu: {result:?}"),
    }
}

async fn execute_ado_cli_action(
    request: dw_app::DwActionRequest,
    print_events: bool,
) -> Result<dw_app::DwActionResult> {
    let action = dw_app::spawn_action(request);
    let result = action.result;
    let mut events = action.events;
    while let Some(event) = events.recv().await {
        if print_events {
            if let DwActionEvent::Ado(event) = event {
                print_ado_action_event(event);
            } else {
                print_cli_action_event(&event);
            }
        }
    }
    result.await?
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

fn parse_agent(value: String) -> Result<Agent> {
    value
        .parse::<Agent>()
        .map_err(|error| anyhow::anyhow!(error))
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
        args.workspace = Some(items[0].path.clone());
        return Ok(args);
    }

    let options = items
        .into_iter()
        .map(|item| {
            (
                format!(
                    "{} / {} / {} / {}",
                    item.project,
                    item.work_items
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                    item.kind,
                    item.path
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
        .map(|(_, path)| path);
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
    let (request, json) = match command {
        DbCommand::Guard { sql } => (
            dw_app::DwActionRequest::DbGuard(dw_db::commands::GuardArgs {
                sql: dw_core::SqlQuery::from(sql),
            }),
            false,
        ),
        DbCommand::Schema {
            project,
            database,
            env,
            json,
        } => (
            dw_app::DwActionRequest::DbSchema(dw_db::commands::SchemaArgs {
                project: project.map(dw_core::ProjectKey::from),
                database: database.map(dw_core::DatabaseKey::from),
                env: env.map(dw_core::DatabaseEnvironmentName::from),
            }),
            json,
        ),
        DbCommand::Describe {
            table,
            project,
            database,
            env,
            json,
        } => (
            dw_app::DwActionRequest::DbDescribe(dw_db::commands::DescribeArgs {
                table: table.map(dw_core::DatabaseTableName::from),
                project: project.map(dw_core::ProjectKey::from),
                database: database.map(dw_core::DatabaseKey::from),
                env: env.map(dw_core::DatabaseEnvironmentName::from),
            }),
            json,
        ),
        DbCommand::Query {
            sql,
            project,
            database,
            env,
            max_rows,
            json,
            sql_parts,
        } => (
            dw_app::DwActionRequest::DbQuery(dw_db::commands::QueryArgs {
                sql: dw_core::SqlQuery::from(resolve_query_sql(sql, sql_parts)?),
                project: project.map(dw_core::ProjectKey::from),
                database: database.map(dw_core::DatabaseKey::from),
                env: env.map(dw_core::DatabaseEnvironmentName::from),
                max_rows,
            }),
            json,
        ),
    };

    let result = execute_db_cli_action(request).await?;
    let dw_app::DwActionResult::Db(result) = result else {
        anyhow::bail!("Résultat DB inattendu: {result:?}");
    };
    let output = dw_cli_adapter::render::db_action_output(
        &result,
        json,
        std::io::stdout().is_terminal(),
        &TerminalTheme::stdout_auto(),
    )?;
    print_db_action_output(&output);
    Ok(())
}

async fn execute_db_cli_action(request: dw_app::DwActionRequest) -> Result<dw_app::DwActionResult> {
    let action = dw_app::spawn_action(request);
    let result = action.result;
    let mut events = action.events;
    let interactive = std::io::stderr().is_terminal();
    let theme = TerminalTheme::stdout_auto();
    let frames = ["|", "/", "-", "\\"];
    let mut frame = 0_usize;
    let mut seen_events = Vec::new();
    let mut current = None;

    while !result.is_finished() {
        while let Ok(event) = events.try_recv() {
            let DwActionEvent::Db(event) = event else {
                continue;
            };
            if !interactive {
                write_db_event_line(&event, &theme)?;
            }
            current = Some(event.clone());
            seen_events.push(event);
        }
        if interactive {
            write_db_spinner_frame(current.as_ref(), frames[frame % frames.len()], &theme)?;
            frame = frame.wrapping_add(1);
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    while let Ok(event) = events.try_recv() {
        let DwActionEvent::Db(event) = event else {
            continue;
        };
        if !interactive {
            write_db_event_line(&event, &theme)?;
        }
        seen_events.push(event);
    }
    if interactive {
        write_db_spinner_clear()?;
        for event in seen_events {
            write_db_event_line(&event, &theme)?;
        }
    }

    result.await?
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

fn write_db_event_line(event: &dw_core::DbActionEvent, theme: &TerminalTheme) -> Result<()> {
    eprintln!(
        "{}",
        theme.style_line(&dw_cli_adapter::render::db_action_event_line(event), false)
    );
    Ok(())
}

fn write_db_spinner_frame(
    event: Option<&dw_core::DbActionEvent>,
    frame: &str,
    theme: &TerminalTheme,
) -> Result<()> {
    eprint!(
        "{}",
        dw_cli_adapter::render::db_spinner_frame(event, frame, theme)
    );
    std::io::stderr().flush()?;
    Ok(())
}

fn write_db_spinner_clear() -> Result<()> {
    eprint!("{}", dw_cli_adapter::render::db_spinner_clear_sequence());
    std::io::stderr().flush()?;
    Ok(())
}

async fn handle_secret(command: SecretCommand) -> Result<()> {
    match command {
        SecretCommand::Set {
            key,
            value,
            from_env,
        } => {
            let request = match (value, from_env) {
                (Some(secret), None) => dw_app::DwActionRequest::SecretSet {
                    key: SecretKey::from(key),
                    value: SecretValue::from(secret),
                },
                (None, Some(name)) => dw_app::DwActionRequest::SecretSetFromEnv {
                    key: SecretKey::from(key),
                    env: EnvironmentVariableName::from(name),
                },
                (None, None) if std::io::stdin().is_terminal() => {
                    dw_app::DwActionRequest::SecretSet {
                        key: SecretKey::from(key),
                        value: SecretValue::from(
                            Password::new("Secret")
                                .with_display_mode(PasswordDisplayMode::Hidden)
                                .without_confirmation()
                                .prompt()?,
                        ),
                    }
                }
                (None, None) => {
                    return Err(anyhow::anyhow!(
                        "secret set requiert --value ou --from-env en mode non interactif"
                    ));
                }
                (Some(_), Some(_)) => unreachable!("clap rejects --value with --from-env"),
            };
            run_cli_action(request).await?;
        }
        SecretCommand::Get { key } => {
            run_cli_action(dw_app::DwActionRequest::SecretGet {
                key: SecretKey::from(key),
            })
            .await?;
        }
        SecretCommand::Delete { key } => {
            run_cli_action(dw_app::DwActionRequest::SecretDelete {
                key: SecretKey::from(key),
            })
            .await?;
        }
    }
    Ok(())
}

async fn handle_agent(command: AgentCommand) -> Result<()> {
    match command {
        AgentCommand::Context => {
            run_cli_action(dw_app::DwActionRequest::AgentContext).await?;
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
            let launch = execute_task_open_cli_action(resolve_open_args_interactively(
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
                    agent: agent.map(parse_agent).transpose()?,
                },
            )?)
            .await?;
            run_external_launch_plan(&launch)?;
        }
        AgentCommand::Config { root } | AgentCommand::Show { root } => {
            run_cli_action(dw_app::DwActionRequest::AgentConfig {
                root: root.map(DevWorkflowRoot::from),
            })
            .await?;
        }
        AgentCommand::SetDefault { root, agent } => {
            run_cli_action(dw_app::DwActionRequest::AgentSetDefault {
                root: root.map(DevWorkflowRoot::from),
                agent: agent.parse::<Agent>()?,
            })
            .await?;
        }
        AgentCommand::Doctor { agent } => {
            run_cli_action(dw_app::DwActionRequest::AgentDoctor {
                agent: agent.as_deref().map(str::parse::<Agent>).transpose()?,
            })
            .await?;
        }
    }
    Ok(())
}

async fn handle_config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Show { root, json } => {
            let result = execute_cli_action(dw_app::DwActionRequest::ConfigShow {
                root: root.map(DevWorkflowRoot::from),
            })
            .await?;
            match result {
                dw_app::DwActionResult::Config(dw_app::ConfigActionResult::Show(report))
                    if json =>
                {
                    print_json(&report)?;
                }
                result => {
                    print_lines(&dw_cli_adapter::render::action_result_lines(
                        &result,
                        &TerminalTheme::stdout_auto(),
                    ));
                }
            }
        }
        ConfigCommand::Doctor { root, json } => {
            let result = execute_cli_action(dw_app::DwActionRequest::ConfigDoctor {
                root: root.map(DevWorkflowRoot::from),
            })
            .await?;
            match result {
                dw_app::DwActionResult::Config(dw_app::ConfigActionResult::Doctor(report)) => {
                    if json {
                        print_json(&report)?;
                    } else {
                        print_lines(&dw_cli_adapter::render::action_result_lines(
                            &dw_app::DwActionResult::Config(dw_app::ConfigActionResult::Doctor(
                                report.clone(),
                            )),
                            &TerminalTheme::stdout_auto(),
                        ));
                    }
                    if !report.passed {
                        std::process::exit(1);
                    }
                }
                result => {
                    print_lines(&dw_cli_adapter::render::action_result_lines(
                        &result,
                        &TerminalTheme::stdout_auto(),
                    ));
                }
            }
        }
        ConfigCommand::SetRoot { path } => {
            run_cli_action(dw_app::DwActionRequest::ConfigSetRoot {
                path: ConfigRootPath::from(path),
            })
            .await?;
        }
        ConfigCommand::SetColor { mode } => {
            run_cli_action(dw_app::DwActionRequest::ConfigSetColor {
                mode: mode.parse::<ConfigColorMode>()?,
            })
            .await?;
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
