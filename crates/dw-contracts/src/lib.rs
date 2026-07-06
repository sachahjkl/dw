use dw_core::{ProjectKey, WorkItemId, WorkspacePath, WorkspaceRepositoryName};
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod completion {
    #[derive(Clone, Copy)]
    pub struct CompletionContext<'a> {
        pub root: &'a str,
        pub project: Option<&'a str>,
        pub workspace: Option<&'a str>,
        pub work_item: Option<&'a str>,
    }

    #[derive(Clone, Copy)]
    pub struct CompletionCatalog {
        pub subcommands: fn() -> &'static [&'static str],
        pub options_for: fn(&str) -> Vec<&'static str>,
        pub option_requires_value: fn(&str) -> bool,
        pub option_allowed: fn(&str, &[&str]) -> bool,
        pub values_for: fn(&str, CompletionContext<'_>) -> Option<Vec<String>>,
    }
}

pub const AI_CONTEXT_VERSION: &str = "dw.ado.ai-context.v1";
pub const PREFLIGHT_VERSION: &str = "dw.task.preflight.v1";
pub const HANDOFF_VALIDATION_VERSION: &str = "dw.task.handoff-validation.v1";
pub const HANDOFF_PREFIX: &str = "handoff-";
pub const MARKDOWN_EXTENSION: &str = ".md";
pub const ATTACHMENT_DIRECTORY_PREFIX: &str = "attachments/ado/";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuredEnvelope<T> {
    pub kind: String,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskHandoffValidationReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub workspace: WorkspacePath,
    pub project: ProjectKey,
    pub items: Vec<TaskHandoffValidationItem>,
    #[serde(rename = "isValid")]
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskHandoffValidationItem {
    pub repository: WorkspaceRepositoryName,
    pub path: String,
    pub status: TaskHandoffValidationStatus,
    pub valid: bool,
    pub message: String,
    #[serde(rename = "doneCount")]
    pub done_count: usize,
    #[serde(rename = "decisionCount")]
    pub decision_count: usize,
    #[serde(rename = "riskCount")]
    pub risk_count: usize,
    #[serde(rename = "blockerCount")]
    pub blocker_count: usize,
    #[serde(rename = "followUpCount")]
    pub follow_up_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskHandoffValidationStatus {
    Missing,
    Invalid,
    Blocked,
    Todo,
    InProgress,
    Valid,
}

impl fmt::Display for TaskHandoffValidationStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Blocked => "blocked",
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Valid => "valid",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextItem {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "workItem")]
    pub work_item: AdoAiContextWorkItem,
    pub core: AdoAiContextCore,
    pub content: AdoAiContextContent,
    pub links: AdoAiContextLinks,
    pub attachments: AdoAiContextAttachments,
    pub relations: Vec<AdoAiContextRelation>,
    pub comments: Vec<AdoAiContextComment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextWorkItem {
    pub id: WorkItemId,
    pub url: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub state: Option<String>,
    #[serde(rename = "assignedTo")]
    pub assigned_to: Option<String>,
    #[serde(rename = "areaPath")]
    pub area_path: Option<String>,
    #[serde(rename = "iterationPath")]
    pub iteration_path: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextCore {
    #[serde(rename = "createdBy")]
    pub created_by: Option<String>,
    #[serde(rename = "createdDate")]
    pub created_date: Option<String>,
    #[serde(rename = "changedBy")]
    pub changed_by: Option<String>,
    #[serde(rename = "changedDate")]
    pub changed_date: Option<String>,
    pub priority: Option<String>,
    #[serde(rename = "valueArea")]
    pub value_area: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextContent {
    pub description: Option<String>,
    #[serde(rename = "acceptanceCriteria")]
    pub acceptance_criteria: Option<String>,
    #[serde(rename = "productContext")]
    pub product_context: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextLinks {
    #[serde(rename = "parentIds")]
    pub parent_ids: Vec<WorkItemId>,
    #[serde(rename = "childIds")]
    pub child_ids: Vec<WorkItemId>,
    #[serde(rename = "predecessorIds")]
    pub predecessor_ids: Vec<WorkItemId>,
    #[serde(rename = "successorIds")]
    pub successor_ids: Vec<WorkItemId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextAttachments {
    #[serde(rename = "directoryHint")]
    pub directory_hint: String,
    pub items: Vec<AdoAiContextAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextAttachment {
    pub name: Option<String>,
    pub url: Option<String>,
    pub comment: Option<String>,
    #[serde(rename = "directoryHint")]
    pub directory_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextRelation {
    pub kind: String,
    pub rel: Option<String>,
    #[serde(rename = "workItemId")]
    pub work_item_id: Option<WorkItemId>,
    pub name: Option<String>,
    pub url: Option<String>,
    pub comment: Option<String>,
    pub artifact: Option<String>,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoAiContextComment {
    pub author: Option<String>,
    #[serde(rename = "createdDate")]
    pub created_date: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskPreflightReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub workspace: WorkspacePath,
    pub project: ProjectKey,
    #[serde(rename = "workItemIds")]
    pub work_item_ids: Vec<WorkItemId>,
    pub issues: Vec<TaskPreflightIssue>,
    #[serde(rename = "hasBlockingIssues")]
    pub has_blocking_issues: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskPreflightIssue {
    pub code: String,
    pub severity: TaskPreflightSeverity,
    #[serde(rename = "workItemId")]
    pub work_item_id: WorkItemId,
    pub message: String,
    pub details: Option<String>,
    #[serde(rename = "relatedIds")]
    pub related_ids: Vec<WorkItemId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TaskPreflightSeverity {
    Blocking,
    Warning,
    Info,
}

impl TaskPreflightSeverity {
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::Blocking)
    }

    pub fn is_warning(self) -> bool {
        matches!(self, Self::Warning)
    }
}

impl fmt::Display for TaskPreflightSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Blocking => "blocking",
            Self::Warning => "warning",
            Self::Info => "info",
        })
    }
}
