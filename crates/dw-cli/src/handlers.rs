use crate::cli::*;
use crate::version::informational_version;
use anyhow::Result;
use dw_cli_adapter::{
    PromptUi, confirm_risk_prompt_spec, print_ado_action_output, print_db_action_output,
    print_json, print_lines, project_prompt_spec,
};
use dw_core::{
    AdoRepositoryName, Agent, ConfigColorMode, ConfigRootPath, DevWorkflowRoot, DiagnosticLogLevel,
    DwActionEvent, EnvironmentVariableName, ExecutionMode, ExternalLaunchPlan, GitRevision,
    InputRequest, InputResponse, ProjectKey, PromptChoiceValue, PromptKind, PromptSpec,
    PullRequestId, SecretKey, SecretValue, TaskId, TaskSlug, WorkItemId, WorkItemTypeName,
    WorkspacePath, WorkspaceRepositoryName,
};
use dw_ui::TerminalTheme;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use inquire::{Confirm, MultiSelect, Password, PasswordDisplayMode, Select, Text};
use std::io::IsTerminal;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

static OUTPUT_VERBOSITY: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputVerbosity {
    level: u8,
}

impl OutputVerbosity {
    fn current() -> Self {
        Self {
            level: OUTPUT_VERBOSITY.load(Ordering::Relaxed),
        }
    }

    fn includes(self, level: DiagnosticLogLevel) -> bool {
        match level {
            DiagnosticLogLevel::Warning => self.level >= 1,
            DiagnosticLogLevel::Info => self.level >= 1,
            DiagnosticLogLevel::Debug => self.level >= 2,
        }
    }
}

pub(crate) async fn run(cli: Cli) -> Result<()> {
    OUTPUT_VERBOSITY.store(cli.verbose, Ordering::Relaxed);
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
                return Err(anyhow::anyhow!("doctor found issues to fix."));
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
    let input = action.input.clone();
    let result = action.result;
    let mut events = action.events;
    let mut progress = CliProgressUi::lazy(print_events && std::io::stderr().is_terminal());
    let mut seen_event_lines = Vec::new();

    while let Some(event) = events.recv().await {
        if print_events && let Some(line) = cli_action_event_line(&event) {
            if progress.is_interactive() {
                progress.set_message(line.clone());
                seen_event_lines.push(line);
            } else {
                print_lines(&[line]);
            }
        }
        if let DwActionEvent::NeedsInput { request } = &event {
            let response = progress.suspend(|| prompt_cli_input(request))?;
            input.respond(response)?;
        }
    }
    if progress.is_enabled() {
        progress.finish_and_clear();
        print_lines(&seen_event_lines);
    }
    result.await?
}

fn cli_action_event_line(event: &DwActionEvent) -> Option<String> {
    match event {
        DwActionEvent::Ado(event) => Some(dw_cli_adapter::render::ado_action_event_line(event)),
        DwActionEvent::Task(event) => Some(dw_cli_adapter::render::task_action_event_line(event)),
        DwActionEvent::Db(event) => Some(dw_cli_adapter::render::db_action_event_line(event)),
        DwActionEvent::Upgrade(event) => {
            (!dw_cli_adapter::render::upgrade_event_is_transient(event))
                .then(|| dw_cli_adapter::render::upgrade_event_line(event))
        }
        DwActionEvent::Log(event) if OutputVerbosity::current().includes(event.level) => {
            Some(dw_cli_adapter::render::diagnostic_log_event_line(event))
        }
        _ => None,
    }
}

fn prompt_cli_input(request: &InputRequest) -> Result<InputResponse> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Action input `{}` requires an interactive terminal.",
            request.id()
        );
    }
    match request {
        InputRequest::Confirm { label, default, .. } => Ok(InputResponse::Confirm {
            accepted: Confirm::new(label).with_default(*default).prompt()?,
        }),
        InputRequest::SelectOne {
            label,
            help,
            choices,
            ..
        } => {
            let labels = choices
                .iter()
                .map(|choice| choice.label.clone())
                .collect::<Vec<_>>();
            let selected = Select::new(label, labels)
                .with_help_message(help.as_deref().unwrap_or(""))
                .prompt_skippable()?
                .ok_or_else(|| anyhow::anyhow!("Selection canceled: {label}"))?;
            let value = choices
                .iter()
                .find(|choice| choice.label == selected)
                .map(|choice| choice.value.clone())
                .ok_or_else(|| anyhow::anyhow!("Invalid selection: {label}"))?;
            Ok(InputResponse::SelectOne { value })
        }
        InputRequest::SelectMany {
            label,
            help,
            choices,
            ..
        } => {
            let labels = choices
                .iter()
                .map(|choice| choice.label.clone())
                .collect::<Vec<_>>();
            let selected = MultiSelect::new(label, labels)
                .with_help_message(help.as_deref().unwrap_or(""))
                .prompt()?;
            let values = selected
                .iter()
                .map(|selected| {
                    choices
                        .iter()
                        .find(|choice| choice.label == *selected)
                        .map(|choice| choice.value.clone())
                        .ok_or_else(|| anyhow::anyhow!("Invalid selection: {label}"))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(InputResponse::SelectMany { values })
        }
        InputRequest::Text { label, default, .. } => {
            let mut prompt = Text::new(label);
            if let Some(default) = default {
                prompt = prompt.with_default(default);
            }
            Ok(InputResponse::Text {
                value: prompt.prompt()?,
            })
        }
        InputRequest::Secret { label, .. } => Ok(InputResponse::Secret {
            value: Password::new(label)
                .with_display_mode(PasswordDisplayMode::Hidden)
                .without_confirmation()
                .prompt()?,
        }),
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
    let mut progress = UpgradeProgressUi::new(interactive);
    let mut seen_events = Vec::new();

    while !result.is_finished() {
        while let Ok(event) = events.try_recv() {
            let DwActionEvent::Upgrade(event) = event else {
                continue;
            };
            if interactive {
                progress.update(&event);
            } else {
                print_upgrade_event_line(&event);
            }
            if !dw_cli_adapter::render::upgrade_event_is_transient(&event) {
                seen_events.push(event);
            }
        }
        progress.tick();
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    while let Ok(event) = events.try_recv() {
        let DwActionEvent::Upgrade(event) = event else {
            continue;
        };
        if interactive {
            progress.update(&event);
        } else {
            print_upgrade_event_line(&event);
        }
        if !dw_cli_adapter::render::upgrade_event_is_transient(&event) {
            seen_events.push(event);
        }
    }
    if interactive {
        progress.finish_and_clear();
        print_lines(
            &seen_events
                .iter()
                .map(dw_cli_adapter::render::upgrade_event_line)
                .collect::<Vec<_>>(),
        );
    }

    let report = match result.await?? {
        dw_app::DwActionResult::Upgrade(dw_app::UpgradeActionResult::Report(report)) => report,
        result => anyhow::bail!("Unexpected upgrade result: {result:?}"),
    };
    print_lines(&dw_cli_adapter::render::upgrade_report_lines(&report));
    Ok(())
}

fn print_upgrade_event_line(event: &dw_core::UpgradeActionEvent) {
    if !dw_cli_adapter::render::upgrade_event_is_transient(event) {
        print_lines(&[dw_cli_adapter::render::upgrade_event_line(event)]);
    }
}

struct CliProgressUi {
    enabled: bool,
    bar: Option<ProgressBar>,
}

impl CliProgressUi {
    fn lazy(enabled: bool) -> Self {
        Self { enabled, bar: None }
    }

    fn spinner(enabled: bool, initial_message: &'static str) -> Self {
        let mut progress = Self::lazy(enabled);
        if enabled {
            progress.start(initial_message.to_string());
        }
        progress
    }

    fn is_interactive(&self) -> bool {
        self.enabled
    }

    fn is_enabled(&self) -> bool {
        self.bar.is_some()
    }

    fn set_message(&mut self, message: String) {
        if !self.enabled {
            return;
        }
        match &self.bar {
            Some(bar) => bar.set_message(message),
            None => self.start(message),
        }
    }

    fn tick(&self) {
        if let Some(bar) = &self.bar {
            bar.tick();
        }
    }

    fn suspend<T>(&self, operation: impl FnOnce() -> T) -> T {
        match &self.bar {
            Some(bar) => bar.suspend(operation),
            None => operation(),
        }
    }

    fn finish_and_clear(self) {
        if let Some(bar) = self.bar {
            bar.finish_and_clear();
        }
    }

    fn start(&mut self, message: String) {
        let bar = ProgressBar::new_spinner();
        bar.set_draw_target(ProgressDrawTarget::stderr());
        bar.set_style(action_spinner_style());
        bar.set_message(message);
        bar.enable_steady_tick(Duration::from_millis(120));
        self.bar = Some(bar);
    }
}

struct UpgradeProgressUi {
    progress: CliProgressUi,
    showing_download: bool,
}

impl UpgradeProgressUi {
    fn new(enabled: bool) -> Self {
        Self {
            progress: CliProgressUi::spinner(enabled, "Upgrade [starting          ] Preparing"),
            showing_download: false,
        }
    }

    fn update(&mut self, event: &dw_core::UpgradeActionEvent) {
        let Some(bar) = &self.progress.bar else {
            return;
        };
        match event {
            dw_core::UpgradeActionEvent::DownloadedAssetBytes {
                file_name,
                received,
                total,
            } => {
                if let Some(total) = total {
                    if !self.showing_download {
                        bar.set_style(upgrade_download_style());
                        self.showing_download = true;
                    }
                    bar.set_length(total.as_u64());
                    bar.set_position(received.as_u64());
                    bar.set_message(format!("Upgrade [download          ] {file_name}"));
                } else {
                    if self.showing_download {
                        bar.set_style(action_spinner_style());
                        self.showing_download = false;
                    }
                    bar.set_message(dw_cli_adapter::render::upgrade_download_progress_line(
                        file_name,
                        *received,
                        *total,
                        &TerminalTheme::plain(),
                    ));
                    bar.tick();
                }
            }
            event => {
                if self.showing_download {
                    bar.set_style(action_spinner_style());
                    self.showing_download = false;
                }
                bar.set_message(dw_cli_adapter::render::upgrade_event_line(event));
                bar.tick();
            }
        }
    }

    fn tick(&self) {
        self.progress.tick();
    }

    fn finish_and_clear(self) {
        self.progress.finish_and_clear();
    }
}

fn action_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner} {msg}")
        .expect("action spinner style should parse")
        .tick_strings(&["|", "/", "-", "\\"])
}

fn upgrade_download_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{msg} [{bar:28.cyan/blue}] {percent:>3}% {binary_bytes} / {binary_total_bytes}",
    )
    .expect("upgrade download style should parse")
    .progress_chars("█░")
}

async fn handle_auth(command: AuthCommand) -> Result<()> {
    match command {
        AuthCommand::Login { root } => {
            let mode = Select::new(
                "Azure DevOps connection mode",
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
                result => anyhow::bail!("Unexpected auth status result: {result:?}"),
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
                    .map(parse_work_item_ids)
                    .unwrap_or_default(),
            };
            if json {
                let result = execute_cli_action(request).await?;
                match result {
                    dw_app::DwActionResult::Task(result) => match result.as_ref() {
                        dw_app::TaskActionResult::List(report) => print_json(&report.items)?,
                        result => anyhow::bail!("Unexpected task list result: {result:?}"),
                    },
                    result => anyhow::bail!("Unexpected task list result: {result:?}"),
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
                        result => anyhow::bail!("Unexpected task current result: {result:?}"),
                    },
                    result => anyhow::bail!("Unexpected task current result: {result:?}"),
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
            let args = dw_task::start::StartArgs {
                work_item_ids: work_item_id
                    .as_deref()
                    .map(parse_work_item_ids)
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
            };
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
                        let create_options =
                            dw_cli_adapter::render::TaskStartCreateCommandOptions {
                                skip_ado,
                                with_active_children,
                                create_child_tasks,
                            };
                        let create_command = dw_cli_adapter::render::task_start_create_command(
                            &report,
                            create_options,
                        );
                        let mut lines = dw_cli_adapter::render::task_start_plan_lines(&report);
                        lines.push(format!("Create command: {create_command}"));
                        print_lines(&lines);
                        if std::io::stdin().is_terminal()
                            && Confirm::new("Create this workspace now?")
                                .with_default(false)
                                .prompt()?
                        {
                            let execution_args =
                                task_start_execute_args_from_plan(&report, create_options);
                            let result = execute_task_cli_action(
                                dw_app::DwActionRequest::TaskStart(execution_args),
                            )
                            .await?;
                            let dw_app::TaskActionResult::StartExecution(execution) = *result
                            else {
                                anyhow::bail!("Unexpected task start execute result: {result:?}");
                            };
                            print_lines(&dw_cli_adapter::render::task_start_execution_lines(
                                &execution,
                            ));
                        }
                    }
                }
                result => anyhow::bail!("Unexpected task start result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task start-pr result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task preflight result: {result:?}"),
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
                    result => anyhow::bail!("Unexpected task handoff validate result: {result:?}"),
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
                    .map(parse_work_item_ids)
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
                    result => anyhow::bail!("Unexpected task prune preview result: {result:?}"),
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
                    print_lines(&["Prune canceled.".into()]);
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
                result => anyhow::bail!("Unexpected task prune execute result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task repo-latest result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task commit result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task add-repo result: {result:?}"),
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
                    result => anyhow::bail!("Unexpected task teardown preview result: {result:?}"),
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
                    print_lines(&["Removal canceled.".into()]);
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
                result => anyhow::bail!("Unexpected task teardown execute result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task sync result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task rename result: {result:?}"),
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
            let report =
                match *execute_task_cli_action(dw_app::DwActionRequest::TaskCreateChildTask(
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
                ))
                .await?
                {
                    dw_app::TaskActionResult::CreateChildTask(report) => report,
                    result => anyhow::bail!("Unexpected task create-child-task result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task add-work-item result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task remove-work-item result: {result:?}"),
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
                result => anyhow::bail!("Unexpected task finish preview result: {result:?}"),
            };
            if json && !execute {
                print_json(&plan)?;
            } else if !json {
                print_lines(&dw_cli_adapter::render::task_finish_plan_lines(&plan));
            }
            if !dw_task::finish::finish_has_work(&plan) {
                if !json {
                    print_lines(&[String::new(), "Nothing to finish.".into()]);
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
                result => anyhow::bail!("Unexpected task finish execute result: {result:?}"),
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

fn task_start_execute_args_from_plan(
    report: &dw_task::start::StartPlanReport,
    options: dw_cli_adapter::render::TaskStartCreateCommandOptions,
) -> dw_task::start::StartArgs {
    let plan = &report.plan;
    dw_task::start::StartArgs {
        work_item_ids: plan.work_item_ids.clone(),
        root: Some(report.root.clone()),
        project: Some(plan.project.clone()),
        task: plan.task_id.clone(),
        type_name: Some(plan.kind.clone()),
        repositories: plan.repositories.clone(),
        slug: Some(plan.slug.clone()),
        skip_ado: options.skip_ado,
        with_active_children: options.with_active_children,
        create_child_tasks: options.create_child_tasks,
        mode: ExecutionMode::Execute,
    }
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
        result => anyhow::bail!("Unexpected task result: {result:?}"),
    }
}

async fn execute_task_open_cli_action(
    args: dw_task::open::OpenWorkspaceArgs,
) -> Result<ExternalLaunchPlan> {
    match *execute_task_cli_action(dw_app::DwActionRequest::TaskOpen(args)).await? {
        dw_app::TaskActionResult::Open(plan) => Ok(plan),
        result => anyhow::bail!("Unexpected task open result: {result:?}"),
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
                    anyhow::anyhow!("ado changelog --from-git requires --git-to.")
                })?;
                dw_ado_commands::commands::changelog::ChangelogSource::GitRange(
                    dw_git::GitRevisionRange::new(
                        GitRevision::from(ids),
                        GitRevision::from(git_to),
                    ),
                )
            } else if from_pr {
                dw_ado_commands::commands::changelog::ChangelogSource::PullRequests(
                    parse_pull_request_ids(&ids),
                )
            } else {
                dw_ado_commands::commands::changelog::ChangelogSource::WorkItems(
                    parse_work_item_ids(&ids),
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
                anyhow::bail!("ado set-state --json requires --yes to stay deterministic.");
            }
            let args = dw_ado_commands::commands::set_state::SetStateArgs {
                ids: parse_work_item_ids(&id),
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
                    ids: parse_work_item_ids(&id),
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
                ids: parse_work_item_ids(&id),
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
                    ids: parse_work_item_ids(&id),
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
        anyhow::bail!("Unexpected ADO result: {result:?}");
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
        result => anyhow::bail!("Unexpected ADO set-state plan: {result:?}"),
    }
}

async fn execute_ado_cli_action(
    request: dw_app::DwActionRequest,
    print_events: bool,
) -> Result<dw_app::DwActionResult> {
    execute_cli_action_with_event_output(request, print_events).await
}

fn add_work_item_choices_loading_line() -> String {
    "Loading ADO work items to add...".into()
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
        anyhow::bail!("{command_name} requires --project in non-interactive mode.");
    }

    let root = dw_config::resolve_root(root.as_deref());
    let projects = dw_config::load_projects_config(&root);
    let choices = dw_config::project_choices(&projects);
    if choices.is_empty() {
        anyhow::bail!(
            "No project configured in projects.json. Run dw init or complete config/projects.json."
        );
    }
    let mut prompt = InquirePrompt;
    prompt
        .select_value(&project_prompt_spec(
            "ado-project",
            "Azure DevOps project",
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
        anyhow::bail!("ADO state change refused: add --yes with ado set-state.");
    }
    let prompt = format!(
        "Move {} work item(s) from project {} to state `{}`?\n{}",
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
        anyhow::bail!("ADO update canceled.")
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
        "Push only, no ADO".to_string(),
        "Push + PR ADO draft".to_string(),
        "Push + PR ADO ready".to_string(),
        "Keep current flags".to_string(),
    ]
}

fn finish_mode_from_label(label: &str) -> FinishMode {
    match label {
        "Push + PR ADO draft" => FinishMode::DraftPr,
        "Push + PR ADO ready" => FinishMode::ReadyPr,
        "Keep current flags" => FinishMode::KeepFlags,
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

fn parse_work_item_ids(input: &str) -> Vec<WorkItemId> {
    input
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(WorkItemId::from)
        .collect()
}

fn parse_pull_request_ids(input: &str) -> Vec<PullRequestId> {
    input
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(PullRequestId::from)
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
        anyhow::bail!("Work item provided both as an option and as a positional argument.");
    }
    Ok(option
        .or(positional)
        .map(parse_work_item_ids)
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
    Ok(Select::new("Finish mode", finish_mode_choices())
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

    format!("Run finish ({})?\n{}", actions.join(" + "), workspace)
}

fn confirm_finish(yes: bool, prompt: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Destructive finish refused: add --yes with --execute.");
    }
    let mut prompt_ui = InquirePrompt;
    if prompt_ui.confirm(&confirm_risk_prompt_spec("task-finish-mode", prompt), false)? {
        Ok(())
    } else {
        anyhow::bail!("Finish canceled.")
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
        anyhow::bail!("Destructive removal refused: add --yes with --execute.");
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
    let selected_choices = MultiSelect::new("Workspaces to remove", choices)
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
        anyhow::bail!("Missing repository. Provide `dw task add-repo <repo>`.");
    }

    let report = dw_task::repo::add_repo_choices(dw_task::repo::AddRepoChoicesArgs {
        workspace: workspace.map(WorkspacePath::from),
        root: root.map(DevWorkflowRoot::from),
    })?;
    let selected = Select::new("Repository to add", report.choices)
        .prompt_skippable()?
        .ok_or_else(|| anyhow::anyhow!("No configured repository to add."))?;
    Ok(selected)
}

fn confirm_teardown(yes: bool, workspace: &str) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Destructive removal refused: add --yes with --execute.");
    }
    Confirm::new(&format!(
        "Remove this workspace and its worktrees?\n{workspace}"
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
        return Ok(Some(parse_work_item_ids(&ids)));
    }
    if skip_ado || !std::io::stdin().is_terminal() {
        anyhow::bail!("Missing work items to add. Provide `dw task add-work-item <ids>`.");
    }

    print_lines(&[add_work_item_choices_loading_line()]);
    let report = dw_task::work_item::add_work_item_choices_report(choices_args).await?;
    if report.choices.is_empty() {
        print_lines(&[format!(
            "No assigned work item is available to add for project {}.",
            report.project
        )]);
        return Ok(None);
    }
    select_work_item_ids("Work items to add", &report.choices)
}

fn resolve_remove_work_item_ids_interactively(
    explicit: Option<String>,
    choices_args: dw_task::work_item::WorkItemChoicesArgs,
) -> Result<Option<Vec<WorkItemId>>> {
    if let Some(ids) = explicit.filter(|ids| !ids.trim().is_empty()) {
        return Ok(Some(parse_work_item_ids(&ids)));
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Missing work items to remove. Provide `dw task remove-work-item <ids>`.");
    }

    let report = dw_task::work_item::removable_work_item_choices_report(choices_args)?;
    if report.choices.is_empty() {
        print_lines(&["No work item is available to remove.".into()]);
        return Ok(None);
    }
    select_work_item_ids("Work items to remove", &report.choices)
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
        anyhow::bail!("{prompt} missing.");
    };
    if selected.is_empty() {
        print_lines(&["No work item selected.".into()]);
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
        anyhow::bail!("No task workspace found.");
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
        .ok_or_else(|| anyhow::anyhow!("Workspace selection canceled."))?;
    args.workspace = options
        .into_iter()
        .find(|(label, _)| *label == selected)
        .map(|(_, path)| path);
    if args.workspace.is_none() {
        anyhow::bail!("Invalid workspace selection");
    }
    Ok(args)
}

struct InquirePrompt;

impl PromptUi for InquirePrompt {
    fn select_value(&mut self, spec: &PromptSpec) -> Result<PromptChoiceValue> {
        prompt_select_value(spec)
    }

    fn multiselect_values(&mut self, spec: &PromptSpec) -> Result<Vec<PromptChoiceValue>> {
        if spec.kind != PromptKind::MultiSelect {
            anyhow::bail!("PromptSpec `{}` is not a multiselect.", spec.id);
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
            anyhow::bail!("PromptSpec `{}` is not a confirmation.", spec.id);
        }
        Ok(Confirm::new(&spec.label).with_default(default).prompt()?)
    }

    fn text_value(&mut self, spec: &PromptSpec) -> Result<String> {
        if spec.kind != PromptKind::Text {
            anyhow::bail!("PromptSpec `{}` is not a text field.", spec.id);
        }
        Ok(Text::new(&spec.label).prompt()?)
    }
}

fn prompt_select_value(spec: &PromptSpec) -> Result<PromptChoiceValue> {
    if spec.kind != PromptKind::Select {
        anyhow::bail!("PromptSpec `{}` is not a select.", spec.id);
    }
    let choices = spec
        .choices
        .iter()
        .map(|choice| choice.label.clone())
        .collect::<Vec<_>>();
    let selected = Select::new(&spec.label, choices)
        .with_help_message(spec.help.as_deref().unwrap_or(""))
        .prompt_skippable()?
        .ok_or_else(|| anyhow::anyhow!("Selection canceled: {}", spec.label))?;
    prompt_choice_value_from_label(spec, &selected)
}

fn prompt_choice_value_from_label(spec: &PromptSpec, selected: &str) -> Result<PromptChoiceValue> {
    spec.choices
        .iter()
        .find(|choice| choice.label == selected)
        .map(|choice| choice.value.clone())
        .ok_or_else(|| anyhow::anyhow!("Invalid selection: {}", spec.label))
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
        anyhow::bail!("Unexpected DB result: {result:?}");
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
    let input = action.input.clone();
    let result = action.result;
    let mut events = action.events;
    let interactive = std::io::stderr().is_terminal();
    let theme = TerminalTheme::stdout_auto();
    let mut progress = CliProgressUi::spinner(interactive, "DB: preparing");
    let mut seen_events = Vec::new();

    while !result.is_finished() {
        while let Ok(event) = events.try_recv() {
            if let DwActionEvent::NeedsInput { request } = &event {
                let response = progress.suspend(|| prompt_cli_input(request))?;
                input.respond(response)?;
                continue;
            }
            let DwActionEvent::Db(event) = event else {
                continue;
            };
            if interactive {
                progress.set_message(dw_cli_adapter::render::db_action_event_line(&event));
            } else {
                write_db_event_line(&event, &theme)?;
            }
            seen_events.push(event);
        }
        if interactive {
            progress.tick();
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }

    while let Ok(event) = events.try_recv() {
        if let DwActionEvent::NeedsInput { request } = &event {
            let response = progress.suspend(|| prompt_cli_input(request))?;
            input.respond(response)?;
            continue;
        }
        let DwActionEvent::Db(event) = event else {
            continue;
        };
        if interactive {
            progress.set_message(dw_cli_adapter::render::db_action_event_line(&event));
        } else {
            write_db_event_line(&event, &theme)?;
        }
        seen_events.push(event);
    }
    if interactive {
        progress.finish_and_clear();
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
            anyhow::bail!("Use either the SQL option or the positional query, not both.")
        }
        (Some(sql), true) => Ok(sql),
        (None, false) => Ok(positional.to_string()),
        (None, true) => {
            anyhow::bail!("Missing SQL query. Provide the SQL option or a positional query.")
        }
    }
}

fn write_db_event_line(event: &dw_core::DbActionEvent, theme: &TerminalTheme) -> Result<()> {
    eprintln!(
        "{}",
        theme.style_line(&dw_cli_adapter::render::db_action_event_line(event), false)
    );
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
                    value: Some(SecretValue::from(secret)),
                },
                (None, Some(name)) => dw_app::DwActionRequest::SecretSetFromEnv {
                    key: SecretKey::from(key),
                    env: EnvironmentVariableName::from(name),
                },
                (None, None) if std::io::stdin().is_terminal() => {
                    dw_app::DwActionRequest::SecretSet {
                        key: SecretKey::from(key),
                        value: None,
                    }
                }
                (None, None) => {
                    return Err(anyhow::anyhow!(
                        "secret set requires --value or --from-env in non-interactive mode"
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
        SecretCommand::Delete { key, yes } => {
            if !yes && !std::io::stdin().is_terminal() {
                anyhow::bail!("Secret deletion refused: add --yes with secret delete.");
            }
            run_cli_action(dw_app::DwActionRequest::SecretDelete {
                key: SecretKey::from(key),
                confirmed: yes,
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

    #[test]
    fn prompt_choice_value_returns_typed_value_not_label_text() {
        let spec = PromptSpec::select(
            "assigned-work-item",
            "Work item Azure DevOps",
            vec![dw_core::PromptChoice::new(
                "55264",
                "#55264 [Task] (Active) Automatic transmission",
            )],
        );

        let value =
            prompt_choice_value_from_label(&spec, "#55264 [Task] (Active) Automatic transmission")
                .expect("choice should resolve");

        assert_eq!(value.as_str(), "55264");
    }

    #[test]
    fn interactive_ado_loads_have_visible_loading_lines() {
        assert_eq!(
            add_work_item_choices_loading_line(),
            "Loading ADO work items to add..."
        );
    }

    #[test]
    fn task_start_execute_args_preserve_resolved_interactive_plan() {
        let report = dw_task::start::StartPlanReport {
            root: DevWorkflowRoot::from("/tmp/dw"),
            plan: dw_workspace::TaskStartPlan {
                work_item_ids: vec![WorkItemId::from("55311")],
                primary_work_item_id: WorkItemId::from("55311"),
                project: ProjectKey::from("ha"),
                task_id: None,
                kind: WorkItemTypeName::from("feat"),
                slug: TaskSlug::from("gestion-retour-succes"),
                branch_name: dw_core::BranchName::from("feat/55311-gestion-retour-succes"),
                subject_name: dw_core::TaskSubjectName::from("feat-55311-gestion-retour-succes"),
                workspace: WorkspacePath::from("/tmp/dw/workspace"),
                repositories: vec![
                    WorkspaceRepositoryName::from("front"),
                    WorkspaceRepositoryName::from("back"),
                ],
                repository_folders: Default::default(),
                repository_worktrees: Vec::new(),
            },
            work_items: Vec::new(),
            child_tasks: Vec::new(),
        };

        let args = task_start_execute_args_from_plan(
            &report,
            dw_cli_adapter::render::TaskStartCreateCommandOptions {
                skip_ado: false,
                with_active_children: true,
                create_child_tasks: false,
            },
        );

        assert_eq!(args.work_item_ids, vec![WorkItemId::from("55311")]);
        assert_eq!(args.project, Some(ProjectKey::from("ha")));
        assert_eq!(args.repositories, report.plan.repositories);
        assert_eq!(args.slug, Some(TaskSlug::from("gestion-retour-succes")));
        assert_eq!(args.mode, ExecutionMode::Execute);
        assert!(args.with_active_children);
    }
}
