use dw_core::{ActionRisk, ExternalLaunchPlan};
use serde::{Deserialize, Serialize};

pub mod render;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiActionMetadata {
    pub label: String,
    pub description: String,
    pub risk: ActionRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiPanel {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiTable {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TuiExternalAction {
    pub metadata: TuiActionMetadata,
    pub launch: ExternalLaunchPlan,
}
