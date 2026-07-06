use anyhow::Result;
pub use dw_core::DwActionEvent;
use dw_core::{
    Agent, ConfigColorMode, ConfigRootPath, DevWorkflowRoot, EnvironmentVariableName, InputRequest,
    InputResponse, ProjectKey, PromptChoice, PromptChoiceValue, PromptKind, PromptSpec,
    RuntimeIdentifier, SecretKey, SecretValue, TaskActionEvent, WorkItemId,
    WorkspaceRepositoryName,
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum DwActionRequest {
    Version,
    Doctor {
        fix: bool,
    },
    Guide,
    Refresh(dw_config::command::RefreshCommandArgs),
    ConfigShow {
        root: Option<DevWorkflowRoot>,
    },
    ConfigInit(dw_config::command::InitCommandArgs),
    ConfigDoctor {
        root: Option<DevWorkflowRoot>,
    },
    ConfigSetColor {
        mode: ConfigColorMode,
    },
    ConfigSetRoot {
        path: ConfigRootPath,
    },
    AgentConfig {
        root: Option<DevWorkflowRoot>,
    },
    AgentSetDefault {
        root: Option<DevWorkflowRoot>,
        agent: Agent,
    },
    AgentDoctor {
        agent: Option<Agent>,
    },
    AgentContext,
    DbGuard(dw_db::commands::GuardArgs),
    DbSchema(dw_db::commands::SchemaArgs),
    DbDescribe(dw_db::commands::DescribeArgs),
    DbQuery(dw_db::commands::QueryArgs),
    AdoAuthLogin {
        root: Option<DevWorkflowRoot>,
        mode: dw_ado_commands::auth::AuthLoginMode,
    },
    AdoAuthStatus {
        root: Option<DevWorkflowRoot>,
    },
    AdoAuthLogout {
        root: Option<DevWorkflowRoot>,
    },
    AdoAssigned(dw_ado_commands::commands::assigned::AssignedArgs),
    AdoPrs(dw_ado_commands::commands::prs::PrsArgs),
    AdoChangelog(dw_ado_commands::commands::changelog::ChangelogArgs),
    AdoContext(dw_ado_commands::commands::context::ContextArgs),
    AdoAiContext(dw_ado_commands::commands::context::AiContextArgs),
    AdoWorkItem(dw_ado_commands::commands::work_item::WorkItemArgs),
    AdoSetStatePlan(dw_ado_commands::commands::set_state::SetStateArgs),
    AdoSetStateExecute(dw_ado_commands::commands::set_state::SetStatePlanReport),
    AdoSetState(dw_ado_commands::commands::set_state::SetStateArgs),
    TaskStatus {
        root: Option<DevWorkflowRoot>,
    },
    TaskList {
        root: Option<DevWorkflowRoot>,
        project: Option<ProjectKey>,
        work_item_ids: Vec<WorkItemId>,
    },
    TaskCurrent,
    TaskOpen(dw_task::open::OpenWorkspaceArgs),
    TaskStart(dw_task::start::StartArgs),
    TaskStartPr(dw_task::start::StartPrArgs),
    TaskPreflight(dw_task::validate::PreflightArgs),
    TaskHandoffValidate(dw_task::validate::HandoffValidateArgs),
    TaskSync(dw_task::lifecycle::SyncArgs),
    TaskRename(dw_task::lifecycle::RenameArgs),
    TaskRepoLatest(dw_task::repo::RepoLatestArgs),
    TaskCommit(dw_task::repo::CommitArgs),
    TaskAddRepo(dw_task::repo::AddRepoArgs),
    TaskTeardown(dw_task::repo::TeardownArgs),
    TaskFinish(dw_task::finish::FinishArgs),
    TaskPrune(dw_task::prune::PruneArgs),
    TaskCreateChildTask(dw_task::lifecycle::CreateChildTaskArgs),
    TaskAddWorkItem(dw_task::work_item::AddWorkItemArgs),
    TaskRemoveWorkItem(dw_task::work_item::RemoveWorkItemArgs),
    SecretGet {
        key: SecretKey,
    },
    SecretSetFromEnv {
        key: SecretKey,
        env: EnvironmentVariableName,
    },
    SecretSet {
        key: SecretKey,
        value: Option<SecretValue>,
    },
    SecretDelete {
        key: SecretKey,
        confirmed: bool,
    },
    Upgrade {
        check: bool,
        rid: Option<RuntimeIdentifier>,
    },
}

#[derive(Debug, Clone)]
pub enum DwActionResult {
    App(AppActionResult),
    Config(ConfigActionResult),
    Agent(AgentActionResult),
    Db(DbActionResult),
    Ado(AdoActionResult),
    Task(Box<TaskActionResult>),
    Secret(SecretActionResult),
    Doctor(dw_doctor::DoctorReport),
    Upgrade(UpgradeActionResult),
}

#[derive(Debug, Clone)]
pub enum AppActionResult {
    Version { version: String },
    Guide { topic: GuideTopic },
}

#[derive(Debug, Clone)]
pub enum GuideTopic {
    Main,
}

#[derive(Debug, Clone)]
pub enum ConfigActionResult {
    Show(dw_config::ConfigShow),
    Init(dw_config::InitReport),
    Refresh(dw_config::RefreshReport),
    Doctor(dw_config::ConfigDoctorReport),
    SetColor(dw_config::command::ConfigColorSetReport),
    SetRoot(dw_config::command::ConfigRootSetReport),
}

#[derive(Debug, Clone)]
pub enum AgentActionResult {
    Config { root: DevWorkflowRoot, agent: Agent },
    SetDefault { root: DevWorkflowRoot, agent: Agent },
    Doctor(dw_agent::command::AgentDoctorReport),
    Context(dw_agent::AgentContextReport),
}

#[derive(Debug, Clone)]
pub enum DbActionResult {
    Guard(dw_db::SqlGuardResult),
    Schema(dw_db::QueryResult),
    Describe(Option<dw_db::QueryResult>),
    Query(dw_db::QueryResult),
}

#[derive(Debug, Clone)]
pub enum AdoActionResult {
    AuthLogin(dw_ado_commands::auth::AuthLoginReport),
    AuthStatus(dw_ado_commands::auth::AuthStatusReport),
    AuthLogout(dw_ado_commands::auth::AuthLogoutReport),
    Assigned(dw_ado_commands::commands::assigned::AssignedReport),
    Prs(dw_ado_commands::commands::prs::PrsReport),
    Changelog(dw_ado_commands::commands::changelog::ChangelogReport),
    Context(dw_ado_commands::commands::context::ContextReport),
    AiContext(dw_ado_commands::commands::context::AiContextReport),
    WorkItem(dw_ado_commands::commands::work_item::WorkItemReport),
    SetStatePlan(dw_ado_commands::commands::set_state::SetStatePlanReport),
    SetState(dw_ado_commands::commands::set_state::SetStateExecutionReport),
}

#[derive(Debug, Clone)]
pub enum TaskActionResult {
    Status(dw_task::open::TaskStatusReport),
    List(dw_task::open::TaskListReport),
    Current(dw_task::open::TaskCurrentReport),
    Open(dw_core::ExternalLaunchPlan),
    StartPlan(dw_task::start::StartPlanReport),
    StartExecution(dw_task::start::StartExecutionReport),
    StartPrPlan(dw_task::start::StartPrPlanReport),
    Preflight(dw_contracts::TaskPreflightReport),
    HandoffValidate(dw_contracts::TaskHandoffValidationReport),
    Sync(dw_task::lifecycle::SyncReport),
    RenamePlan(dw_task::lifecycle::RenamePlanReport),
    RenameExecution(dw_task::lifecycle::RenameExecutionReport),
    RepoLatestPlan(dw_task::repo::RepoLatestPlanReport),
    RepoLatestExecution {
        plan: dw_task::repo::RepoLatestPlanReport,
        execution: dw_task::repo::RepoLatestExecutionReport,
    },
    CommitPlan(dw_task::repo::CommitPlanReport),
    CommitExecution {
        plan: dw_task::repo::CommitPlanReport,
        execution: dw_task::repo::CommitExecutionReport,
    },
    AddRepoPlan(dw_task::repo::AddRepoPlanReport),
    AddRepoExecution {
        plan: dw_task::repo::AddRepoPlanReport,
        execution: dw_task::repo::AddRepoExecutionReport,
    },
    TeardownPlan {
        plan: dw_task::repo::TeardownPlanReport,
        execute_requested: bool,
    },
    TeardownExecution(dw_task::repo::TeardownExecutionReport),
    FinishPlan(dw_task::finish::FinishPlanReport),
    FinishExecution(dw_task::finish::FinishExecutionReport),
    PrunePlan(dw_task::prune::PrunePlanReport),
    PruneExecution(dw_task::prune::PruneExecutionReport),
    CreateChildTask(dw_task::lifecycle::CreateChildTaskReport),
    WorkItemPlan(dw_task::work_item::WorkItemUpdatePlanReport),
    WorkItemExecution {
        plan: dw_task::work_item::WorkItemUpdatePlanReport,
        execution: Option<dw_task::work_item::WorkItemUpdateExecutionReport>,
    },
}

#[derive(Debug, Clone)]
pub enum SecretActionResult {
    Get(dw_secret::command::SecretGetReport),
    Set(dw_secret::command::SecretSetReport),
    Delete(dw_secret::command::SecretDeleteReport),
}

#[derive(Debug, Clone)]
pub enum UpgradeActionResult {
    Report(dw_upgrade::UpgradeReport),
}

pub struct DwActionRun {
    pub events: UnboundedReceiver<DwActionEvent>,
    pub input: DwActionInput,
    pub result: JoinHandle<Result<DwActionResult>>,
}

#[derive(Debug, Clone)]
pub struct DwActionInput {
    sender: UnboundedSender<InputResponse>,
}

impl DwActionInput {
    pub fn respond(&self, response: InputResponse) -> Result<()> {
        self.sender
            .send(response)
            .map_err(|_| anyhow::anyhow!("action is no longer waiting for input"))
    }
}

pub fn spawn_action(request: DwActionRequest) -> DwActionRun {
    match request {
        DwActionRequest::Upgrade { check, rid } => spawn_upgrade_action(check, rid),
        request => spawn_callback_action(request),
    }
}

fn spawn_callback_action(request: DwActionRequest) -> DwActionRun {
    let (sender, receiver) = mpsc::unbounded_channel();
    let (input_sender, mut input_receiver) = mpsc::unbounded_channel();
    let input = DwActionInput {
        sender: input_sender,
    };
    let result = tokio::spawn(async move {
        let mut emit = |event| {
            let _ = sender.send(event);
        };
        run_action_inner(request, &mut emit, Some(&mut input_receiver)).await
    });
    DwActionRun {
        events: receiver,
        input,
        result,
    }
}

fn spawn_upgrade_action(check: bool, rid: Option<RuntimeIdentifier>) -> DwActionRun {
    let upgrade = dw_upgrade::spawn_upgrade(check, rid);
    let (sender, receiver) = mpsc::unbounded_channel();
    let (input_sender, _input_receiver) = mpsc::unbounded_channel();
    let result = tokio::spawn(async move {
        let mut upgrade_events = upgrade.events;
        while let Some(event) = upgrade_events.recv().await {
            let _ = sender.send(DwActionEvent::Upgrade(event));
        }
        let report = upgrade.result.await??;
        Ok(DwActionResult::Upgrade(UpgradeActionResult::Report(report)))
    });
    DwActionRun {
        events: receiver,
        input: DwActionInput {
            sender: input_sender,
        },
        result,
    }
}

pub async fn run_action(
    request: DwActionRequest,
    mut emit: impl FnMut(DwActionEvent),
) -> Result<DwActionResult> {
    run_action_inner(request, &mut emit, None).await
}

async fn run_action_inner(
    request: DwActionRequest,
    emit: &mut impl FnMut(DwActionEvent),
    mut input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<DwActionResult> {
    match request {
        DwActionRequest::Version => Ok(DwActionResult::App(AppActionResult::Version {
            version: env!("DW_VERSION").into(),
        })),
        DwActionRequest::Guide => Ok(DwActionResult::App(AppActionResult::Guide {
            topic: GuideTopic::Main,
        })),
        DwActionRequest::Doctor { fix } => Ok(DwActionResult::Doctor(dw_doctor::run_doctor(fix)?)),
        DwActionRequest::Refresh(args) => Ok(DwActionResult::Config(ConfigActionResult::Refresh(
            dw_config::command::refresh(args)?,
        ))),
        DwActionRequest::ConfigShow { root } => Ok(DwActionResult::Config(
            ConfigActionResult::Show(dw_config::command::show(root.as_ref())),
        )),
        DwActionRequest::ConfigInit(args) => Ok(DwActionResult::Config(ConfigActionResult::Init(
            dw_config::command::init(args)?,
        ))),
        DwActionRequest::ConfigDoctor { root } => Ok(DwActionResult::Config(
            ConfigActionResult::Doctor(dw_config::command::doctor(root.as_ref())),
        )),
        DwActionRequest::ConfigSetColor { mode } => Ok(DwActionResult::Config(
            ConfigActionResult::SetColor(dw_config::command::set_color(&mode)?),
        )),
        DwActionRequest::ConfigSetRoot { path } => Ok(DwActionResult::Config(
            ConfigActionResult::SetRoot(dw_config::command::set_root(&path)?),
        )),
        DwActionRequest::AgentConfig { root } => {
            let root = dw_config::resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
            let root = DevWorkflowRoot::from(root);
            let agent = dw_config::default_agent(&root);
            Ok(DwActionResult::Agent(AgentActionResult::Config {
                root,
                agent,
            }))
        }
        DwActionRequest::AgentSetDefault { root, agent } => {
            let root = dw_config::resolve_root(root.as_ref().map(DevWorkflowRoot::as_str));
            let root = DevWorkflowRoot::from(root);
            let agent = dw_config::set_default_agent(&root, agent)?;
            Ok(DwActionResult::Agent(AgentActionResult::SetDefault {
                root,
                agent,
            }))
        }
        DwActionRequest::AgentDoctor { agent } => Ok(DwActionResult::Agent(
            AgentActionResult::Doctor(dw_agent::command::agent_doctor(agent)?),
        )),
        DwActionRequest::AgentContext => {
            let root = dw_config::resolve_root(None);
            let root = DevWorkflowRoot::from(root);
            Ok(DwActionResult::Agent(AgentActionResult::Context(
                dw_agent::agent_context(&root),
            )))
        }
        DwActionRequest::DbGuard(args) => Ok(DwActionResult::Db(DbActionResult::Guard(
            dw_db::commands::guard_with_events(args, |event| emit(DwActionEvent::Db(event))),
        ))),
        DwActionRequest::DbSchema(args) => Ok(DwActionResult::Db(DbActionResult::Schema(
            dw_db::commands::schema_with_events(args, |event| emit(DwActionEvent::Db(event)))
                .await?,
        ))),
        DwActionRequest::DbDescribe(args) => Ok(DwActionResult::Db(DbActionResult::Describe(
            dw_db::commands::describe_with_events(args, |event| emit(DwActionEvent::Db(event)))
                .await?,
        ))),
        DwActionRequest::DbQuery(args) => Ok(DwActionResult::Db(DbActionResult::Query(
            dw_db::commands::query_with_events(args, |event| emit(DwActionEvent::Db(event)))
                .await?,
        ))),
        DwActionRequest::AdoAuthLogin { root, mode } => {
            let report = dw_ado_commands::auth::login_report_with_events(root, mode, |event| {
                emit(DwActionEvent::Ado(event))
            })
            .await?;
            Ok(DwActionResult::Ado(AdoActionResult::AuthLogin(report)))
        }
        DwActionRequest::AdoAuthStatus { root } => Ok(DwActionResult::Ado(
            AdoActionResult::AuthStatus(dw_ado_commands::auth::status_report(root).await?),
        )),
        DwActionRequest::AdoAuthLogout { root } => Ok(DwActionResult::Ado(
            AdoActionResult::AuthLogout(dw_ado_commands::auth::logout_report(root)?),
        )),
        DwActionRequest::AdoAssigned(args) => {
            let report =
                dw_ado_commands::commands::assigned::report_with_events(args, &mut |event| {
                    emit(DwActionEvent::Ado(event))
                })
                .await?;
            Ok(DwActionResult::Ado(AdoActionResult::Assigned(report)))
        }
        DwActionRequest::AdoPrs(args) => Ok(DwActionResult::Ado(AdoActionResult::Prs(
            dw_ado_commands::commands::prs::report(args).await?,
        ))),
        DwActionRequest::AdoChangelog(args) => {
            let report =
                dw_ado_commands::commands::changelog::report_with_events(args, &mut |event| {
                    emit(DwActionEvent::Ado(event))
                })
                .await?;
            Ok(DwActionResult::Ado(AdoActionResult::Changelog(report)))
        }
        DwActionRequest::AdoContext(args) => {
            let report = dw_ado_commands::commands::context::context_report_with_events(
                args,
                &mut |event| emit(DwActionEvent::Ado(event)),
            )
            .await?;
            Ok(DwActionResult::Ado(AdoActionResult::Context(report)))
        }
        DwActionRequest::AdoAiContext(args) => {
            let report = dw_ado_commands::commands::context::ai_context_report_with_events(
                args,
                &mut |event| emit(DwActionEvent::Ado(event)),
            )
            .await?;
            Ok(DwActionResult::Ado(AdoActionResult::AiContext(report)))
        }
        DwActionRequest::AdoWorkItem(args) => {
            let report =
                dw_ado_commands::commands::work_item::report_with_events(args, &mut |event| {
                    emit(DwActionEvent::Ado(event))
                })
                .await?;
            Ok(DwActionResult::Ado(AdoActionResult::WorkItem(report)))
        }
        DwActionRequest::AdoSetStatePlan(args) => Ok(DwActionResult::Ado(
            AdoActionResult::SetStatePlan(dw_ado_commands::commands::set_state::plan(args)?),
        )),
        DwActionRequest::AdoSetStateExecute(plan) => {
            let execution =
                dw_ado_commands::commands::set_state::execute_with_events(plan, &mut |event| {
                    emit(DwActionEvent::Ado(event))
                })
                .await?;
            Ok(DwActionResult::Ado(AdoActionResult::SetState(execution)))
        }
        DwActionRequest::AdoSetState(args) => {
            let plan = dw_ado_commands::commands::set_state::plan(args)?;
            let execution =
                dw_ado_commands::commands::set_state::execute_with_events(plan, &mut |event| {
                    emit(DwActionEvent::Ado(event))
                })
                .await?;
            Ok(DwActionResult::Ado(AdoActionResult::SetState(execution)))
        }
        DwActionRequest::TaskStatus { root } => Ok(task_result(TaskActionResult::Status(
            dw_task::open::status_report(root),
        ))),
        DwActionRequest::TaskList {
            root,
            project,
            work_item_ids,
        } => Ok(task_result(TaskActionResult::List(
            dw_task::open::list_report(root, project, work_item_ids),
        ))),
        DwActionRequest::TaskCurrent => Ok(task_result(TaskActionResult::Current(
            dw_task::open::current_report()?,
        ))),
        DwActionRequest::TaskOpen(args) => Ok(task_result(TaskActionResult::Open(
            dw_task::open::resolve_open_launch_async(args).await?,
        ))),
        DwActionRequest::TaskStart(args) => {
            let args = resolve_task_start_input(args, emit, input_receiver.as_deref_mut()).await?;
            let plan = dw_task::start::start_plan(args.clone()).await?;
            if args.mode.executes() {
                Ok(task_result(TaskActionResult::StartExecution(
                    dw_task::start::execute_start(plan, &args).await?,
                )))
            } else {
                Ok(task_result(TaskActionResult::StartPlan(plan)))
            }
        }
        DwActionRequest::TaskStartPr(args) => {
            emit(DwActionEvent::Task(
                TaskActionEvent::ResolvingPullRequestWorkItems {
                    pull_request_id: args.pull_request_id.clone(),
                },
            ));
            let report = dw_task::start::start_pr_plan(args.clone()).await?;
            emit(DwActionEvent::Task(
                TaskActionEvent::ResolvedPullRequestWorkItems {
                    work_item_ids: report.work_item_ids.clone(),
                },
            ));
            if args.mode.executes() {
                Ok(task_result(TaskActionResult::StartExecution(
                    dw_task::start::execute_start_pr(report, &args).await?,
                )))
            } else {
                Ok(task_result(TaskActionResult::StartPrPlan(report)))
            }
        }
        DwActionRequest::TaskPreflight(args) => Ok(task_result(TaskActionResult::Preflight(
            dw_task::validate::preflight_report(args)?,
        ))),
        DwActionRequest::TaskHandoffValidate(args) => Ok(task_result(
            TaskActionResult::HandoffValidate(dw_task::validate::handoff_validation_report(args)?),
        )),
        DwActionRequest::TaskSync(args) => Ok(task_result(TaskActionResult::Sync(
            dw_task::lifecycle::sync_report(args).await?,
        ))),
        DwActionRequest::TaskRename(args) => {
            let plan = dw_task::lifecycle::rename_plan(args.clone())?;
            if args.mode.executes() {
                Ok(task_result(TaskActionResult::RenameExecution(
                    dw_task::lifecycle::execute_rename(&plan)?,
                )))
            } else {
                Ok(task_result(TaskActionResult::RenamePlan(plan)))
            }
        }
        DwActionRequest::TaskRepoLatest(args) => {
            let plan = dw_task::repo::repo_latest_plan(args.clone())?;
            if args.mode.executes() {
                let execution = dw_task::repo::execute_repo_latest(&plan)?;
                Ok(task_result(TaskActionResult::RepoLatestExecution {
                    plan,
                    execution,
                }))
            } else {
                Ok(task_result(TaskActionResult::RepoLatestPlan(plan)))
            }
        }
        DwActionRequest::TaskCommit(args) => {
            let plan = dw_task::repo::commit_plan(args.clone())?;
            if args.mode.executes() {
                let execution = dw_task::repo::execute_commit(&plan)?;
                Ok(task_result(TaskActionResult::CommitExecution {
                    plan,
                    execution,
                }))
            } else {
                Ok(task_result(TaskActionResult::CommitPlan(plan)))
            }
        }
        DwActionRequest::TaskAddRepo(args) => {
            let plan = dw_task::repo::add_repo_plan(args.clone())?;
            if args.mode.executes() {
                let execution = dw_task::repo::execute_add_repo(&plan)?;
                Ok(task_result(TaskActionResult::AddRepoExecution {
                    plan,
                    execution,
                }))
            } else {
                Ok(task_result(TaskActionResult::AddRepoPlan(plan)))
            }
        }
        DwActionRequest::TaskTeardown(args) => {
            let plan = dw_task::repo::teardown_plan(args.clone())?;
            if args.mode.executes() && plan.workspace.is_some() {
                Ok(task_result(TaskActionResult::TeardownExecution(
                    dw_task::repo::execute_teardown(&plan)?,
                )))
            } else {
                Ok(task_result(TaskActionResult::TeardownPlan {
                    plan,
                    execute_requested: args.mode.executes(),
                }))
            }
        }
        DwActionRequest::TaskFinish(args) => {
            let plan = dw_task::finish::finish_plan(args.clone())?;
            if args.mode.executes() {
                let execution =
                    dw_task::finish::execute_finish_with_events(plan, &args, &mut |event| {
                        emit(DwActionEvent::Task(event))
                    })
                    .await?;
                Ok(task_result(TaskActionResult::FinishExecution(execution)))
            } else {
                Ok(task_result(TaskActionResult::FinishPlan(plan)))
            }
        }
        DwActionRequest::TaskPrune(args) => {
            let plan = dw_task::prune::plan(args.clone()).await?;
            if args.mode.executes() {
                let candidates = args
                    .selected_workspaces
                    .as_ref()
                    .map(|selected_workspaces| {
                        plan.candidates
                            .iter()
                            .filter(|candidate| selected_workspaces.contains(&candidate.path))
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_else(|| plan.candidates.clone());
                Ok(task_result(TaskActionResult::PruneExecution(
                    dw_task::prune::execute(&plan.root, candidates)?,
                )))
            } else {
                Ok(task_result(TaskActionResult::PrunePlan(plan)))
            }
        }
        DwActionRequest::TaskCreateChildTask(args) => {
            Ok(task_result(TaskActionResult::CreateChildTask(
                dw_task::lifecycle::create_child_task_report(args).await?,
            )))
        }
        DwActionRequest::TaskAddWorkItem(args) => {
            let plan = dw_task::work_item::add_plan(args.clone()).await?;
            if args.mode.executes() {
                let execution = dw_task::work_item::execute_update(&plan)?;
                Ok(task_result(TaskActionResult::WorkItemExecution {
                    plan,
                    execution,
                }))
            } else {
                Ok(task_result(TaskActionResult::WorkItemPlan(plan)))
            }
        }
        DwActionRequest::TaskRemoveWorkItem(args) => {
            let plan = dw_task::work_item::remove_plan(args.clone())?;
            if args.mode.executes() {
                let execution = dw_task::work_item::execute_update(&plan)?;
                Ok(task_result(TaskActionResult::WorkItemExecution {
                    plan,
                    execution,
                }))
            } else {
                Ok(task_result(TaskActionResult::WorkItemPlan(plan)))
            }
        }
        DwActionRequest::SecretGet { key } => Ok(DwActionResult::Secret(SecretActionResult::Get(
            dw_secret::command::get_secret(&key)?,
        ))),
        DwActionRequest::SecretSetFromEnv { key, env } => {
            let secret = dw_secret::secret_from_env(&env)?;
            Ok(DwActionResult::Secret(SecretActionResult::Set(
                dw_secret::command::set_secret(&key, &secret)?,
            )))
        }
        DwActionRequest::SecretSet { key, value } => {
            let value = match value {
                Some(value) => value,
                None => request_secret_value(&key, emit, input_receiver.as_deref_mut()).await?,
            };
            Ok(DwActionResult::Secret(SecretActionResult::Set(
                dw_secret::command::set_secret(&key, &value)?,
            )))
        }
        DwActionRequest::SecretDelete { key, confirmed } => {
            if !confirmed {
                confirm_secret_delete(&key, emit, input_receiver).await?;
            }
            Ok(DwActionResult::Secret(SecretActionResult::Delete(
                dw_secret::command::delete_secret_key(&key)?,
            )))
        }
        DwActionRequest::Upgrade { check, rid } => {
            let report = dw_upgrade::handle_upgrade(check, rid).await?;
            Ok(DwActionResult::Upgrade(UpgradeActionResult::Report(report)))
        }
    }
}

async fn resolve_task_start_input(
    mut args: dw_task::start::StartArgs,
    emit: &mut impl FnMut(DwActionEvent),
    mut input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<dw_task::start::StartArgs> {
    if args.project.is_none() {
        args.project =
            request_task_start_project(&args, emit, input_receiver.as_deref_mut()).await?;
    }

    if args.repositories.is_empty() {
        args.repositories =
            request_task_start_repositories(&args, emit, input_receiver.as_deref_mut()).await?;
    }

    if !args.work_item_ids.is_empty() {
        return Ok(args);
    }

    let work_item_id = if args.skip_ado {
        request_work_item_text(emit, input_receiver.as_deref_mut()).await?
    } else if let Some(project) = args.project.clone() {
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
        resolve_work_item_id_from_assigned_report(&report, emit, input_receiver.as_deref_mut())
            .await?
    } else {
        request_work_item_text(emit, input_receiver).await?
    };
    args.work_item_ids = vec![work_item_id];
    Ok(args)
}

async fn request_task_start_project(
    args: &dw_task::start::StartArgs,
    emit: &mut impl FnMut(DwActionEvent),
    input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<Option<ProjectKey>> {
    let root = dw_config::resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let projects = dw_config::load_projects_config(&root);
    let choices = dw_config::project_choices(&projects);
    if choices.is_empty() {
        return Ok(None);
    }

    let request = InputRequest::SelectOne {
        id: "project".into(),
        label: "Project".into(),
        help: Some("Choose the ADO project for the task workspace".into()),
        choices: choices
            .iter()
            .map(|choice| PromptChoice::new(choice.key.clone(), choice.to_string()))
            .collect(),
    };
    match request_input(emit, input_receiver, request).await? {
        InputResponse::SelectOne { value } => Ok(Some(ProjectKey::from(value.as_str()))),
        response => anyhow::bail!("input response kind mismatch for project: {response:?}"),
    }
}

async fn request_task_start_repositories(
    args: &dw_task::start::StartArgs,
    emit: &mut impl FnMut(DwActionEvent),
    input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<Vec<WorkspaceRepositoryName>> {
    let Some(project) = args.project.as_ref() else {
        return Ok(Vec::new());
    };
    let root = dw_config::resolve_root(args.root.as_ref().map(DevWorkflowRoot::as_str));
    let projects = dw_config::load_projects_config(&root);
    let Some(project_config) = dw_config::resolve_project(&projects, project.as_str()) else {
        return Ok(Vec::new());
    };
    let repositories = project_config
        .repositories
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if repositories.len() <= 1 {
        return Ok(Vec::new());
    }

    let request = InputRequest::SelectMany {
        id: "repositories".into(),
        label: "Repositories".into(),
        help: Some("Leave empty to include every configured repository".into()),
        choices: repositories
            .into_iter()
            .map(|repository| PromptChoice::new(repository.clone(), repository))
            .collect(),
    };
    match request_input(emit, input_receiver, request).await? {
        InputResponse::SelectMany { values } => Ok(values
            .into_iter()
            .map(|value| WorkspaceRepositoryName::from(value.as_str()))
            .collect()),
        response => anyhow::bail!("input response kind mismatch for repositories: {response:?}"),
    }
}

async fn request_secret_value(
    key: &SecretKey,
    emit: &mut impl FnMut(DwActionEvent),
    input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<SecretValue> {
    let request = InputRequest::Secret {
        id: format!("secret-set:{key}").into(),
        label: format!("Secret value for `{key}`"),
        help: Some("Hidden input; value is sent only to the central action runtime".into()),
    };
    match request_input(emit, input_receiver, request).await? {
        InputResponse::Secret { value } => Ok(SecretValue::from(value)),
        response => anyhow::bail!("input response kind mismatch for secret `{key}`: {response:?}"),
    }
}

async fn confirm_secret_delete(
    key: &SecretKey,
    emit: &mut impl FnMut(DwActionEvent),
    input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<()> {
    let request = InputRequest::Confirm {
        id: format!("secret-delete:{key}").into(),
        label: format!("Delete secret `{key}` from the system keyring?"),
        help: Some("This removes the local entry if it exists.".into()),
        default: false,
    };
    match request_input(emit, input_receiver, request).await? {
        InputResponse::Confirm { accepted: true } => Ok(()),
        InputResponse::Confirm { accepted: false } => {
            anyhow::bail!("Secret deletion canceled.")
        }
        response => anyhow::bail!("input response kind mismatch for secret `{key}`: {response:?}"),
    }
}

async fn resolve_work_item_id_from_assigned_report(
    report: &dw_ado_commands::commands::assigned::AssignedReport,
    emit: &mut impl FnMut(DwActionEvent),
    mut input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<WorkItemId> {
    if report.items.is_empty() {
        return request_work_item_text(emit, input_receiver).await;
    }

    let spec = dw_ado_commands::commands::assigned::assigned_work_item_prompt_spec(&report.items);
    let response = request_input(
        emit,
        input_receiver.as_deref_mut(),
        input_request_from_prompt_spec(&spec),
    )
    .await?;
    let InputResponse::SelectOne { value } = response else {
        anyhow::bail!("input response kind mismatch for `{}`", spec.id);
    };
    if value.as_str() == dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_VALUE {
        request_work_item_text(emit, input_receiver).await
    } else {
        Ok(WorkItemId::from(value.as_str()))
    }
}

async fn request_work_item_text(
    emit: &mut impl FnMut(DwActionEvent),
    input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
) -> Result<WorkItemId> {
    let request = input_request_from_prompt_spec(&PromptSpec::text("work-item-id", "Work item ID"));
    match request_input(emit, input_receiver, request).await? {
        InputResponse::Text { value } => Ok(WorkItemId::from(value)),
        response => anyhow::bail!("input response kind mismatch for work-item-id: {response:?}"),
    }
}

async fn request_input(
    emit: &mut impl FnMut(DwActionEvent),
    input_receiver: Option<&mut UnboundedReceiver<InputResponse>>,
    request: InputRequest,
) -> Result<InputResponse> {
    emit(DwActionEvent::NeedsInput {
        request: request.clone(),
    });
    let Some(input_receiver) = input_receiver else {
        anyhow::bail!(
            "action requires input `{}` but this runtime has no input responder",
            request.id()
        );
    };
    let response = input_receiver.recv().await.ok_or_else(|| {
        anyhow::anyhow!(
            "action input `{}` was cancelled before a response",
            request.id()
        )
    })?;
    validate_input_response(&request, &response)?;
    Ok(response)
}

fn input_request_from_prompt_spec(spec: &PromptSpec) -> InputRequest {
    match spec.kind {
        PromptKind::Confirm => InputRequest::Confirm {
            id: spec.id.clone(),
            label: spec.label.clone(),
            help: spec.help.clone(),
            default: false,
        },
        PromptKind::Select => InputRequest::SelectOne {
            id: spec.id.clone(),
            label: spec.label.clone(),
            help: spec.help.clone(),
            choices: spec.choices.clone(),
        },
        PromptKind::MultiSelect => InputRequest::SelectMany {
            id: spec.id.clone(),
            label: spec.label.clone(),
            help: spec.help.clone(),
            choices: spec.choices.clone(),
        },
        PromptKind::Text => InputRequest::Text {
            id: spec.id.clone(),
            label: spec.label.clone(),
            help: spec.help.clone(),
            default: None,
        },
    }
}

fn validate_input_response(request: &InputRequest, response: &InputResponse) -> Result<()> {
    match (request, response) {
        (InputRequest::Confirm { .. }, InputResponse::Confirm { .. })
        | (InputRequest::Text { .. }, InputResponse::Text { .. })
        | (InputRequest::Secret { .. }, InputResponse::Secret { .. }) => Ok(()),
        (InputRequest::SelectOne { choices, .. }, InputResponse::SelectOne { value }) => {
            validate_choice_value(request, choices, value)
        }
        (InputRequest::SelectMany { choices, .. }, InputResponse::SelectMany { values }) => {
            for value in values {
                validate_choice_value(request, choices, value)?;
            }
            Ok(())
        }
        _ => anyhow::bail!("input response kind mismatch for `{}`", request.id()),
    }
}

fn validate_choice_value(
    request: &InputRequest,
    choices: &[dw_core::PromptChoice],
    value: &PromptChoiceValue,
) -> Result<()> {
    if choices.iter().any(|choice| choice.value == *value) {
        Ok(())
    } else {
        anyhow::bail!("invalid input choice `{value}` for `{}`", request.id())
    }
}

fn task_result(result: TaskActionResult) -> DwActionResult {
    DwActionResult::Task(Box::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_input_emits_typed_request_and_waits_for_response() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        sender
            .send(InputResponse::Text {
                value: "55264".into(),
            })
            .expect("response channel should be open");
        let request = InputRequest::Text {
            id: "work-item-id".into(),
            label: "Work item ID".into(),
            help: None,
            default: None,
        };
        let mut events = Vec::new();

        let response = request_input(
            &mut |event| events.push(event),
            Some(&mut receiver),
            request,
        )
        .await
        .expect("text response should be accepted");

        assert_eq!(
            response,
            InputResponse::Text {
                value: "55264".into()
            }
        );
        assert!(matches!(
            events.as_slice(),
            [DwActionEvent::NeedsInput {
                request: InputRequest::Text { id, .. }
            }] if id.as_str() == "work-item-id"
        ));
    }

    #[tokio::test]
    async fn request_input_rejects_select_value_outside_request_choices() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        sender
            .send(InputResponse::SelectOne {
                value: "unknown".into(),
            })
            .expect("response channel should be open");
        let request = InputRequest::SelectOne {
            id: "assigned-work-item".into(),
            label: "Work item Azure DevOps".into(),
            help: None,
            choices: vec![dw_core::PromptChoice::new("55264", "#55264")],
        };
        let mut events = Vec::new();

        let error = request_input(
            &mut |event| events.push(event),
            Some(&mut receiver),
            request,
        )
        .await
        .expect_err("unknown choice value should fail");

        assert!(error.to_string().contains("invalid input choice"));
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn secret_delete_confirmation_uses_duplex_input() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        sender
            .send(InputResponse::Confirm { accepted: true })
            .expect("response channel should be open");
        let mut events = Vec::new();

        confirm_secret_delete(
            &SecretKey::from("db/password"),
            &mut |event| events.push(event),
            Some(&mut receiver),
        )
        .await
        .expect("accepted confirmation should continue");

        assert!(matches!(
            events.as_slice(),
            [DwActionEvent::NeedsInput {
                request: InputRequest::Confirm { id, default: false, .. }
            }] if id.as_str() == "secret-delete:db/password"
        ));
    }

    #[tokio::test]
    async fn secret_value_uses_hidden_duplex_input() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        sender
            .send(InputResponse::Secret {
                value: "s3cr3t".into(),
            })
            .expect("response channel should be open");
        let mut events = Vec::new();

        let value = request_secret_value(
            &SecretKey::from("db/password"),
            &mut |event| events.push(event),
            Some(&mut receiver),
        )
        .await
        .expect("secret input should be accepted");

        assert_eq!(value, SecretValue::from("s3cr3t"));
        assert!(matches!(
            events.as_slice(),
            [DwActionEvent::NeedsInput {
                request: InputRequest::Secret { id, .. }
            }] if id.as_str() == "secret-set:db/password"
        ));
    }
}
