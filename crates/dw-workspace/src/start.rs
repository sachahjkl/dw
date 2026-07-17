use dw_config::WorkflowConfig;
use dw_core::WorkItemState;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStartOptions {
    pub update_work_item_state: bool,
    pub create_child_tasks: bool,
    pub user_story_state: WorkItemState,
    pub anomaly_state: WorkItemState,
    pub bug_state: WorkItemState,
    pub task_state: WorkItemState,
}

impl Default for TaskStartOptions {
    fn default() -> Self {
        Self {
            update_work_item_state: true,
            create_child_tasks: false,
            user_story_state: WorkItemState::from("En réalisation"),
            anomaly_state: WorkItemState::from("En réalisation"),
            bug_state: WorkItemState::from("En développement"),
            task_state: WorkItemState::from("En développement"),
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
        state_property(value, "userStoryState").unwrap_or(options.user_story_state);
    options.anomaly_state = state_property(value, "anomalyState").unwrap_or(options.anomaly_state);
    options.bug_state = state_property(value, "bugState").unwrap_or(options.bug_state);
    options.task_state = state_property(value, "taskState").unwrap_or(options.task_state);
    options
}

pub fn start_state(
    work_item_type: Option<&str>,
    options: &TaskStartOptions,
) -> Option<WorkItemState> {
    match normalize_work_item_type(work_item_type).as_str() {
        "user story" => Some(options.user_story_state.clone()),
        "anomalie" => Some(options.anomaly_state.clone()),
        "bug" | "activite" | "activité" => Some(options.bug_state.clone()),
        "task" | "tache" | "tâche" => Some(options.task_state.clone()),
        _ => None,
    }
}

fn bool_property(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn state_property(value: &Value, key: &str) -> Option<WorkItemState> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(WorkItemState::from)
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
            start_state(Some("User Story"), &options),
            Some(WorkItemState::from("En réalisation"))
        );
        assert_eq!(
            start_state(Some("Anomalie"), &options),
            Some(WorkItemState::from("En réalisation"))
        );
        assert_eq!(
            start_state(Some("Bug"), &options),
            Some(WorkItemState::from("En développement"))
        );
        assert_eq!(
            start_state(Some("Activité"), &options),
            Some(WorkItemState::from("En développement"))
        );
        assert_eq!(
            start_state(Some("Activite"), &options),
            Some(WorkItemState::from("En développement"))
        );
        assert_eq!(
            start_state(Some("Tâche"), &options),
            Some(WorkItemState::from("En développement"))
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
        assert_eq!(options.user_story_state, WorkItemState::from("Ready"));
        assert_eq!(options.anomaly_state, WorkItemState::from("Analyse"));
        assert_eq!(options.bug_state, WorkItemState::from("Dev"));
        assert_eq!(options.task_state, WorkItemState::from("Todo"));
    }
}
