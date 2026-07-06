use anyhow::Result;
use dw_core::{
    Agent, ConfigColorMode, ConfigRootPath, DevWorkflowRoot, DwActionEvent,
    EnvironmentVariableName, RuntimeIdentifier, SecretKey, TaskActionEvent,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub enum DwActionRequest {
    Version,
    Doctor,
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
    DbGuard(dw_db::commands::GuardArgs),
    DbSchema(dw_db::commands::SchemaArgs),
    DbDescribe(dw_db::commands::DescribeArgs),
    DbQuery(dw_db::commands::QueryArgs),
    AdoAssigned(dw_ado_commands::commands::assigned::AssignedArgs),
    AdoPrs(dw_ado_commands::commands::prs::PrsArgs),
    AdoChangelog(dw_ado_commands::commands::changelog::ChangelogArgs),
    AdoContext(dw_ado_commands::commands::context::ContextArgs),
    AdoAiContext(dw_ado_commands::commands::context::AiContextArgs),
    AdoWorkItem(dw_ado_commands::commands::work_item::WorkItemArgs),
    AdoSetState(dw_ado_commands::commands::set_state::SetStateArgs),
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
    SecretDelete {
        key: SecretKey,
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
    Assigned(dw_ado_commands::commands::assigned::AssignedReport),
    Prs(dw_ado_commands::commands::prs::PrsReport),
    Changelog(dw_ado_commands::commands::changelog::ChangelogReport),
    Context(dw_ado_commands::commands::context::ContextReport),
    AiContext(dw_ado_commands::commands::context::AiContextReport),
    WorkItem(dw_ado_commands::commands::work_item::WorkItemReport),
    SetState(dw_ado_commands::commands::set_state::SetStateExecutionReport),
}

#[derive(Debug, Clone)]
pub enum TaskActionResult {
    StartPlan(dw_task::start::StartPlanReport),
    StartExecution(dw_task::start::StartExecutionReport),
    StartPrPlan(dw_task::start::StartPrPlanReport),
    Preflight(dw_contracts::TaskPreflightReport),
    HandoffValidate(dw_contracts::TaskHandoffValidationReport),
    Sync(dw_task::lifecycle::SyncReport),
    RenamePlan(dw_task::lifecycle::RenamePlanReport),
    RenameExecution(dw_task::lifecycle::RenameExecutionReport),
    RepoLatest {
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
    pub result: JoinHandle<Result<DwActionResult>>,
}

pub fn spawn_action(request: DwActionRequest) -> DwActionRun {
    match request {
        DwActionRequest::Upgrade { check, rid } => spawn_upgrade_action(check, rid),
        request => spawn_callback_action(request),
    }
}

fn spawn_callback_action(request: DwActionRequest) -> DwActionRun {
    let (sender, receiver) = mpsc::unbounded_channel();
    let result = tokio::spawn(async move {
        run_action(request, |event| {
            let _ = sender.send(event);
        })
        .await
    });
    DwActionRun {
        events: receiver,
        result,
    }
}

fn spawn_upgrade_action(check: bool, rid: Option<RuntimeIdentifier>) -> DwActionRun {
    let upgrade = dw_upgrade::spawn_upgrade(check, rid);
    let (sender, receiver) = mpsc::unbounded_channel();
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
        result,
    }
}

pub async fn run_action(
    request: DwActionRequest,
    mut emit: impl FnMut(DwActionEvent),
) -> Result<DwActionResult> {
    match request {
        DwActionRequest::Version => Ok(DwActionResult::App(AppActionResult::Version {
            version: env!("DW_VERSION").into(),
        })),
        DwActionRequest::Guide => Ok(DwActionResult::App(AppActionResult::Guide {
            topic: GuideTopic::Main,
        })),
        DwActionRequest::Doctor => Ok(DwActionResult::Doctor(dw_doctor::run_doctor(false)?)),
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
        DwActionRequest::AdoSetState(args) => {
            let plan = dw_ado_commands::commands::set_state::plan(args)?;
            let execution =
                dw_ado_commands::commands::set_state::execute_with_events(plan, &mut |event| {
                    emit(DwActionEvent::Ado(event))
                })
                .await?;
            Ok(DwActionResult::Ado(AdoActionResult::SetState(execution)))
        }
        DwActionRequest::TaskStart(args) => {
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
            let plan = dw_task::repo::repo_latest_plan(args)?;
            let execution = dw_task::repo::execute_repo_latest(&plan)?;
            Ok(task_result(TaskActionResult::RepoLatest {
                plan,
                execution,
            }))
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
                Ok(task_result(TaskActionResult::PruneExecution(
                    dw_task::prune::execute(&plan.root, plan.candidates.clone())?,
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
        DwActionRequest::SecretDelete { key } => Ok(DwActionResult::Secret(
            SecretActionResult::Delete(dw_secret::command::delete_secret_key(&key)?),
        )),
        DwActionRequest::Upgrade { check, rid } => {
            let report = dw_upgrade::handle_upgrade(check, rid).await?;
            Ok(DwActionResult::Upgrade(UpgradeActionResult::Report(report)))
        }
    }
}

fn task_result(result: TaskActionResult) -> DwActionResult {
    DwActionResult::Task(Box::new(result))
}
