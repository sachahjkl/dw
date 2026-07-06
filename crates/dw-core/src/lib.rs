use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

macro_rules! string_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

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
    pub id: PromptId,
    pub kind: PromptKind,
    pub label: String,
    pub help: Option<String>,
    pub required: bool,
    pub choices: Vec<PromptChoice>,
}

impl PromptSpec {
    pub fn select(
        id: impl Into<PromptId>,
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
        id: impl Into<PromptId>,
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

    pub fn text(id: impl Into<PromptId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: PromptKind::Text,
            label: label.into(),
            help: None,
            required: true,
            choices: Vec::new(),
        }
    }

    pub fn confirm(id: impl Into<PromptId>, label: impl Into<String>) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromptId(String);

impl PromptId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PromptId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for PromptId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for PromptId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
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
    pub value: PromptChoiceValue,
    pub label: String,
    pub description: Option<String>,
}

impl PromptChoice {
    pub fn new(value: impl Into<PromptChoiceValue>, label: impl Into<String>) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PromptChoiceValue(String);

impl PromptChoiceValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for PromptChoiceValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for PromptChoiceValue {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for PromptChoiceValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InputRequest {
    Confirm {
        id: PromptId,
        label: String,
        help: Option<String>,
        default: bool,
    },
    SelectOne {
        id: PromptId,
        label: String,
        help: Option<String>,
        choices: Vec<PromptChoice>,
    },
    SelectMany {
        id: PromptId,
        label: String,
        help: Option<String>,
        choices: Vec<PromptChoice>,
    },
    Text {
        id: PromptId,
        label: String,
        help: Option<String>,
        default: Option<String>,
    },
    Secret {
        id: PromptId,
        label: String,
        help: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InputResponse {
    Confirm { accepted: bool },
    SelectOne { value: PromptChoiceValue },
    SelectMany { values: Vec<PromptChoiceValue> },
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

    pub fn parse_many(input: &str) -> Vec<Self> {
        input
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(Self::from)
            .collect()
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
pub struct WorkItemTypeName(String);

impl WorkItemTypeName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkItemTypeName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkItemTypeName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkItemTypeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkItemTitle(String);

impl WorkItemTitle {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkItemTitle {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkItemTitle {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkItemTitle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkItemState(String);

impl WorkItemState {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, WorkItemStateParseError> {
        let value = value.into().trim().to_string();
        if value.is_empty() {
            return Err(WorkItemStateParseError);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkItemState {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkItemState {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkItemState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkItemStateParseError;

impl fmt::Display for WorkItemStateParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("état work item vide")
    }
}

impl std::error::Error for WorkItemStateParseError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkItemHistoryComment(String);

impl WorkItemHistoryComment {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for WorkItemHistoryComment {
    fn default() -> Self {
        Self("ado set-state".into())
    }
}

impl From<String> for WorkItemHistoryComment {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkItemHistoryComment {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkItemHistoryComment {
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

    pub fn parse_many(input: &str) -> Vec<Self> {
        input
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(Self::from)
            .collect()
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectKey(String);

impl ProjectKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ProjectKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SqlQuery(String);

impl SqlQuery {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SqlQuery {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SqlQuery {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for SqlQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DatabaseKey(String);

impl DatabaseKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DatabaseKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DatabaseKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DatabaseKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DatabaseEnvironmentName(String);

impl DatabaseEnvironmentName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DatabaseEnvironmentName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DatabaseEnvironmentName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DatabaseEnvironmentName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DatabaseTableName(String);

impl DatabaseTableName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DatabaseTableName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DatabaseTableName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DatabaseTableName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

string_newtype!(DatabaseConnectionString);
string_newtype!(CommitMessage);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspacePath(String);

impl WorkspacePath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkspacePath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkspacePath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkspacePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DevWorkflowRoot(String);

impl DevWorkflowRoot {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for DevWorkflowRoot {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for DevWorkflowRoot {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for DevWorkflowRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigRootPath(String);

impl ConfigRootPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ConfigRootPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ConfigRootPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ConfigRootPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigColorMode {
    Auto,
    Always,
    Never,
}

impl ConfigColorMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Always, Self::Never];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Always => "always",
            Self::Never => "never",
        }
    }
}

impl FromStr for ConfigColorMode {
    type Err = ConfigColorModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.as_str().eq_ignore_ascii_case(value.trim()))
            .ok_or_else(|| ConfigColorModeParseError {
                value: value.into(),
            })
    }
}

impl fmt::Display for ConfigColorMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigColorModeParseError {
    value: String,
}

impl fmt::Display for ConfigColorModeParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "mode couleur inconnu: {}", self.value)
    }
}

impl std::error::Error for ConfigColorModeParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigWriteField {
    Root,
    Color,
}

impl ConfigWriteField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Color => "color",
        }
    }
}

impl fmt::Display for ConfigWriteField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceRepositoryName(String);

impl WorkspaceRepositoryName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for WorkspaceRepositoryName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for WorkspaceRepositoryName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for WorkspaceRepositoryName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    Opencode,
    Cursor,
    Claude,
    Codex,
    CodexCli,
    Copilot,
}

impl Agent {
    pub const ALL: [Self; 6] = [
        Self::Opencode,
        Self::Cursor,
        Self::Claude,
        Self::Codex,
        Self::CodexCli,
        Self::Copilot,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::Cursor => "cursor",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::CodexCli => "codex-cli",
            Self::Copilot => "copilot",
        }
    }
}

impl FromStr for Agent {
    type Err = AgentParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|agent| agent.as_str().eq_ignore_ascii_case(value.trim()))
            .ok_or_else(|| AgentParseError {
                value: value.into(),
            })
    }
}

impl fmt::Display for Agent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentParseError {
    value: String,
}

impl fmt::Display for AgentParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "agent inconnu: {}", self.value)
    }
}

impl std::error::Error for AgentParseError {}

string_newtype!(UpgradeOwner);
string_newtype!(UpgradeRepositoryName);
string_newtype!(UpgradeAssetName);
string_newtype!(RuntimeIdentifier);
string_newtype!(UpgradeFileName);
string_newtype!(Sha256Digest);
string_newtype!(ExecutablePath);
string_newtype!(SemanticVersion);
string_newtype!(UpgradeReleaseTag);
string_newtype!(GitCommitSha);
string_newtype!(AgentExecutableName);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SecretKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SecretValue {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretValue {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue(***)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnvironmentVariableName(String);

impl EnvironmentVariableName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for EnvironmentVariableName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for EnvironmentVariableName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for EnvironmentVariableName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepositoryPath(String);

impl RepositoryPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RepositoryPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for RepositoryPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for RepositoryPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRootPath(String);

impl ProjectRootPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProjectRootPath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for ProjectRootPath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ProjectRootPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BranchName(String);

impl BranchName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for BranchName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for BranchName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GitAnchorName(String);

impl GitAnchorName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for GitAnchorName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for GitAnchorName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for GitAnchorName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TaskId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for TaskId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskSlug(String);

impl TaskSlug {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for TaskSlug {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for TaskSlug {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for TaskSlug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AiContextFilePath(String);

impl AiContextFilePath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for AiContextFilePath {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for AiContextFilePath {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for AiContextFilePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

string_newtype!(HandoffFilePath);
string_newtype!(HandoffParseError);
string_newtype!(WorkspaceOperationError);
string_newtype!(SecretStoreErrorMessage);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitOperation {
    CommitAndPush,
    Push,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TaskActionEvent {
    ResolvingPullRequestWorkItems {
        pull_request_id: PullRequestId,
    },
    ResolvedPullRequestWorkItems {
        work_item_ids: Vec<WorkItemId>,
    },
    VerifyingFinish {
        pull_request_candidate_count: usize,
    },
    FinishVerificationCompleted,
    RunningGitOperation {
        operation: GitOperation,
        repository_count: usize,
    },
    RunningRepositoryGitOperation {
        repository: WorkspaceRepositoryName,
        operation: GitOperation,
    },
    GitOperationCompleted {
        operation: GitOperation,
    },
    SkippingPullRequestCreation,
    AuthenticatingAdoForPullRequests {
        pull_request_candidate_count: usize,
    },
    CheckingActivePullRequest {
        repository: WorkspaceRepositoryName,
    },
    CreatingPullRequest {
        repository: WorkspaceRepositoryName,
    },
    PullRequestWorkItemLinkSkipped {
        work_item_id: WorkItemId,
        error: String,
    },
    UpdatingFinishWorkItemStates {
        work_item_ids: Vec<WorkItemId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AdoActionEvent {
    Authenticating {
        project: Option<ProjectKey>,
    },
    LoadingAssignedWorkItems {
        project: ProjectKey,
        top: i32,
    },
    GroupingAssignedWorkItems {
        project: ProjectKey,
    },
    LoadingPullRequests {
        project: ProjectKey,
    },
    ResolvingPullRequestWorkItems {
        repositories: Vec<AdoRepositoryName>,
    },
    ExtractingGitWorkItems {
        git_to: Option<String>,
    },
    LoadingWorkItem {
        id: WorkItemId,
    },
    LoadingWorkItems {
        ids: Vec<WorkItemId>,
    },
    LoadingWorkItemContext {
        id: WorkItemId,
    },
    LoadingChangelog {
        ids: Vec<WorkItemId>,
    },
    LoadingChangelogItems {
        ids: Vec<WorkItemId>,
    },
    UpdatingWorkItemState {
        ids: Vec<WorkItemId>,
        state: WorkItemState,
    },
    UpdatedWorkItemState {
        id: WorkItemId,
        state: WorkItemState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ConfigActionEvent {
    Reading { root: Option<DevWorkflowRoot> },
    Writing { field: ConfigWriteField },
    Validating { root: Option<DevWorkflowRoot> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AgentActionEvent {
    Checking { agent: Option<Agent> },
    ResolvingDefault { root: DevWorkflowRoot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DbActionEvent {
    GuardingQuery,
    ResolvingConnection { database: Option<DatabaseKey> },
    ExecutingReadOnlyQuery { max_rows: Option<usize> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SecretActionEvent {
    Reading { key: SecretKey },
    Writing { key: SecretKey },
    Deleting { key: SecretKey },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpgradeActionEvent {
    CheckingHost,
    ResolvingConfig,
    FetchingRelease {
        owner: UpgradeOwner,
        repository: UpgradeRepositoryName,
    },
    FetchingManifest {
        asset_name: UpgradeAssetName,
    },
    SelectingAsset {
        rid: RuntimeIdentifier,
    },
    DownloadingAsset {
        file_name: UpgradeFileName,
    },
    VerifyingChecksum {
        file_name: UpgradeFileName,
        expected_sha256: Sha256Digest,
    },
    PreparingExecutable {
        file_name: UpgradeFileName,
        rid: RuntimeIdentifier,
    },
    ReplacingExecutable {
        executable_path: ExecutablePath,
    },
    Completed {
        version: SemanticVersion,
    },
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
