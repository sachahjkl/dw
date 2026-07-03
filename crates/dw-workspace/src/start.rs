use dw_config::WorkflowConfig;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStartOptions {
    pub update_work_item_state: bool,
    pub create_child_tasks: bool,
    pub user_story_state: String,
    pub anomaly_state: String,
    pub bug_state: String,
    pub task_state: String,
}

impl Default for TaskStartOptions {
    fn default() -> Self {
        Self {
            update_work_item_state: true,
            create_child_tasks: false,
            user_story_state: "En réalisation".into(),
            anomaly_state: "En réalisation".into(),
            bug_state: "En développement".into(),
            task_state: "En développement".into(),
        }
    }
}

pub fn task_start_options(workflow: &WorkflowConfig) -> TaskStartOptions {
    let Some(value) = workflow.task_start.as_ref() else {
        return TaskStartOptions::default();
    };

    let mut options = TaskStartOptions::default();
    options.update_work_item_state =
        bool_property(value, "updateWorkItemState", options.update_work_item_state);
    options.create_child_tasks =
        bool_property(value, "createChildTasks", options.create_child_tasks);
    options.user_story_state =
        string_property(value, "userStoryState").unwrap_or(options.user_story_state);
    options.anomaly_state = string_property(value, "anomalyState").unwrap_or(options.anomaly_state);
    options.bug_state = string_property(value, "bugState").unwrap_or(options.bug_state);
    options.task_state = string_property(value, "taskState").unwrap_or(options.task_state);
    options
}

pub fn start_state(work_item_type: Option<&str>, options: &TaskStartOptions) -> Option<String> {
    match normalize_work_item_type(work_item_type).as_str() {
        "user story" => Some(options.user_story_state.clone()),
        "anomalie" => Some(options.anomaly_state.clone()),
        "bug" => Some(options.bug_state.clone()),
        "task" | "tache" | "tâche" => Some(options.task_state.clone()),
        _ => None,
    }
    .filter(|state| !state.trim().is_empty())
}

fn bool_property(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn string_property(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_work_item_type(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .replace('â', "a")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_state_matches_dotnet_defaults() {
        let options = TaskStartOptions::default();

        assert_eq!(
            start_state(Some("User Story"), &options).as_deref(),
            Some("En réalisation")
        );
        assert_eq!(
            start_state(Some("Anomalie"), &options).as_deref(),
            Some("En réalisation")
        );
        assert_eq!(
            start_state(Some("Bug"), &options).as_deref(),
            Some("En développement")
        );
        assert_eq!(
            start_state(Some("Tâche"), &options).as_deref(),
            Some("En développement")
        );
        assert_eq!(start_state(Some("Epic"), &options), None);
    }

    #[test]
    fn task_start_options_read_configured_values() {
        let workflow = WorkflowConfig {
            task_start: Some(serde_json::json!({
                "updateWorkItemState": false,
                "createChildTasks": true,
                "userStoryState": "Ready",
                "anomalyState": "Analyse",
                "bugState": "Dev",
                "taskState": "Todo"
            })),
            ..WorkflowConfig::default()
        };

        let options = task_start_options(&workflow);

        assert!(!options.update_work_item_state);
        assert!(options.create_child_tasks);
        assert_eq!(options.user_story_state, "Ready");
        assert_eq!(options.anomaly_state, "Analyse");
        assert_eq!(options.bug_state, "Dev");
        assert_eq!(options.task_state, "Todo");
    }
}
