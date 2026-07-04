use dw_config::{
    ConfigDoctorReport, ConfigShow, DatabasesConfig, ProjectsConfig, WorkflowConfig, config_doctor,
    load_databases_config, load_projects_config, load_user_settings, load_workflow_config,
    project_choices, repository_config, resolve_project, resolve_root,
};
use dw_workspace::{TaskListItem, plan_task_prune, task_list};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Workspaces,
    Ado,
    PullRequests,
    Db,
    Config,
    Composer,
    Help,
}

impl View {
    pub const ALL: [View; 8] = [
        View::Dashboard,
        View::Workspaces,
        View::Ado,
        View::PullRequests,
        View::Db,
        View::Config,
        View::Composer,
        View::Help,
    ];

    pub fn label(self) -> &'static str {
        match self {
            View::Dashboard => "Dashboard",
            View::Workspaces => "Workspaces",
            View::Ado => "ADO",
            View::PullRequests => "PRs",
            View::Db => "DB",
            View::Config => "Config",
            View::Composer => "Composer",
            View::Help => "Aide",
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
            ActionRisk::OpensExternal => "Ouvrir une processus externe",
            ActionRisk::DryRun => "Confirmation preview",
            ActionRisk::Destructive => "Confirmation destructive",
        }
    }

    pub fn risk_label(self) -> &'static str {
        match self {
            ActionRisk::Safe => "Lecture/inspection",
            ActionRisk::OpensExternal => "Ouvre un outil ou un flux interactif",
            ActionRisk::DryRun => "Prévisualisation sans modification attendue",
            ActionRisk::Destructive => "Modifie ou supprime des données/workspaces",
        }
    }

    pub fn confirmation_hint(self) -> &'static str {
        match self {
            ActionRisk::Safe => "Entrée/y: lancer    Esc/n: annuler",
            ActionRisk::OpensExternal => "Entrée/y: ouvrir    Esc/n: annuler",
            ActionRisk::DryRun => "Entrée/y: prévisualiser    Esc/n: annuler",
            ActionRisk::Destructive => "Entrée/y: confirmer l'action destructive    Esc/n: annuler",
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
    ColorMode(String),
    DefaultAgent(String),
    Root(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TuiActionRequest {
    Version,
    Doctor,
    Guide,
    Refresh(dw_config::command::RefreshCommandArgs),
    ConfigShow { root: Option<String> },
    ConfigDoctor { root: Option<String> },
    ConfigSetColor { mode: String },
    ConfigSetRoot { path: String },
    AgentConfig { root: Option<String> },
    AgentSetDefault { root: Option<String>, agent: String },
    AgentDoctor { agent: Option<String> },
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
    SecretGet { key: String },
    SecretSetFromEnv { key: String, env: String },
    SecretDelete { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailPanel {
    pub content: DetailPanelContent,
    pub scroll: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailPanelContent {
    Guide(Vec<String>),
    ConfigShow(ConfigShow),
    ConfigDoctor(ConfigDoctorReport),
    AgentDoctor(dw_agent::command::AgentDoctorReport),
    OperationResult { title: String, lines: Vec<String> },
}

impl DetailPanel {
    pub fn guide(lines: Vec<String>) -> Self {
        Self {
            content: DetailPanelContent::Guide(lines),
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

    pub fn operation_result(title: impl Into<String>, output: &str) -> Self {
        let mut lines = output
            .lines()
            .map(|line| line.to_owned())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push("Aucun détail retourné.".into());
        }
        Self {
            content: DetailPanelContent::OperationResult {
                title: title.into(),
                lines,
            },
            scroll: 0,
        }
    }

    pub fn title(&self) -> String {
        match &self.content {
            DetailPanelContent::Guide(_) => "Guide DevWorkflow".into(),
            DetailPanelContent::ConfigShow(_) => "Configuration effective".into(),
            DetailPanelContent::ConfigDoctor(_) => "Diagnostic configuration".into(),
            DetailPanelContent::AgentDoctor(_) => "Diagnostic agents".into(),
            DetailPanelContent::OperationResult { title, .. } => title.clone(),
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
            DetailPanelContent::Guide(lines) => lines.len(),
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
            DetailPanelContent::OperationResult { lines, .. } => lines.len(),
        }
    }
}

impl TuiAction {
    pub fn with_root(mut self, root: String) -> Self {
        match &mut self.request {
            TuiActionRequest::TaskPreflight(args) => args.root = Some(root),
            TuiActionRequest::TaskSync(args) => args.root = Some(root),
            TuiActionRequest::TaskRepoLatest(args) => args.root = Some(root),
            TuiActionRequest::TaskHandoffValidate(args) => args.root = Some(root),
            TuiActionRequest::TaskCommit(args) => args.root = Some(root),
            TuiActionRequest::TaskFinish(args) => args.root = Some(root),
            TuiActionRequest::TaskTeardown(args) => args.root = Some(root),
            TuiActionRequest::AgentOpen(args) => args.root = Some(root),
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
            | TuiActionRequest::ConfigSetRoot { .. }
            | TuiActionRequest::ConfigSetColor { .. }
            | TuiActionRequest::AgentSetDefault { .. }
            | TuiActionRequest::AdoSetState(_)
            | TuiActionRequest::TaskSync(_)
            | TuiActionRequest::TaskRepoLatest(_)
            | TuiActionRequest::TaskCreateChildTask(_) => true,
            TuiActionRequest::TaskStart(args) => args.mode.executes(),
            TuiActionRequest::TaskStartPr(args) => args.mode.executes(),
            TuiActionRequest::TaskRename(args) => args.mode.executes(),
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
            TuiActionRequest::ConfigSetColor { mode } => {
                Some(ActionEffect::ColorMode(mode.clone()))
            }
            TuiActionRequest::AgentSetDefault { agent, .. } => {
                Some(ActionEffect::DefaultAgent(agent.clone()))
            }
            TuiActionRequest::ConfigSetRoot { path } => Some(ActionEffect::Root(path.clone())),
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

    pub fn is_config_action(&self) -> bool {
        matches!(
            self.request,
            TuiActionRequest::Doctor
                | TuiActionRequest::Guide
                | TuiActionRequest::Refresh(_)
                | TuiActionRequest::ConfigShow { .. }
                | TuiActionRequest::ConfigDoctor { .. }
                | TuiActionRequest::ConfigSetColor { .. }
                | TuiActionRequest::ConfigSetRoot { .. }
                | TuiActionRequest::AgentConfig { .. }
                | TuiActionRequest::AgentSetDefault { .. }
                | TuiActionRequest::AgentDoctor { .. }
                | TuiActionRequest::SecretGet { .. }
                | TuiActionRequest::SecretSetFromEnv { .. }
                | TuiActionRequest::SecretDelete { .. }
        )
    }

    pub fn workspace_path(&self) -> Option<&str> {
        match &self.request {
            TuiActionRequest::AgentOpen(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskPreflight(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskHandoffValidate(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskSync(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskRename(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskRepoLatest(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskCommit(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskAddRepo(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskTeardown(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskFinish(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskAddWorkItem(args) => args.workspace.as_deref(),
            TuiActionRequest::TaskRemoveWorkItem(args) => args.workspace.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TuiSnapshot {
    pub root: String,
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
    pub color_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiDatabase {
    pub project: Option<String>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoAssignedProject {
    pub key: String,
    pub label: String,
    pub items: Vec<AdoAssignedItem>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoAssignedItem {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub title: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiPullRequest {
    pub workspace: Option<String>,
    pub project: String,
    pub repository: String,
    pub ado_repository: String,
    pub branch: String,
    pub target_branch: String,
    pub pull_request_id: Option<i64>,
    pub title: Option<String>,
    pub is_draft: bool,
    pub work_item_ids: Vec<String>,
    pub url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct PullRequestTarget {
    order: usize,
    project: String,
    repository: String,
    ado_repository: String,
    options: Option<dw_ado::AzureDevOpsOptions>,
}

impl TuiSnapshot {
    pub fn loading(root: Option<&str>) -> Self {
        let root = resolve_root(root);
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
        let color_mode = load_user_settings()
            .color
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "auto".into());
        Self {
            root,
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
        let projects = load_projects_config(&root);
        let workflow = load_workflow_config(&root);
        let databases = load_databases_config(&root);
        let database_entries = database_entries_for_tui(&databases);
        let config_doctor = config_doctor(Some(&root));
        let workspaces = task_list(&root, None, None);
        let prune_candidates = plan_task_prune(&root, None, None).len();
        let actions = build_actions(&root, &projects, &databases, &workspaces);
        let color_mode = load_user_settings()
            .color
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "auto".into());
        Self {
            root,
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

    pub fn default_agent(&self) -> String {
        self.workflow
            .agent
            .as_ref()
            .map(|agent| agent.default.clone())
            .filter(|agent| !agent.trim().is_empty())
            .unwrap_or_else(|| "opencode".into())
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
                            item.id.clone(),
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
                .with_help("Choisir un work item assigné hors états finaux")
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
        if item.state.is_empty() {
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
                    key: choice.key,
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
                    label: "Projet ADO".into(),
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
    match dw_ado_commands::resolve_options(&projects, &workflow, &choice.key) {
        Ok(options) => match dw_ado::query_assigned_work_items(&options, 50, &token).await {
            Ok(items) => AdoAssignedProject {
                key: choice.key,
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
                key: choice.key,
                label: choice.label,
                items: Vec::new(),
                error: Some(error.to_string()),
            },
        },
        Err(error) => AdoAssignedProject {
            key: choice.key,
            label: choice.label,
            items: Vec::new(),
            error: Some(error.to_string()),
        },
    }
}

impl From<dw_ado::WorkItemSnapshot> for AdoAssignedItem {
    fn from(value: dw_ado::WorkItemSnapshot) -> Self {
        Self {
            id: value.id,
            kind: value.kind.unwrap_or_else(|| "-".into()),
            state: value.state.unwrap_or_else(|| "-".into()),
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
                    project: choice.key,
                    repository: "-".into(),
                    ado_repository: "-".into(),
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
        let project_config = resolve_project(projects, &choice.key);
        let options = dw_ado_commands::resolve_options(projects, workflow, &choice.key).ok();
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
                project: choice.key.clone(),
                repository: repository.clone(),
                ado_repository,
                options: options.clone(),
            });
        }
    }
    targets
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
                    project: "-".into(),
                    repository: "-".into(),
                    ado_repository: "-".into(),
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

    match dw_ado::list_active_pull_requests_authenticated(options, &target.ado_repository, token) {
        Ok(prs) => prs
            .into_iter()
            .map(|pr| {
                let branch = trim_branch(pr.source_ref_name.as_deref()).to_string();
                let workspace = workspaces
                    .iter()
                    .find(|workspace| {
                        workspace.project == target.project
                            && workspace.branch_name.eq_ignore_ascii_case(&branch)
                            && workspace
                                .repositories
                                .iter()
                                .any(|item| item == &target.repository)
                    })
                    .map(|workspace| workspace.path.clone());
                TuiPullRequest {
                    workspace,
                    project: target.project.clone(),
                    repository: target.repository.clone(),
                    ado_repository: pr.repository,
                    branch,
                    target_branch: trim_branch(pr.target_ref_name.as_deref()).to_string(),
                    pull_request_id: Some(pr.pull_request_id),
                    title: pr.title,
                    is_draft: pr.is_draft,
                    work_item_ids: pr.work_item_ids,
                    url: Some(dw_ado::pull_request_web_url(
                        options,
                        &target.ado_repository,
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
            label: "Guide".into(),
            request: TuiActionRequest::Guide,
            description: "Afficher le parcours de démarrage".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Doctor".into(),
            request: TuiActionRequest::Doctor,
            description: "Diagnostiquer machine et configuration".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Voir configuration".into(),
            request: TuiActionRequest::ConfigShow {
                root: Some(root.into()),
            },
            description: "Afficher les chemins de configuration".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Diagnostiquer configuration".into(),
            request: TuiActionRequest::ConfigDoctor {
                root: Some(root.into()),
            },
            description: "Valider les fichiers de configuration".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Refresh".into(),
            request: TuiActionRequest::Refresh(dw_config::command::RefreshCommandArgs {
                root: Some(root.into()),
                profile: "business".into(),
            }),
            description: "Régénérer schémas et contextes agents".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Mes work items".into(),
            request: TuiActionRequest::AdoAssigned(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(root.into()),
                    project: None,
                    top: 20,
                    all: false,
                    group_by_parent: false,
                },
            ),
            description: "Lister les work items assignés".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Mes work items groupés".into(),
            request: TuiActionRequest::AdoAssigned(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(root.into()),
                    project: None,
                    top: 20,
                    all: false,
                    group_by_parent: true,
                },
            ),
            description: "Lister les work items assignés groupés".into(),
            kind: ActionRisk::Safe,
        },
        TuiAction {
            label: "Tester SQL read-only".into(),
            request: TuiActionRequest::DbGuard(dw_db::commands::GuardArgs {
                sql: "select 1".into(),
            }),
            description: "Tester la garde SQL read-only".into(),
            kind: ActionRisk::Safe,
        },
    ];

    for project in project_choices(projects) {
        actions.push(TuiAction {
            label: format!("Work items · {}", project.key),
            request: TuiActionRequest::AdoAssigned(
                dw_ado_commands::commands::assigned::AssignedArgs {
                    root: Some(root.into()),
                    project: Some(project.key.clone()),
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
    let label_context = format!("{} {}", workspace.project, workspace.display_work_items);
    match action {
        WorkspaceAction::Open => TuiAction {
            label: format!("Ouvrir · {label_context}"),
            request: TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item: None,
                positional_work_item: None,
                r#continue: false,
                repo: None,
                agent: None,
            }),
            description: "Ouvrir le workspace avec l'agent configuré".into(),
            kind: ActionRisk::OpensExternal,
        },
        WorkspaceAction::Preflight => TuiAction {
            label: format!("Vérifier · {label_context}"),
            request: TuiActionRequest::TaskPreflight(dw_task::validate::PreflightArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item: None,
                r#continue: false,
                ai_context_file: Vec::new(),
                positional_work_item: None,
            }),
            description: workspace.path.clone(),
            kind: ActionRisk::Safe,
        },
        WorkspaceAction::Sync => TuiAction {
            label: format!("Synchroniser · {label_context}"),
            request: TuiActionRequest::TaskSync(dw_task::lifecycle::SyncArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item: None,
                r#continue: false,
                positional_work_item: None,
            }),
            description: "Rafraîchir task.json depuis Azure DevOps".into(),
            kind: ActionRisk::Safe,
        },
        WorkspaceAction::RepoLatest => TuiAction {
            label: format!("Mettre à jour les repos · {label_context}"),
            request: TuiActionRequest::TaskRepoLatest(dw_task::repo::RepoLatestArgs {
                workspace: Some(workspace_arg),
                r#continue: false,
                only: None,
                root: None,
            }),
            description: "Mettre les repositories à jour depuis leur branche cible".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::HandoffValidate => TuiAction {
            label: format!("Valider handoff · {label_context}"),
            request: TuiActionRequest::TaskHandoffValidate(
                dw_task::validate::HandoffValidateArgs {
                    workspace: Some(workspace_arg),
                    root: None,
                    project: None,
                    work_item: None,
                    r#continue: false,
                    positional_work_item: None,
                },
            ),
            description: "Valider les handoffs".into(),
            kind: ActionRisk::Safe,
        },
        WorkspaceAction::CommitPreview => TuiAction {
            label: format!("Prévisualiser commit · {label_context}"),
            request: TuiActionRequest::TaskCommit(dw_task::repo::CommitArgs {
                workspace: Some(workspace_arg),
                r#continue: false,
                root: None,
                mode: dw_core::ExecutionMode::Preview,
                message: None,
            }),
            description: "Prévisualiser les commits".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::FinishPreview => TuiAction {
            label: format!("Prévisualiser finalisation · {label_context}"),
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
            description: "Prévisualiser finish".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::FinishExecute => TuiAction {
            label: format!("Finaliser workspace · {label_context}"),
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
            description: "Terminer le workspace: commit/push/PR/ADO selon options".into(),
            kind: ActionRisk::Destructive,
        },
        WorkspaceAction::TeardownPreview => TuiAction {
            label: format!("Prévisualiser suppression · {label_context}"),
            request: TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                workspace: Some(workspace_arg.clone()),
                root: None,
                project: None,
                work_item: None,
                r#continue: false,
                positional_work_item: None,
                mode: dw_core::ExecutionMode::Preview,
                yes: false,
            }),
            description: "Prévisualiser suppression workspace".into(),
            kind: ActionRisk::DryRun,
        },
        WorkspaceAction::TeardownExecute => TuiAction {
            label: format!("Supprimer workspace · {label_context}"),
            request: TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                workspace: Some(workspace_arg),
                root: None,
                project: None,
                work_item: None,
                r#continue: false,
                positional_work_item: None,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
            }),
            description: "Supprimer les worktrees et le workspace".into(),
            kind: ActionRisk::Destructive,
        },
    }
}

fn database_actions(databases: &DatabasesConfig) -> Vec<TuiAction> {
    let mut actions = Vec::new();
    for key in databases.globals.keys() {
        actions.push(TuiAction {
            label: format!("Explorer schéma · {key}"),
            request: TuiActionRequest::DbSchema(dw_db::commands::SchemaArgs {
                project: None,
                database: Some(key.clone()),
                env: None,
            }),
            description: "Base globale".into(),
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
                label: format!("Explorer schéma · {project}/{key}"),
                request: TuiActionRequest::DbSchema(dw_db::commands::SchemaArgs {
                    project: Some(project.clone()),
                    database: Some(key.clone()),
                    env: None,
                }),
                description: "Base projet".into(),
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

        assert!(labels.iter().any(|label| label.starts_with("Vérifier")));
        assert!(
            labels
                .iter()
                .any(|label| label.starts_with("Mes work items"))
        );
        assert!(
            labels.iter().any(
                |label| label.starts_with("Tester SQL") || label.starts_with("Explorer schéma")
            )
        );
        assert!(labels.iter().any(|label| {
            label.starts_with("Voir configuration")
                || label.starts_with("Diagnostiquer configuration")
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
            .find(|action| action.label == "Explorer schéma · shared")
            .expect("db schema action");

        assert!(matches!(
            db_schema.request,
            TuiActionRequest::DbSchema(ref args)
                if args.project.is_none() && args.database.as_deref() == Some("shared")
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
            label: "Créer workspace · ha #42".into(),
            request: TuiActionRequest::TaskStart(dw_task::start::StartArgs {
                work_item_id: Some("42".into()),
                root: Some("/tmp/dw".into()),
                project: Some("ha".into()),
                task: None,
                type_name: None,
                only: None,
                slug: None,
                skip_ado: true,
                with_active_children: false,
                create_child_tasks: false,
                mode: dw_core::ExecutionMode::Preview,
            }),
            description: "Préparer le workspace local".into(),
            kind: ActionRisk::DryRun,
        };

        assert_eq!(action.action_kind(), ActionKind::TaskStart);
        assert_eq!(action.display_label(), "Créer workspace · ha #42");
    }

    #[test]
    fn workspace_actions_are_contextual_and_non_destructive_by_default() {
        let workspace = TaskListItem {
            path: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            display_work_items: "#42 Demo".into(),
            task_id: None,
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
                if args.workspace.as_deref() == Some("/tmp/ws")
        )));
        assert!(
            actions
                .iter()
                .any(|action| action.label.starts_with("Prévisualiser suppression"))
        );
        assert!(
            actions
                .iter()
                .any(|action| action.label.starts_with("Supprimer workspace"))
        );
    }

    #[test]
    fn action_execution_mode_keeps_external_actions_attached() {
        let external = TuiAction {
            label: "Open".into(),
            request: TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                workspace: Some("/tmp/ws".into()),
                root: None,
                project: None,
                work_item: None,
                positional_work_item: None,
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
                work_item: None,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
                no_sync: false,
            }),
            description: "prune".into(),
            kind: ActionRisk::Destructive,
        };
        let unconfirmed_destructive = TuiAction {
            label: "Delete".into(),
            request: TuiActionRequest::SecretDelete { key: "KEY".into() },
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

        assert_eq!(specs[0].id, "assigned-work-item:ha");
        assert_eq!(specs[0].choices[0].value, "55264");
        assert_eq!(
            specs[0].choices[0].label,
            "#55264 [Task] (Actif) Transmission automatique"
        );
        assert_eq!(
            specs[0].choices[1].value,
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
                mode: "always".into(),
            },
            description: String::new(),
            kind: ActionRisk::Safe,
        };
        assert_eq!(
            color.successful_effect(),
            Some(ActionEffect::ColorMode("always".into()))
        );
    }

    #[test]
    fn action_risk_confirmation_copy_is_explicit() {
        assert_eq!(
            ActionRisk::Destructive.confirmation_title(),
            "Confirmation destructive"
        );
        assert!(
            ActionRisk::Destructive
                .risk_label()
                .contains("Modifie ou supprime")
        );
        assert!(
            ActionRisk::OpensExternal
                .confirmation_hint()
                .contains("ouvrir")
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

        assert_eq!(item.project, "ha");
        assert_eq!(item.repository, "front");
        assert_eq!(item.ado_repository, "HA Front");
        assert_eq!(item.error.as_deref(), Some("boom"));
    }

    fn workspace_item() -> TaskListItem {
        TaskListItem {
            path: "/tmp/ws".into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            display_work_items: "#42 Demo".into(),
            task_id: None,
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
