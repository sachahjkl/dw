use crate::json::read_json;
use crate::types::{DatabasesConfig, WorkflowConfig};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub const AGENT_DEFAULT_CHOICES: &[&str] = &[
    "opencode",
    "cursor",
    "claude",
    "codex",
    "codex-cli",
    "copilot",
];

pub fn normalize_default_agent(agent: &str) -> Option<&'static str> {
    AGENT_DEFAULT_CHOICES
        .iter()
        .copied()
        .find(|item| item.eq_ignore_ascii_case(agent.trim()))
}

pub fn load_workflow_config(root: &str) -> WorkflowConfig {
    let path = Path::new(root).join("config").join("workflow.json");
    read_json::<WorkflowConfig>(&path).unwrap_or_default()
}

pub fn load_databases_config(root: &str) -> DatabasesConfig {
    let path = Path::new(root).join("config").join("databases.json");
    read_json::<DatabasesConfig>(&path).unwrap_or_default()
}

pub fn set_default_agent(root: &str, agent: &str) -> std::io::Result<String> {
    let Some(normalized_agent) = normalize_default_agent(agent) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Agent inconnu: {agent}. Agents disponibles: {}",
                AGENT_DEFAULT_CHOICES.join(", ")
            ),
        ));
    };
    let path = Path::new(root).join("config").join("workflow.json");
    let text = fs::read_to_string(&path)?;
    let mut value: Value = serde_json::from_str(&text).map_err(std::io::Error::other)?;
    if !value.is_object() {
        value = serde_json::json!({});
    }
    let object = value.as_object_mut().expect("object was ensured");
    let agent_node = object
        .entry("agent")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("workflow.agent doit etre un objet JSON"))?;
    agent_node.insert("default".into(), Value::String(normalized_agent.into()));
    fs::write(path, serde_json::to_string_pretty(&value)?)?;
    Ok(normalized_agent.into())
}

pub fn default_agent(root: &str) -> String {
    load_workflow_config(root)
        .agent
        .map(|agent| agent.default)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "opencode".into())
}
