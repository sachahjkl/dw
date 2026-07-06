use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreContext {
    pub root: String,
    pub config_dir: Option<String>,
    pub environment: BTreeMap<String, String>,
}

impl CoreContext {
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            config_dir: None,
            environment: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionRisk {
    ReadOnly,
    Preview,
    Mutating,
    Destructive,
    ExternalLaunch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    Preview,
    Execute,
}

impl ExecutionMode {
    pub fn from_execute(execute: bool) -> Self {
        if execute {
            Self::Execute
        } else {
            Self::Preview
        }
    }

    pub fn executes(self) -> bool {
        self == Self::Execute
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionDomain {
    Config,
    Agent,
    Auth,
    Ado,
    Db,
    Task,
    Secret,
    Upgrade,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionDescriptor {
    pub id: &'static str,
    pub domain: ActionDomain,
    pub label: &'static str,
    pub description: &'static str,
    pub risk: ActionRisk,
    pub refresh_after_success: bool,
}

pub const ACTION_CATALOG: &[ActionDescriptor] = &[
    ActionDescriptor {
        id: "config.show",
        domain: ActionDomain::Config,
        label: "Config show",
        description: "Afficher chemins et réglages effectifs",
        risk: ActionRisk::ReadOnly,
        refresh_after_success: false,
    },
    ActionDescriptor {
        id: "config.doctor",
        domain: ActionDomain::Config,
        label: "Config doctor",
        description: "Valider les fichiers de configuration",
        risk: ActionRisk::ReadOnly,
        refresh_after_success: false,
    },
    ActionDescriptor {
        id: "ado.assigned",
        domain: ActionDomain::Ado,
        label: "ADO assigned",
        description: "Lister les work items assignés",
        risk: ActionRisk::ReadOnly,
        refresh_after_success: false,
    },
    ActionDescriptor {
        id: "ado.set-state",
        domain: ActionDomain::Ado,
        label: "ADO set-state",
        description: "Changer l'état de work items Azure DevOps",
        risk: ActionRisk::Mutating,
        refresh_after_success: true,
    },
    ActionDescriptor {
        id: "db.query",
        domain: ActionDomain::Db,
        label: "DB query",
        description: "Exécuter une requête SQL read-only",
        risk: ActionRisk::ReadOnly,
        refresh_after_success: false,
    },
    ActionDescriptor {
        id: "task.start",
        domain: ActionDomain::Task,
        label: "Task start",
        description: "Créer ou prévisualiser un workspace task",
        risk: ActionRisk::Mutating,
        refresh_after_success: true,
    },
    ActionDescriptor {
        id: "task.finish",
        domain: ActionDomain::Task,
        label: "Task finish",
        description: "Terminer un workspace task",
        risk: ActionRisk::Mutating,
        refresh_after_success: true,
    },
    ActionDescriptor {
        id: "task.teardown",
        domain: ActionDomain::Task,
        label: "Task teardown",
        description: "Supprimer un workspace task",
        risk: ActionRisk::Destructive,
        refresh_after_success: true,
    },
    ActionDescriptor {
        id: "agent.open",
        domain: ActionDomain::Agent,
        label: "Agent open",
        description: "Ouvrir un workspace avec un agent externe",
        risk: ActionRisk::ExternalLaunch,
        refresh_after_success: false,
    },
];

pub fn action_descriptor(id: &str) -> Option<&'static ActionDescriptor> {
    ACTION_CATALOG.iter().find(|descriptor| descriptor.id == id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSpec {
    pub id: String,
    pub kind: PromptKind,
    pub label: String,
    pub help: Option<String>,
    pub required: bool,
    pub choices: Vec<PromptChoice>,
}

impl PromptSpec {
    pub fn select(
        id: impl Into<String>,
        label: impl Into<String>,
        choices: Vec<PromptChoice>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: PromptKind::Select,
            label: label.into(),
            help: None,
            required: true,
            choices,
        }
    }

    pub fn multiselect(
        id: impl Into<String>,
        label: impl Into<String>,
        choices: Vec<PromptChoice>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: PromptKind::MultiSelect,
            label: label.into(),
            help: None,
            required: true,
            choices,
        }
    }

    pub fn text(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: PromptKind::Text,
            label: label.into(),
            help: None,
            required: true,
            choices: Vec::new(),
        }
    }

    pub fn confirm(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: PromptKind::Confirm,
            label: label.into(),
            help: None,
            required: true,
            choices: Vec::new(),
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptKind {
    Text,
    Select,
    MultiSelect,
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptChoice {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

impl PromptChoice {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InputRequest {
    Confirm {
        id: String,
        label: String,
        help: Option<String>,
        default: bool,
    },
    SelectOne {
        id: String,
        label: String,
        help: Option<String>,
        choices: Vec<PromptChoice>,
    },
    SelectMany {
        id: String,
        label: String,
        help: Option<String>,
        choices: Vec<PromptChoice>,
    },
    Text {
        id: String,
        label: String,
        help: Option<String>,
        default: Option<String>,
    },
    Secret {
        id: String,
        label: String,
        help: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InputResponse {
    Confirm { accepted: bool },
    SelectOne { value: String },
    SelectMany { values: Vec<String> },
    Text { value: String },
    Secret { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DwActionEvent {
    Started { action_id: String },
    Task(TaskActionEvent),
    Ado(AdoActionEvent),
    Config(ConfigActionEvent),
    Agent(AgentActionEvent),
    Db(DbActionEvent),
    Secret(SecretActionEvent),
    Upgrade(UpgradeActionEvent),
    NeedsInput { request: InputRequest },
    ExternalLaunch { plan: ExternalLaunchPlan },
    Completed { summary: ActionSummary },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkItemId(String);

impl WorkItemId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkItemId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkItemId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PullRequestId(String);

impl PullRequestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PullRequestId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for PullRequestId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for PullRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AdoRepositoryName(String);

impl AdoRepositoryName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AdoRepositoryName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for AdoRepositoryName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for AdoRepositoryName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TaskActionEvent {
    ResolvingPullRequestWorkItems { pull_request_id: PullRequestId },
    ResolvedPullRequestWorkItems { work_item_ids: Vec<WorkItemId> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AdoActionEvent {
    Authenticating {
        project: Option<String>,
    },
    LoadingAssignedWorkItems {
        project: String,
        top: i32,
    },
    GroupingAssignedWorkItems {
        project: String,
    },
    LoadingPullRequests {
        project: String,
    },
    ResolvingPullRequestWorkItems {
        repositories: Vec<AdoRepositoryName>,
    },
    ExtractingGitWorkItems {
        git_to: Option<String>,
    },
    LoadingWorkItem {
        id: String,
    },
    LoadingWorkItems {
        ids: Vec<WorkItemId>,
    },
    LoadingWorkItemContext {
        id: String,
    },
    LoadingChangelog {
        ids: Vec<WorkItemId>,
    },
    LoadingChangelogItems {
        ids: Vec<WorkItemId>,
    },
    UpdatingWorkItemState {
        ids: Vec<WorkItemId>,
        state: String,
    },
    UpdatedWorkItemState {
        id: WorkItemId,
        state: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ConfigActionEvent {
    Reading { root: Option<String> },
    Writing { field: String },
    Validating { root: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AgentActionEvent {
    Checking { agent: Option<String> },
    ResolvingDefault { root: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DbActionEvent {
    GuardingQuery,
    ResolvingConnection { database: Option<String> },
    ExecutingReadOnlyQuery { max_rows: Option<usize> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SecretActionEvent {
    Reading { key: String },
    Writing { key: String },
    Deleting { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpgradeActionEvent {
    CheckingHost,
    ResolvingConfig,
    FetchingRelease { owner: String, repository: String },
    FetchingManifest { asset_name: String },
    SelectingAsset { rid: String },
    DownloadingAsset { file_name: String },
    VerifyingChecksum { file_name: String },
    PreparingExecutable { file_name: String },
    ReplacingExecutable { executable_path: String },
    Completed { version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionSummary {
    pub title: String,
    pub status: String,
    pub risk: ActionRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaunchPlan {
    pub program: String,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub working_directory: Option<String>,
}

impl ExternalLaunchPlan {
    pub fn display_command(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.arguments.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
