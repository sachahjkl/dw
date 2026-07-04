use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
pub enum ActionSeverity {
    Trace,
    Info,
    Warning,
    Error,
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
pub struct ActionEvent {
    pub severity: ActionSeverity,
    pub message: String,
}

impl ActionEvent {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            severity: ActionSeverity::Info,
            message: message.into(),
        }
    }
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

#[async_trait]
pub trait CoreAction {
    type Request: Send + Sync;
    type Response: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn run(
        &self,
        context: &CoreContext,
        request: Self::Request,
        events: &mut dyn FnMut(ActionEvent),
    ) -> Result<Self::Response, Self::Error>;
}
