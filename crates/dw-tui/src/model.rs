use dw_app::DwActionResult;
use dw_config::{
    ConfigDoctorReport, ConfigShow, DatabasesConfig, ProjectsConfig, WorkflowConfig, config_doctor,
    load_databases_config, load_projects_config, load_user_settings, load_workflow_config,
    project_choices, repository_config, resolve_project, resolve_root, root_status,
};
use dw_core::{
    AdoRepositoryName, Agent, ConfigColorMode, ConfigRootPath, DevWorkflowRoot, DwActionEvent,
    EnvironmentVariableName, ProjectKey, PullRequestId, SecretKey, WorkItemId, WorkItemState,
    WorkspaceRepositoryName,
};
use dw_workspace::{TaskListItem, plan_task_prune, task_list};

const GUIDE_DETAIL_LINE_COUNT: usize = 29;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Workspaces,
    Ado,
    PullRequests,
    Db,
    Composer,
}

impl View {
    pub const ALL: [View; 6] = [
        View::Dashboard,
        View::Workspaces,
        View::Ado,
        View::PullRequests,
        View::Db,
        View::Composer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            View::Dashboard => "Dashboard",
            View::Workspaces => "Workspaces",
            View::Ado => "ADO",
            View::PullRequests => "PRs",
            View::Db => "DB",
            View::Composer => "Composer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRisk {
    Safe,
    OpensExternal,
    DryRun,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceAction {
    Open,
    Preflight,
    Sync,
    RepoLatest,
    HandoffValidate,
    CommitPreview,
    FinishPreview,
    FinishExecute,
    TeardownPreview,
    TeardownExecute,
}

impl WorkspaceAction {
    const CATALOG: [WorkspaceAction; 10] = [
        WorkspaceAction::Open,
        WorkspaceAction::Preflight,
        WorkspaceAction::Sync,
        WorkspaceAction::RepoLatest,
        WorkspaceAction::HandoffValidate,
        WorkspaceAction::CommitPreview,
        WorkspaceAction::FinishPreview,
        WorkspaceAction::FinishExecute,
        WorkspaceAction::TeardownPreview,
        WorkspaceAction::TeardownExecute,
    ];
}

impl ActionRisk {
    pub fn confirmation_title(self) -> &'static str {
        match self {
            ActionRisk::Safe => "Confirmation",
            ActionRisk::OpensExternal => "Open external process",
            ActionRisk::DryRun => "Preview confirmation",
            ActionRisk::Destructive => "Destructive confirmation",
        }
    }

    pub fn risk_label(self) -> &'static str {
        match self {
            ActionRisk::Safe => "Read/inspect",
            ActionRisk::OpensExternal => "Opens a tool or interactive flow",
            ActionRisk::DryRun => "Preview, no expected modification",
            ActionRisk::Destructive => "Modifies or deletes data/workspaces",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TuiAction {
    pub label: String,
    pub request: TuiActionRequest,
    pub description: String,
    pub kind: ActionRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Version,
    Doctor,
    Guide,
    Refresh,
    ConfigShow,
    ConfigInit,
    ConfigDoctor,
    ConfigSetColor,
    ConfigSetRoot,
    AgentConfig,
    AgentSetDefault,
    AgentDoctor,
    AgentOpen,
    DbGuard,
    DbSchema,
    DbDescribe,
    DbQuery,
    AdoAssigned,
    AdoPrs,
    AdoChangelog,
    AdoContext,
    AdoAiContext,
    AdoWorkItem,
    AdoSetState,
    TaskStart,
    TaskStartPr,
    TaskPreflight,
    TaskHandoffValidate,
    TaskSync,
    TaskRename,
    TaskRepoLatest,
    TaskCommit,
    TaskAddRepo,
    TaskTeardown,
    TaskFinish,
    TaskPrune,
    TaskCreateChildTask,
    TaskAddWorkItem,
    TaskRemoveWorkItem,
    SecretGet,
    SecretSetFromEnv,
    SecretDelete,
}

#[derive(Debug, Clone)]
pub struct CockpitItem {
    pub section: &'static str,
    pub title: String,
    pub subtitle: String,
    pub status: String,
    pub severity: CockpitSeverity,
    pub primary_action: TuiAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CockpitSeverity {
    Normal,
    Attention,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionEffect {
    ColorMode(ConfigColorMode),
    DefaultAgent(Agent),
    Root(String),
    InitializedRoot(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TuiActionRequest {
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
    AgentOpen(dw_task::open::OpenWorkspaceArgs),
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
}

#[derive(Debug, Clone)]
pub struct DetailPanel {
    pub content: DetailPanelContent,
    pub scroll: usize,
}

#[derive(Debug, Clone)]
pub enum DetailPanelContent {
    Guide,
    ConfigShow(ConfigShow),
    ConfigDoctor(ConfigDoctorReport),
    AgentDoctor(dw_agent::command::AgentDoctorReport),
    ActionResult {
        title: String,
        events: Vec<DwActionEvent>,
        result: DwActionResult,
    },
}

impl DetailPanel {
    pub fn guide() -> Self {
        Self {
            content: DetailPanelContent::Guide,
            scroll: 0,
        }
    }

    pub fn config_show(report: &ConfigShow) -> Self {
        Self {
            content: DetailPanelContent::ConfigShow(report.clone()),
            scroll: 0,
        }
    }

    pub fn config_doctor(report: &ConfigDoctorReport) -> Self {
        Self {
            content: DetailPanelContent::ConfigDoctor(report.clone()),
            scroll: 0,
        }
    }

    pub fn agent_doctor(report: &dw_agent::command::AgentDoctorReport) -> Self {
        Self {
            content: DetailPanelContent::AgentDoctor(report.clone()),
            scroll: 0,
        }
    }

    pub fn action_result(
        title: impl Into<String>,
        events: Vec<DwActionEvent>,
        result: DwActionResult,
    ) -> Self {
        Self {
            content: DetailPanelContent::ActionResult {
                title: title.into(),
                events,
                result,
            },
            scroll: 0,
        }
    }

    pub fn title(&self) -> String {
        match &self.content {
            DetailPanelContent::Guide => "DevWorkflow guide".into(),
            DetailPanelContent::ConfigShow(_) => "Effective configuration".into(),
            DetailPanelContent::ConfigDoctor(_) => "Configuration doctor".into(),
            DetailPanelContent::AgentDoctor(_) => "Agent doctor".into(),
            DetailPanelContent::ActionResult { title, .. } => title.clone(),
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_home(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_end(&mut self) {
        self.scroll = self.content.line_count().saturating_sub(1);
    }
}

impl DetailPanelContent {
    fn line_count(&self) -> usize {
        match self {
            DetailPanelContent::Guide => GUIDE_DETAIL_LINE_COUNT,
            DetailPanelContent::ConfigShow(_) => 10,
            DetailPanelContent::ConfigDoctor(report) => {
                let header = 4;
                let result = 3;
                header
                    + report.checks.len()
                    + report
                        .checks
                        .iter()
                        .filter(|check| check.message.is_some())
                        .count()
                    + result
            }
            DetailPanelContent::AgentDoctor(report) => {
                let header = 4;
                let footer = 2;
                header
                    + report.checks.len()
                    + report
                        .checks
                        .iter()
                        .filter(|check| !check.available)
                        .count()
                    + footer
            }
            DetailPanelContent::ActionResult { events, .. } => events.len().max(1),
        }
    }
}

impl TuiAction {
    pub fn with_root(mut self, root: String) -> Self {
        match &mut self.request {
            TuiActionRequest::TaskPreflight(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskSync(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskRepoLatest(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskHandoffValidate(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskCommit(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskAddRepo(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskFinish(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskTeardown(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::TaskPrune(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::AgentOpen(args) => {
                args.root = Some(dw_core::DevWorkflowRoot::from(root))
            }
            TuiActionRequest::ConfigShow { root: value }
            | TuiActionRequest::ConfigDoctor { root: value }
            | TuiActionRequest::AgentConfig { root: value }
            | TuiActionRequest::AgentSetDefault { root: value, .. } => {
                *value = Some(dw_core::DevWorkflowRoot::from(root))
            }
            _ => {}
        }
        self
    }

    pub fn display_label(&self) -> String {
        self.label.clone()
    }

    pub fn action_kind(&self) -> ActionKind {
        match &self.request {
            TuiActionRequest::Version => ActionKind::Version,
            TuiActionRequest::Doctor => ActionKind::Doctor,
            TuiActionRequest::Guide => ActionKind::Guide,
            TuiActionRequest::Refresh(_) => ActionKind::Refresh,
            TuiActionRequest::ConfigShow { .. } => ActionKind::ConfigShow,
            TuiActionRequest::ConfigInit(_) => ActionKind::ConfigInit,
            TuiActionRequest::ConfigDoctor { .. } => ActionKind::ConfigDoctor,
            TuiActionRequest::ConfigSetColor { .. } => ActionKind::ConfigSetColor,
            TuiActionRequest::ConfigSetRoot { .. } => ActionKind::ConfigSetRoot,
            TuiActionRequest::AgentConfig { .. } => ActionKind::AgentConfig,
            TuiActionRequest::AgentSetDefault { .. } => ActionKind::AgentSetDefault,
            TuiActionRequest::AgentDoctor { .. } => ActionKind::AgentDoctor,
            TuiActionRequest::AgentOpen(_) => ActionKind::AgentOpen,
            TuiActionRequest::DbGuard(_) => ActionKind::DbGuard,
            TuiActionRequest::DbSchema(_) => ActionKind::DbSchema,
            TuiActionRequest::DbDescribe(_) => ActionKind::DbDescribe,
            TuiActionRequest::DbQuery(_) => ActionKind::DbQuery,
            TuiActionRequest::AdoAssigned(_) => ActionKind::AdoAssigned,
            TuiActionRequest::AdoPrs(_) => ActionKind::AdoPrs,
            TuiActionRequest::AdoChangelog(_) => ActionKind::AdoChangelog,
            TuiActionRequest::AdoContext(_) => ActionKind::AdoContext,
            TuiActionRequest::AdoAiContext(_) => ActionKind::AdoAiContext,
            TuiActionRequest::AdoWorkItem(_) => ActionKind::AdoWorkItem,
            TuiActionRequest::AdoSetState(_) => ActionKind::AdoSetState,
            TuiActionRequest::TaskStart(_) => ActionKind::TaskStart,
            TuiActionRequest::TaskStartPr(_) => ActionKind::TaskStartPr,
            TuiActionRequest::TaskPreflight(_) => ActionKind::TaskPreflight,
            TuiActionRequest::TaskHandoffValidate(_) => ActionKind::TaskHandoffValidate,
            TuiActionRequest::TaskSync(_) => ActionKind::TaskSync,
            TuiActionRequest::TaskRename(_) => ActionKind::TaskRename,
            TuiActionRequest::TaskRepoLatest(_) => ActionKind::TaskRepoLatest,
            TuiActionRequest::TaskCommit(_) => ActionKind::TaskCommit,
            TuiActionRequest::TaskAddRepo(_) => ActionKind::TaskAddRepo,
            TuiActionRequest::TaskTeardown(_) => ActionKind::TaskTeardown,
            TuiActionRequest::TaskFinish(_) => ActionKind::TaskFinish,
            TuiActionRequest::TaskPrune(_) => ActionKind::TaskPrune,
            TuiActionRequest::TaskCreateChildTask(_) => ActionKind::TaskCreateChildTask,
            TuiActionRequest::TaskAddWorkItem(_) => ActionKind::TaskAddWorkItem,
            TuiActionRequest::TaskRemoveWorkItem(_) => ActionKind::TaskRemoveWorkItem,
            TuiActionRequest::SecretGet { .. } => ActionKind::SecretGet,
            TuiActionRequest::SecretSetFromEnv { .. } => ActionKind::SecretSetFromEnv,
            TuiActionRequest::SecretDelete { .. } => ActionKind::SecretDelete,
        }
    }

    pub fn runs_attached_in_tui(&self) -> bool {
        matches!(self.kind, ActionRisk::OpensExternal)
    }

    pub fn bypasses_cli_confirmation(&self) -> bool {
        match &self.request {
            TuiActionRequest::AdoSetState(args) => args.yes,
            TuiActionRequest::TaskFinish(args) => args.yes,
            TuiActionRequest::TaskTeardown(args) => args.yes,
            TuiActionRequest::TaskPrune(args) => args.yes,
            _ => false,
        }
    }

    pub fn descriptor(&self) -> Option<&'static dw_core::ActionDescriptor> {
        let id = match self.action_kind() {
            ActionKind::ConfigShow => "config.show",
            ActionKind::ConfigDoctor => "config.doctor",
            ActionKind::AdoAssigned => "ado.assigned",
            ActionKind::AdoSetState => "ado.set-state",
            ActionKind::DbQuery => "db.query",
            ActionKind::TaskStart | ActionKind::TaskStartPr => "task.start",
            ActionKind::TaskFinish => "task.finish",
            ActionKind::TaskTeardown | ActionKind::TaskPrune => "task.teardown",
            ActionKind::AgentOpen => "agent.open",
            _ => return None,
        };
        dw_core::action_descriptor(id)
    }

    pub fn should_refresh_after_success(&self) -> bool {
        match &self.request {
            TuiActionRequest::Refresh(_)
            | TuiActionRequest::ConfigInit(_)
            | TuiActionRequest::ConfigSetRoot { .. }
            | TuiActionRequest::ConfigSetColor { .. }
            | TuiActionRequest::AgentSetDefault { .. }
            | TuiActionRequest::AdoSetState(_)
            | TuiActionRequest::TaskSync(_)
            | TuiActionRequest::TaskCreateChildTask(_) => true,
            TuiActionRequest::TaskStart(args) => args.mode.executes(),
            TuiActionRequest::TaskStartPr(args) => args.mode.executes(),
            TuiActionRequest::TaskRename(args) => args.mode.executes(),
            TuiActionRequest::TaskRepoLatest(args) => args.mode.executes(),
            TuiActionRequest::TaskCommit(args) => args.mode.executes(),
            TuiActionRequest::TaskAddWorkItem(args) => args.mode.executes(),
            TuiActionRequest::TaskRemoveWorkItem(args) => args.mode.executes(),
            TuiActionRequest::TaskAddRepo(args) => args.mode.executes(),
            TuiActionRequest::TaskFinish(args) => args.mode.executes(),
            TuiActionRequest::TaskTeardown(args) => args.mode.executes(),
            TuiActionRequest::TaskPrune(args) => args.mode.executes(),
            _ => self
                .descriptor()
                .is_some_and(|descriptor| descriptor.refresh_after_success),
        }
    }

    pub fn opens_result_after_success(&self) -> bool {
        matches!(
            self.request,
            TuiActionRequest::Doctor
                | TuiActionRequest::Refresh(_)
                | TuiActionRequest::ConfigInit(_)
                | TuiActionRequest::AdoAssigned(_)
                | TuiActionRequest::AdoPrs(_)
                | TuiActionRequest::AdoChangelog(_)
                | TuiActionRequest::AdoContext(_)
                | TuiActionRequest::AdoAiContext(_)
                | TuiActionRequest::AdoWorkItem(_)
                | TuiActionRequest::DbGuard(_)
                | TuiActionRequest::DbSchema(_)
                | TuiActionRequest::DbDescribe(_)
                | TuiActionRequest::DbQuery(_)
                | TuiActionRequest::TaskStart(_)
                | TuiActionRequest::TaskStartPr(_)
                | TuiActionRequest::TaskPreflight(_)
                | TuiActionRequest::TaskHandoffValidate(_)
                | TuiActionRequest::TaskSync(_)
                | TuiActionRequest::TaskRename(_)
                | TuiActionRequest::TaskRepoLatest(_)
                | TuiActionRequest::TaskCommit(_)
                | TuiActionRequest::TaskAddRepo(_)
                | TuiActionRequest::TaskTeardown(_)
                | TuiActionRequest::TaskFinish(_)
                | TuiActionRequest::TaskPrune(_)
                | TuiActionRequest::TaskCreateChildTask(_)
                | TuiActionRequest::TaskAddWorkItem(_)
                | TuiActionRequest::TaskRemoveWorkItem(_)
                | TuiActionRequest::SecretGet { .. }
                | TuiActionRequest::SecretSetFromEnv { .. }
                | TuiActionRequest::SecretDelete { .. }
        )
    }

    pub fn successful_effect(&self) -> Option<ActionEffect> {
        match &self.request {
            TuiActionRequest::ConfigSetColor { mode } => Some(ActionEffect::ColorMode(*mode)),
            TuiActionRequest::AgentSetDefault { agent, .. } => {
                Some(ActionEffect::DefaultAgent(*agent))
            }
            TuiActionRequest::ConfigSetRoot { path } => Some(ActionEffect::Root(path.to_string())),
            TuiActionRequest::ConfigInit(args) => Some(ActionEffect::InitializedRoot(
                resolve_root(args.root.as_deref()),
            )),
            _ => None,
        }
    }

    pub fn is_workspace_action(&self) -> bool {
        matches!(
            self.request,
            TuiActionRequest::AgentOpen(_)
                | TuiActionRequest::TaskStart(_)
                | TuiActionRequest::TaskStartPr(_)
                | TuiActionRequest::TaskPreflight(_)
                | TuiActionRequest::TaskHandoffValidate(_)
                | TuiActionRequest::TaskSync(_)
                | TuiActionRequest::TaskRename(_)
                | TuiActionRequest::TaskRepoLatest(_)
                | TuiActionRequest::TaskCommit(_)
                | TuiActionRequest::TaskAddRepo(_)
                | TuiActionRequest::TaskTeardown(_)
                | TuiActionRequest::TaskFinish(_)
                | TuiActionRequest::TaskCreateChildTask(_)
                | TuiActionRequest::TaskAddWorkItem(_)
                | TuiActionRequest::TaskRemoveWorkItem(_)
        )
    }

    pub fn is_ado_action(&self) -> bool {
        matches!(
            self.request,
            TuiActionRequest::AdoAssigned(_)
                | TuiActionRequest::AdoPrs(_)
                | TuiActionRequest::AdoChangelog(_)
                | TuiActionRequest::AdoContext(_)
                | TuiActionRequest::AdoAiContext(_)
                | TuiActionRequest::AdoWorkItem(_)
                | TuiActionRequest::AdoSetState(_)
        )
    }

    pub fn is_db_action(&self) -> bool {
        matches!(
            self.request,
            TuiActionRequest::DbGuard(_)
                | TuiActionRequest::DbSchema(_)
                | TuiActionRequest::DbDescribe(_)
                | TuiActionRequest::DbQuery(_)
        )
    }

    pub fn workspace_path(&self) -> Option<&str> {
        match &self.request {
            TuiActionRequest::AgentOpen(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskPreflight(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskHandoffValidate(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskSync(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskRename(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskRepoLatest(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskCommit(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskAddRepo(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskTeardown(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskFinish(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskAddWorkItem(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            TuiActionRequest::TaskRemoveWorkItem(args) => {
                args.workspace.as_ref().map(dw_core::WorkspacePath::as_str)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TuiSnapshot {
    pub root: String,
    pub needs_init: bool,
    pub projects: ProjectsConfig,
    pub workflow: WorkflowConfig,
    pub databases: DatabasesConfig,
    pub database_entries: Vec<TuiDatabase>,
    pub config_doctor: ConfigDoctorReport,
    pub workspaces: Vec<TaskListItem>,
    pub assigned: Vec<AdoAssignedProject>,
    pub assigned_loaded: bool,
    pub pull_requests: Vec<TuiPullRequest>,
    pub pull_requests_loaded: bool,
    pub prune_candidates: usize,
    pub actions: Vec<TuiAction>,
    pub color_mode: ConfigColorMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiDatabase {
    pub project: Option<String>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoAssignedProject {
    pub key: ProjectKey,
    pub label: String,
    pub items: Vec<AdoAssignedItem>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoAssignedItem {
    pub id: WorkItemId,
    pub kind: String,
    pub state: WorkItemState,
    pub title: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiPullRequest {
    pub workspace: Option<String>,
    pub project: ProjectKey,
    pub repository: WorkspaceRepositoryName,
    pub ado_repository: AdoRepositoryName,
    pub branch: String,
    pub target_branch: String,
    pub pull_request_id: Option<PullRequestId>,
    pub title: Option<String>,
    pub is_draft: bool,
    pub work_item_ids: Vec<WorkItemId>,
    pub url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct PullRequestTarget {
    order: usize,
    project: ProjectKey,
    repository: WorkspaceRepositoryName,
    ado_repository: AdoRepositoryName,
    options: Option<dw_ado::AzureDevOpsOptions>,
}

impl TuiSnapshot {
    pub fn loading(root: Option<&str>) -> Self {
        let root = resolve_root(root);
        let needs_init = !root_status(Some(&root)).initialized;
        let projects = ProjectsConfig::default();
        let workflow = WorkflowConfig::default();
        let databases = DatabasesConfig::default();
        let database_entries = Vec::new();
        let config_doctor = ConfigDoctorReport {
            root: root.clone(),
            checks: Vec::new(),
            passed: false,
        };
        let actions = build_actions(&root, &projects, &databases, &[]);
        let color_mode = load_user_settings().color.unwrap_or(ConfigColorMode::Auto);
        Self {
            root,
            needs_init,
            projects,
            workflow,
            databases,
            database_entries,
            config_doctor,
            workspaces: Vec::new(),
            assigned: Vec::new(),
            assigned_loaded: false,
            pull_requests: Vec::new(),
            pull_requests_loaded: false,
            prune_candidates: 0,
            actions,
            color_mode,
        }
    }

    pub fn load(root: Option<&str>) -> Self {
        let root = resolve_root(root);
        let needs_init = !root_status(Some(&root)).initialized;
        let projects = load_projects_config(&root);
        let workflow = load_workflow_config(&root);
        let databases = load_databases_config(&root);
        let database_entries = database_entries_for_tui(&databases);
        let config_doctor = config_doctor(Some(&root));
        let workspaces = task_list(&root, None, None);
        let prune_candidates = plan_task_prune(&root, None, None).len();
        let actions = build_actions(&root, &projects, &databases, &workspaces);
        let color_mode = load_user_settings().color.unwrap_or(ConfigColorMode::Auto);
        Self {
            root,
            needs_init,
            projects,
            workflow,
            databases,
            database_entries,
            config_doctor,
            workspaces,
            assigned: Vec::new(),
            assigned_loaded: false,
            pull_requests: Vec::new(),
            pull_requests_loaded: false,
            prune_candidates,
            actions,
            color_mode,
        }
    }

    pub fn project_count(&self) -> usize {
        self.projects.projects.len()
    }

    pub fn repository_count(&self) -> usize {
        self.projects
            .projects
            .keys()
            .filter_map(|project| resolve_project(&self.projects, project))
            .map(|project| project.repositories.len())
            .sum()
    }

    pub fn database_count(&self) -> usize {
        self.databases.globals.len()
            + self
                .databases
                .projects
                .values()
                .filter_map(|value| {
                    value
                        .get("databases")
                        .and_then(serde_json::Value::as_object)
                })
                .map(serde_json::Map::len)
                .sum::<usize>()
    }

    pub fn workspace_for_work_item(
        &self,
        project: &ProjectKey,
        work_item_id: &WorkItemId,
    ) -> Option<&TaskListItem> {
        self.workspaces.iter().find(|workspace| {
            workspace.project.as_str() == project.as_str()
                && workspace
                    .all_known_work_item_ids
                    .iter()
                    .any(|id| id.as_str().eq_ignore_ascii_case(work_item_id.as_str()))
        })
    }

    pub fn selected_work_item_workspace(
        &self,
        selected_project: usize,
        selected_item: usize,
    ) -> Option<&TaskListItem> {
        let project = self.assigned.get(selected_project)?;
        let item = project.items.get(selected_item)?;
        self.workspace_for_work_item(&project.key, &item.id)
    }

    pub fn default_agent(&self) -> Agent {
        self.workflow
            .agent
            .as_ref()
            .map(|agent| agent.default.clone())
            .filter(|agent| !agent.trim().is_empty())
            .and_then(|agent| agent.parse::<Agent>().ok())
            .unwrap_or(Agent::Opencode)
    }

    pub fn assigned_count(&self) -> usize {
        self.assigned
            .iter()
            .map(|project| project.items.len())
            .sum::<usize>()
    }

    pub fn assigned_work_item_prompt_specs(&self) -> Vec<dw_core::PromptSpec> {
        self.assigned
            .iter()
            .filter(|project| !project.items.is_empty())
            .map(|project| {
                let mut choices = project
                    .items
                    .iter()
                    .map(|item| {
                        dw_core::PromptChoice::new(
                            item.id.to_string(),
                            assigned_item_prompt_label(item),
                        )
                    })
                    .collect::<Vec<_>>();
                choices.push(dw_core::PromptChoice::new(
                    dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_VALUE,
                    dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_LABEL,
                ));

                dw_core::PromptSpec::select(
                    format!("assigned-work-item:{}", project.key),
                    format!("Work item Azure DevOps · {}", project.label),
                    choices,
                )
                .with_help("Choose an assigned work item outside final states")
            })
            .collect()
    }
}

fn assigned_item_prompt_label(item: &AdoAssignedItem) -> String {
    format!(
        "#{}{}{}{}",
        item.id,
        if item.kind.is_empty() {
            String::new()
        } else {
            format!(" [{}]", item.kind)
        },
        if item.state.as_str().is_empty() {
            String::new()
        } else {
            format!(" ({})", item.state)
        },
        if item.title.is_empty() {
            String::new()
        } else {
            format!(" {}", item.title)
        }
    )
}

pub fn database_entries_for_tui(databases: &DatabasesConfig) -> Vec<TuiDatabase> {
    let mut entries = databases
        .globals
        .keys()
        .map(|key| TuiDatabase {
            project: None,
            key: key.clone(),
        })
        .collect::<Vec<_>>();
    for (project, value) in &databases.projects {
        let Some(items) = value
            .get("databases")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        entries.extend(items.keys().map(|key| TuiDatabase {
            project: Some(project.clone()),
            key: key.clone(),
        }));
    }
    entries
}

pub async fn load_assigned_data(
    root: String,
    projects: ProjectsConfig,
    workflow: WorkflowConfig,
) -> Vec<AdoAssignedProject> {
    let choices = project_choices(&projects);
    if choices.is_empty() {
        return Vec::new();
    }

    let token = match dw_ado::auth::require_token(
        dw_ado_commands::load_auth_options(Some(&root))
            .ok()
            .flatten(),
    )
    .await
    {
        Ok(token) => token,
        Err(error) => {
            return choices
                .into_iter()
                .map(|choice| AdoAssignedProject {
                    key: ProjectKey::from(choice.key),
                    label: choice.label,
                    items: Vec::new(),
                    error: Some(error.to_string()),
                })
                .collect();
        }
    };

    load_assigned_projects(choices, projects, workflow, token).await
}

async fn load_assigned_projects(
    choices: Vec<dw_config::ProjectChoice>,
    projects: ProjectsConfig,
    workflow: WorkflowConfig,
    token: dw_ado::auth::AdoToken,
) -> Vec<AdoAssignedProject> {
    let mut jobs = tokio::task::JoinSet::new();
    for (index, choice) in choices.into_iter().enumerate() {
        let projects = projects.clone();
        let workflow = workflow.clone();
        let token = token.clone();
        jobs.spawn(async move {
            (
                index,
                load_assigned_project(choice, projects, workflow, token).await,
            )
        });
    }

    let mut results = Vec::new();
    while let Some(result) = jobs.join_next().await {
        match result {
            Ok(item) => results.push(item),
            Err(error) => results.push((
                usize::MAX,
                AdoAssignedProject {
                    key: "-".into(),
                    label: "ADO project".into(),
                    items: Vec::new(),
                    error: Some(format!("Chargement ADO interrompu: {error}")),
                },
            )),
        }
    }
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, project)| project).collect()
}

async fn load_assigned_project(
    choice: dw_config::ProjectChoice,
    projects: ProjectsConfig,
    workflow: WorkflowConfig,
    token: dw_ado::auth::AdoToken,
) -> AdoAssignedProject {
    let project_key = ProjectKey::from(choice.key);
    match dw_ado_commands::resolve_options(&projects, &workflow, &project_key) {
        Ok(options) => match dw_ado::query_assigned_work_items(&options, 50, &token).await {
            Ok(items) => AdoAssignedProject {
                key: project_key.clone(),
                label: choice.label,
                items: items
                    .into_iter()
                    .filter(|item| {
                        !dw_ado::is_final_state(item.kind.as_deref(), item.state.as_deref())
                    })
                    .map(AdoAssignedItem::from)
                    .collect(),
                error: None,
            },
            Err(error) => AdoAssignedProject {
                key: project_key.clone(),
                label: choice.label,
                items: Vec::new(),
                error: Some(error.to_string()),
            },
        },
        Err(error) => AdoAssignedProject {
            key: project_key.clone(),
            label: choice.label,
            items: Vec::new(),
            error: Some(error.to_string()),
        },
    }
}

impl From<dw_ado::WorkItemSnapshot> for AdoAssignedItem {
    fn from(value: dw_ado::WorkItemSnapshot) -> Self {
        Self {
            id: WorkItemId::from(value.id),
            kind: value.kind.unwrap_or_else(|| "-".into()),
            state: WorkItemState::from(value.state.unwrap_or_else(|| "-".into())),
            title: value.title.unwrap_or_default(),
            url: value.url,
        }
    }
}

pub async fn load_pull_request_data(
    root: String,
    projects: ProjectsConfig,
    workflow: WorkflowConfig,
    workspaces: Vec<TaskListItem>,
) -> Vec<TuiPullRequest> {
    let choices = project_choices(&projects);

    let targets = pull_request_targets(&projects, &workflow, choices.clone());
    let token = match dw_ado::auth::require_token(
        dw_ado_commands::load_auth_options(Some(&root))
            .ok()
            .flatten(),
    )
    .await
    {
        Ok(token) => token,
        Err(error) => {
            return choices
                .into_iter()
                .map(|choice| TuiPullRequest {
                    workspace: None,
                    project: ProjectKey::from(choice.key),
                    repository: WorkspaceRepositoryName::from("-"),
                    ado_repository: AdoRepositoryName::from("-"),
                    branch: "-".into(),
                    target_branch: "-".into(),
                    pull_request_id: None,
                    title: None,
                    is_draft: false,
                    work_item_ids: Vec::new(),
                    url: None,
                    error: Some(error.to_string()),
                })
                .collect();
        }
    };

    load_pull_request_targets(targets, workspaces, token).await
}

fn pull_request_targets(
    projects: &ProjectsConfig,
    workflow: &WorkflowConfig,
    choices: Vec<dw_config::ProjectChoice>,
) -> Vec<PullRequestTarget> {
    let mut targets = Vec::new();
    for choice in choices {
        if is_aggregate_project_key(&choice.key) {
            continue;
        }
        let project_key = ProjectKey::from(choice.key);
        let project_config = resolve_project(projects, project_key.as_str());
        let options = dw_ado_commands::resolve_options(projects, workflow, &project_key).ok();
        let Some(project_config) = project_config.as_ref() else {
            continue;
        };
        for repository in project_config.repositories.keys() {
            let repository_config = repository_config(project_config, repository);
            let ado_repository = repository_config
                .as_ref()
                .and_then(|repository| repository.azure_dev_ops_repository.clone())
                .filter(|value| !value.trim().is_empty());
            let Some(ado_repository) = ado_repository else {
                continue;
            };
            targets.push(PullRequestTarget {
                order: targets.len(),
                project: project_key.clone(),
                repository: WorkspaceRepositoryName::from(repository.clone()),
                ado_repository: AdoRepositoryName::from(ado_repository),
                options: options.clone(),
            });
        }
    }
    targets
}

fn is_aggregate_project_key(key: &str) -> bool {
    key.trim().to_ascii_lowercase().starts_with("cross-")
}

async fn load_pull_request_targets(
    targets: Vec<PullRequestTarget>,
    workspaces: Vec<TaskListItem>,
    token: dw_ado::auth::AdoToken,
) -> Vec<TuiPullRequest> {
    let mut jobs = tokio::task::JoinSet::new();
    for target in targets {
        let token = token.clone();
        let workspaces = workspaces.clone();
        jobs.spawn_blocking(move || {
            let order = target.order;
            (order, load_pull_request_target(target, &workspaces, &token))
        });
    }

    let mut results = Vec::new();
    while let Some(result) = jobs.join_next().await {
        match result {
            Ok(item) => results.push(item),
            Err(error) => results.push((
                usize::MAX,
                vec![TuiPullRequest {
                    workspace: None,
                    project: ProjectKey::from("-"),
                    repository: WorkspaceRepositoryName::from("-"),
                    ado_repository: AdoRepositoryName::from("-"),
                    branch: "-".into(),
                    target_branch: "-".into(),
                    pull_request_id: None,
                    title: None,
                    is_draft: false,
                    work_item_ids: Vec::new(),
                    url: None,
                    error: Some(format!("Chargement PR interrompu: {error}")),
                }],
            )),
        }
    }
    results.sort_by_key(|(order, _)| *order);
    results
        .into_iter()
        .flat_map(|(_, items)| items)
        .collect::<Vec<_>>()
}

fn load_pull_request_target(
    target: PullRequestTarget,
    workspaces: &[TaskListItem],
    token: &dw_ado::auth::AdoToken,
) -> Vec<TuiPullRequest> {
    let Some(options) = target.options.as_ref() else {
        return vec![pull_request_error(
            target,
            "Configuration Azure DevOps introuvable.",
        )];
    };

    match dw_ado::list_active_pull_requests_authenticated(
        options,
        target.ado_repository.as_str(),
        token,
    ) {
        Ok(prs) => prs
            .into_iter()
            .map(|pr| {
                let branch = trim_branch(pr.source_ref_name.as_deref()).to_string();
                let workspace = workspaces
                    .iter()
                    .find(|workspace| {
                        workspace.project.as_str() == target.project.as_str()
                            && workspace.branch_name.as_str().eq_ignore_ascii_case(&branch)
                            && workspace
                                .repositories
                                .iter()
                                .any(|item| item.as_str() == target.repository.as_str())
                    })
                    .map(|workspace| workspace.path.to_string());
                TuiPullRequest {
                    workspace,
                    project: target.project.clone(),
                    repository: target.repository.clone(),
                    ado_repository: AdoRepositoryName::from(pr.repository),
                    branch,
                    target_branch: trim_branch(pr.target_ref_name.as_deref()).to_string(),
                    pull_request_id: Some(PullRequestId::from(pr.pull_request_id.to_string())),
                    title: pr.title,
                    is_draft: pr.is_draft,
                    work_item_ids: pr.work_item_ids.into_iter().map(WorkItemId::from).collect(),
                    url: Some(dw_ado::pull_request_web_url(
                        options,
                        target.ado_repository.as_str(),
                        pr.pull_request_id,
                    )),
                    error: None,
                }
            })
            .collect(),
        Err(error) => vec![pull_request_error(target, error.to_string())],
    }
}

fn pull_request_error(target: PullRequestTarget, error: impl Into<String>) -> TuiPullRequest {
    TuiPullRequest {
        workspace: None,
        project: target.project,
        repository: target.repository,
        ado_repository: target.ado_repository,
        branch: "-".into(),
        target_branch: "-".into(),
        pull_request_id: None,
        title: None,
        is_draft: false,
        work_item_ids: Vec::new(),
        url: None,
        error: Some(error.into()),
    }
}

fn trim_branch(value: Option<&str>) -> &str {
    value
        .unwrap_or("-")
        .strip_prefix("refs/heads/")
        .unwrap_or_else(|| value.unwrap_or("-"))
}

pub fn build_actions(
    root: &str,
    projects: &ProjectsConfig,
    databases: &DatabasesConfig,
    workspaces: &[TaskListItem],
) -> Vec<TuiAction> {
    let mut actions = vec![
        TuiAction {
            label: "Quick start".into(),
            request: TuiActionRequest::Guide,
            description: "Show the startup path".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Doctor".into(),
            request: TuiActionRequest::Doctor,
            description: "Check the machine and configuration".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Show configuration".into(),
            request: TuiActionRequest::ConfigShow {
                root: Some(root.into()),
            },
            description: "Show configuration paths".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Configuration doctor".into(),
            request: TuiActionRequest::ConfigDoctor {
                root: Some(root.into()),
            },
            description: "Validate configuration files".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Refresh".into(),
            request: TuiActionRequest::Refresh(dw_config::command::RefreshCommandArgs {
                root: Some(root.into()),
                profile: "business".into(),
            }),
            description: "Regenerate schemas and agent contexts".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "My work items".into(),
            request: TuiActionRequest::AdoAssigned(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(dw_core::DevWorkflowRoot::from(root)),
                    project: None,
                    top: 20,
                    all: false,
                    group_by_parent: false,
                },
            ),
            description: "List assigned work items".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "My grouped work items".into(),
            request: TuiActionRequest::AdoAssigned(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(dw_core::DevWorkflowRoot::from(root)),
                    project: None,
                    top: 20,
                    all: false,
                    group_by_parent: true,
                },
            ),
            description: "List assigned work items grouped by parent".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Test read-only SQL".into(),
            request: TuiActionRequest::DbGuard(dw_db::commands::GuardArgs {
                sql: dw_core::SqlQuery::from("select 1"),
            }),
            description: "Test the read-only SQL guard".into(),
            kind: ActionRisk::Safe,
        },
    ];

    for project in project_choices(projects) {
        actions.push(TuiAction {
            label: format!("Work items · {}", project.key),
            request: TuiActionRequest::AdoAssigned(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(dw_core::DevWorkflowRoot::from(root)),
                    project: Some(dw_core::ProjectKey::from(project.key.clone())),
                    top: 20,
                    all: false,
                    group_by_parent: false,
                },
            ),
            description: project.label,
            kind: ActionRisk::Safe,
        });
    }

    for workspace in workspaces.iter().take(30) {
        actions.extend(
            workspace_actions_for_tui(workspace)
                .into_iter()
                .map(|action| action.with_root(root.into())),
        );
    }

    actions.extend(database_actions(databases));
    actions
}

pub fn workspace_actions_for_tui(workspace: &TaskListItem) -> Vec<TuiAction> {
    WorkspaceAction::CATALOG
        .into_iter()
        .map(|action| workspace_action(workspace, action))
        .collect()
}

pub fn workspace_action(workspace: &TaskListItem, action: WorkspaceAction) -> TuiAction {
    let workspace_arg = workspace.path.clone();
    match action {
        WorkspaceAction::Open => TuiAction {
            label: "Open workspace".into(),
            request: TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                pull_request: None,
                r#continue: false,
                repo: None,
                agent: None,
            }),
            description: "Open the workspace with the configured agent".into(),
            kind: ActionRisk::OpensExternal,
        },
        WorkspaceAction::Preflight => TuiAction {
            label: "Check workspace".into(),
            request: TuiActionRequest::TaskPreflight(dw_task::validate::PreflightArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                r#continue: false,
                ai_context_files: Vec::new(),
            }),
            description: workspace.path.to_string(),
            kind: ActionRisk::Safe,
        },
        WorkspaceAction::Sync => TuiAction {
            label: "Sync ADO metadata".into(),
            request: TuiActionRequest::TaskSync(dw_task::lifecycle::SyncArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                r#continue: false,
            }),
            description: "Refresh task.json from Azure DevOps".into(),
            kind: ActionRisk::Safe,
        },
        WorkspaceAction::RepoLatest => TuiAction {
            label: "Update repositories".into(),
            request: TuiActionRequest::TaskRepoLatest(dw_task::repo::RepoLatestArgs {
                workspace: Some(workspace_arg),
                r#continue: false,
                repositories: Vec::new(),
                root: None,
                mode: dw_core::ExecutionMode::Execute,
            }),
            description: "Update repositories from their target branch".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::HandoffValidate => TuiAction {
            label: "Validate handoff".into(),
            request: TuiActionRequest::TaskHandoffValidate(
                dw_task::validate::HandoffValidateArgs {
                    workspace: Some(workspace_arg),
                    root: None,
                    project: None,
                    work_item_ids: Vec::new(),
                    r#continue: false,
                },
            ),
            description: "Validate handoffs".into(),
            kind: ActionRisk::Safe,
        },
        WorkspaceAction::CommitPreview => TuiAction {
            label: "Preview commit".into(),
            request: TuiActionRequest::TaskCommit(dw_task::repo::CommitArgs {
                workspace: Some(workspace_arg),
                r#continue: false,
                root: None,
                mode: dw_core::ExecutionMode::Preview,
                message: None,
            }),
            description: "Preview commits".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::FinishPreview => TuiAction {
            label: "Preview finish".into(),
            request: TuiActionRequest::TaskFinish(dw_task::finish::FinishArgs {
                workspace: Some(workspace_arg),
                r#continue: false,
                root: None,
                mode: dw_core::ExecutionMode::Preview,
                yes: false,
                message: None,
                create_pr: false,
                ready: false,
                skip_verify: false,
                skip_ado: false,
            }),
            description: "Preview finish".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::FinishExecute => TuiAction {
            label: "Finish workspace".into(),
            request: TuiActionRequest::TaskFinish(dw_task::finish::FinishArgs {
                workspace: Some(workspace_arg.clone()),
                r#continue: false,
                root: None,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
                message: None,
                create_pr: false,
                ready: false,
                skip_verify: false,
                skip_ado: false,
            }),
            description: "Finish the workspace: commit/push/PR/ADO according to options".into(),
            kind: ActionRisk::Destructive,
        },
        WorkspaceAction::TeardownPreview => TuiAction {
            label: "Preview removal".into(),
            request: TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                workspace: Some(workspace_arg.clone()),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                r#continue: false,
                mode: dw_core::ExecutionMode::Preview,
                yes: false,
            }),
            description: "Preview workspace removal".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::TeardownExecute => TuiAction {
            label: "Remove workspace".into(),
            request: TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                r#continue: false,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
            }),
            description: "Remove worktrees and the workspace".into(),
            kind: ActionRisk::Destructive,
        },
    }
}

fn database_actions(databases: &DatabasesConfig) -> Vec<TuiAction> {
    let mut actions = Vec::new();
    for key in databases.globals.keys() {
        actions.push(TuiAction {
            label: format!("Explore schema · {key}"),
            request: TuiActionRequest::DbSchema(dw_db::commands::SchemaArgs {
                project: None,
                database: Some(dw_core::DatabaseKey::from(key.clone())),
                env: None,
            }),
            description: "Global database".into(),
            kind: ActionRisk::Safe,
        });
    }
    for (project, value) in &databases.projects {
        let Some(items) = value
            .get("databases")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        for key in items.keys() {
            actions.push(TuiAction {
                label: format!("Explore schema · {project}/{key}"),
                request: TuiActionRequest::DbSchema(dw_db::commands::SchemaArgs {
                    project: Some(dw_core::ProjectKey::from(project.clone())),
                    database: Some(dw_core::DatabaseKey::from(key.clone())),
                    env: None,
                }),
                description: "Project database".into(),
                kind: ActionRisk::Safe,
            });
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_cover_major_domains() {
        let workspaces = vec![workspace_item()];
        let actions = build_actions(
            "/tmp/dw",
            &ProjectsConfig::default(),
            &DatabasesConfig::default(),
            &workspaces,
        );
        let labels = actions
            .iter()
            .map(|action| action.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.iter().any(|label| label.starts_with("Check")));
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("My work items"))
        );
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("Test read-only SQL")
                    || label.starts_with("Explore schema"))
        );
        assert!(labels.iter().any(|label| {
            label.starts_with("Show configuration") || label.starts_with("Configuration doctor")
        }));
    }

    #[test]
    fn root_is_carried_by_typed_requests_that_need_it() {
        let mut databases = DatabasesConfig::default();
        databases
            .globals
            .insert("shared".into(), serde_json::json!({}));
        let actions = build_actions("/tmp/dw", &ProjectsConfig::default(), &databases, &[]);
        let db_schema = actions
            .iter()
            .find(|action| action.label == "Explore schema · shared")
            .expect("db schema action");

        assert!(matches!(
            db_schema.request,
            TuiActionRequest::DbSchema(ref args)
                if args.project.is_none()
                    && args.database.as_ref().map(dw_core::DatabaseKey::as_str) == Some("shared")
        ));
    }

    #[test]
    fn repository_count_uses_resolved_project_repositories() {
        let mut snapshot = TuiSnapshot::loading(Some("/tmp/dw"));
        snapshot.projects = serde_json::from_str(
            r#"{
  "projects": {
    "base": {
      "displayName": "BASE",
      "repositories": {
        "shared": { "url": "", "defaultBranch": "develop" }
      }
    },
    "ha": {
      "displayName": "HA",
      "includedProjects": ["base"],
      "repositories": {
        "front": { "url": "", "defaultBranch": "develop" },
        "back": { "url": "", "defaultBranch": "main" }
      }
    }
  }
}"#,
        )
        .expect("projects config");

        assert_eq!(snapshot.repository_count(), 4);
    }

    #[test]
    fn typed_action_kind_is_separate_from_display_label() {
        let action = TuiAction {
            label: "Create workspace · ha #42".into(),
            request: TuiActionRequest::TaskStart(dw_task::start::StartArgs {
                work_item_ids: vec![dw_core::WorkItemId::from("42")],
                root: Some(dw_core::DevWorkflowRoot::from("/tmp/dw")),
                project: Some(dw_core::ProjectKey::from("ha")),
                task: None,
                type_name: None,
                repositories: Vec::new(),
                slug: None,
                skip_ado: true,
                with_active_children: false,
                create_child_tasks: false,
                mode: dw_core::ExecutionMode::Preview,
            }),
            description: "Prepare the local workspace".into(),
            kind: ActionRisk::DryRun,
        };

        assert_eq!(action.action_kind(), ActionKind::TaskStart);
        assert_eq!(action.display_label(), "Create workspace · ha #42");
    }

    #[test]
    fn workspace_actions_are_contextual_and_non_destructive_by_default() {
        let workspace = TaskListItem {
            path: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            work_items: vec![dw_workspace::WorkspaceWorkItem {
                id: "42".into(),
                kind: Some("User Story".into()),
                title: Some("Demo".into()),
                state: Some("Active".into()),
            }],
            task_id: None,
            all_known_work_item_ids: vec!["42".into()],
            kind: "feature".into(),
            slug: "demo".into(),
            branch_name: "feature/42-demo".into(),
            created_at: "2026-07-04T00:00:00Z".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Demo".into()),
            work_item_state: Some("Active".into()),
            repositories: vec!["front".into()],
        };

        let actions = workspace_actions_for_tui(&workspace);

        assert!(actions.iter().any(|action| matches!(
            &action.request,
            TuiActionRequest::TaskPreflight(args)
                if args.workspace.as_ref().map(dw_core::WorkspacePath::as_str) == Some("/tmp/ws")
        )));
        assert!(
            actions
                .iter()
                .any(|action| action.label.starts_with("Preview removal"))
        );
        assert!(
            actions
                .iter()
                .any(|action| action.label.starts_with("Remove workspace"))
        );
    }

    #[test]
    fn action_execution_mode_keeps_external_actions_attached() {
        let external = TuiAction {
            label: "Open".into(),
            request: TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                workspace: Some(dw_core::WorkspacePath::from("/tmp/ws")),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                pull_request: None,
                r#continue: false,
                repo: None,
                agent: None,
            }),
            description: "open".into(),
            kind: ActionRisk::OpensExternal,
        };
        let confirmed_destructive = TuiAction {
            label: "Prune".into(),
            request: TuiActionRequest::TaskPrune(dw_task::prune::PruneArgs {
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                selected_workspaces: None,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
                no_sync: false,
            }),
            description: "prune".into(),
            kind: ActionRisk::Destructive,
        };
        let unconfirmed_destructive = TuiAction {
            label: "Delete".into(),
            request: TuiActionRequest::SecretDelete {
                key: dw_core::SecretKey::from("KEY"),
            },
            description: "delete".into(),
            kind: ActionRisk::Destructive,
        };

        assert!(external.runs_attached_in_tui());
        assert!(!external.bypasses_cli_confirmation());
        assert!(!confirmed_destructive.runs_attached_in_tui());
        assert!(confirmed_destructive.bypasses_cli_confirmation());
        assert!(!unconfirmed_destructive.runs_attached_in_tui());
        assert!(!unconfirmed_destructive.bypasses_cli_confirmation());
    }

    #[test]
    fn refresh_after_success_is_subcommand_aware() {
        let mut action = workspace_action(&workspace_item(), WorkspaceAction::Sync);
        assert!(action.should_refresh_after_success());

        action = workspace_action(&workspace_item(), WorkspaceAction::Preflight);
        assert!(!action.should_refresh_after_success());

        action = workspace_action(&workspace_item(), WorkspaceAction::FinishExecute);
        assert!(action.should_refresh_after_success());

        action = workspace_action(&workspace_item(), WorkspaceAction::FinishPreview);
        assert!(!action.should_refresh_after_success());
    }

    #[test]
    fn actions_can_resolve_shared_core_descriptors() {
        let action = workspace_action(&workspace_item(), WorkspaceAction::FinishExecute);
        let descriptor = action.descriptor().expect("descriptor");

        assert_eq!(descriptor.id, "task.finish");
        assert_eq!(descriptor.domain, dw_core::ActionDomain::Task);
        assert!(descriptor.refresh_after_success);
    }

    #[test]
    fn snapshot_exposes_assigned_work_item_prompt_specs_for_tui_forms() {
        let mut snapshot = TuiSnapshot::loading(Some("/tmp/dw"));
        snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "ha - Hommage Agence".into(),
            items: vec![AdoAssignedItem {
                id: "55264".into(),
                kind: "Task".into(),
                state: "Actif".into(),
                title: "Transmission automatique".into(),
                url: None,
            }],
            error: None,
        }];

        let specs = snapshot.assigned_work_item_prompt_specs();

        assert_eq!(specs[0].id.as_str(), "assigned-work-item:ha");
        assert_eq!(specs[0].choices[0].value.as_str(), "55264");
        assert_eq!(
            specs[0].choices[0].label,
            "#55264 [Task] (Actif) Transmission automatique"
        );
        assert_eq!(
            specs[0].choices[1].value.as_str(),
            dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_VALUE
        );
        assert_eq!(
            specs[0].choices[1].label,
            dw_ado_commands::commands::assigned::MANUAL_WORK_ITEM_PROMPT_LABEL
        );
    }

    #[test]
    fn successful_effect_is_argument_based() {
        let color = TuiAction {
            label: "Color".into(),
            request: TuiActionRequest::ConfigSetColor {
                mode: dw_core::ConfigColorMode::Always,
            },
            description: String::new(),
            kind: ActionRisk::Safe,
        };
        assert_eq!(
            color.successful_effect(),
            Some(ActionEffect::ColorMode(dw_core::ConfigColorMode::Always))
        );
    }

    #[test]
    fn action_risk_confirmation_copy_is_explicit() {
        assert_eq!(
            ActionRisk::Destructive.confirmation_title(),
            "Destructive confirmation"
        );
        assert!(
            ActionRisk::Destructive
                .risk_label()
                .contains("Modifies or deletes")
        );
        assert_eq!(
            ActionRisk::OpensExternal.confirmation_title(),
            "Open external process"
        );
    }

    #[tokio::test]
    async fn concurrent_assigned_loader_keeps_config_order() {
        let choices = vec![
            dw_config::ProjectChoice {
                key: "front".into(),
                label: "Front".into(),
            },
            dw_config::ProjectChoice {
                key: "back".into(),
                label: "Back".into(),
            },
        ];

        let items = load_assigned_projects(
            choices,
            ProjectsConfig::default(),
            WorkflowConfig::default(),
            dw_ado::auth::AdoToken {
                access_token: "token".into(),
                source: "test".into(),
                scheme: dw_ado::auth::AdoAuthScheme::Bearer,
                expires_on: None,
            },
        )
        .await;

        assert_eq!(
            items
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            ["front", "back"]
        );
        assert!(items.iter().all(|item| item.error.is_some()));
    }

    #[test]
    fn pull_request_targets_keep_project_and_repository_order() {
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "ha": {
      "displayName": "HA",
      "azureDevOps": { "organization": "https://dev.azure.com/acme", "project": "HA" },
      "repositories": {
        "front": {
          "url": "",
          "defaultBranch": "develop",
          "azureDevOpsRepository": "HA Front"
        },
        "ignored": { "url": "", "defaultBranch": "develop" },
        "back": {
          "url": "",
          "defaultBranch": "develop",
          "azureDevOpsRepository": "HA Back"
        }
      }
    },
    "ops": {
      "displayName": "OPS",
      "azureDevOps": { "organization": "https://dev.azure.com/acme", "project": "OPS" },
      "repositories": {
        "tools": {
          "url": "",
          "defaultBranch": "main",
          "azureDevOpsRepository": "OPS Tools"
        }
      }
    }
  }
}"#,
        )
        .expect("projects config");
        let choices = project_choices(&projects);

        let targets = pull_request_targets(&projects, &WorkflowConfig::default(), choices);

        assert_eq!(
            targets
                .iter()
                .map(|target| format!("{}:{}", target.project, target.repository))
                .collect::<Vec<_>>(),
            ["ha:front", "ha:back", "ops:tools"]
        );
        assert_eq!(
            targets
                .iter()
                .map(|target| target.ado_repository.as_str())
                .collect::<Vec<_>>(),
            ["HA Front", "HA Back", "OPS Tools"]
        );
        assert!(targets.iter().all(|target| target.options.is_some()));
    }

    #[test]
    fn pull_request_targets_skip_aggregate_cross_projects() {
        let projects: ProjectsConfig = serde_json::from_str(
            r#"{
  "projects": {
    "ha": {
      "displayName": "HA",
      "azureDevOps": { "organization": "https://dev.azure.com/acme", "project": "HA" },
      "repositories": {
        "front": {
          "url": "",
          "defaultBranch": "develop",
          "azureDevOpsRepository": "HA Front"
        }
      }
    },
    "cross-ha-he": {
      "displayName": "HA + HE",
      "azureDevOps": { "organization": "https://dev.azure.com/acme", "project": "HA" },
      "repositories": {
        "front": {
          "url": "",
          "defaultBranch": "develop",
          "azureDevOpsRepository": "HA Front"
        },
        "he": {
          "url": "",
          "defaultBranch": "develop",
          "azureDevOpsRepository": "HE"
        }
      }
    }
  }
}"#,
        )
        .expect("projects config");
        let choices = project_choices(&projects);

        let targets = pull_request_targets(&projects, &WorkflowConfig::default(), choices);

        assert_eq!(
            targets
                .iter()
                .map(|target| format!("{}:{}", target.project, target.repository))
                .collect::<Vec<_>>(),
            ["ha:front"]
        );
    }

    #[test]
    fn pull_request_error_keeps_action_context() {
        let item = pull_request_error(
            PullRequestTarget {
                order: 0,
                project: "ha".into(),
                repository: "front".into(),
                ado_repository: "HA Front".into(),
                options: None,
            },
            "boom",
        );

        assert_eq!(item.project, ProjectKey::from("ha"));
        assert_eq!(item.repository, WorkspaceRepositoryName::from("front"));
        assert_eq!(item.ado_repository, AdoRepositoryName::from("HA Front"));
        assert_eq!(item.error.as_deref(), Some("boom"));
    }

    fn workspace_item() -> TaskListItem {
        TaskListItem {
            path: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            work_items: vec![dw_workspace::WorkspaceWorkItem {
                id: "42".into(),
                kind: Some("User Story".into()),
                title: Some("Demo".into()),
                state: Some("Active".into()),
            }],
            task_id: None,
            all_known_work_item_ids: vec!["42".into()],
            kind: "feature".into(),
            slug: "demo".into(),
            branch_name: "feature/42-demo".into(),
            created_at: "2026-07-04T00:00:00Z".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Demo".into()),
            work_item_state: Some("Active".into()),
            repositories: vec!["front".into()],
        }
    }
}
