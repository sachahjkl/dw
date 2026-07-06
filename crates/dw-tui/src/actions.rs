use crate::model::{
    ActionRisk, TuiAction, TuiActionRequest, TuiDatabase, TuiSnapshot, WorkspaceAction,
    workspace_action,
};
use dw_core::{Agent, ConfigColorMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoItemAction {
    StartPreview,
    StartExecute,
    OpenAgent,
    Context,
    WorkItem,
    SetStartState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestAction {
    StartPreview,
    StartExecute,
    OpenAgent,
    FinishPreview,
    FinishExecute,
    Changelog,
    DiffPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseAction {
    Schema,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickOptionAction {
    Agent(Agent),
    Color(ConfigColorMode),
    ConfigShow,
    ConfigDoctor,
    Refresh,
    Guide,
    AgentDoctor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickOptionState {
    Agent(&'static str),
    Color(&'static str),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickOptionItem {
    pub key: char,
    pub section: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub action: QuickOptionAction,
    pub state: QuickOptionState,
}

pub const QUICK_OPTIONS: &[QuickOptionItem] = &[
    QuickOptionItem {
        key: '1',
        section: "Default agent",
        label: "opencode",
        hint: "Set opencode as the default agent",
        action: QuickOptionAction::Agent(Agent::Opencode),
        state: QuickOptionState::Agent("opencode"),
    },
    QuickOptionItem {
        key: '2',
        section: "Default agent",
        label: "cursor",
        hint: "Set cursor as the default agent",
        action: QuickOptionAction::Agent(Agent::Cursor),
        state: QuickOptionState::Agent("cursor"),
    },
    QuickOptionItem {
        key: '3',
        section: "Default agent",
        label: "claude",
        hint: "Set claude as the default agent",
        action: QuickOptionAction::Agent(Agent::Claude),
        state: QuickOptionState::Agent("claude"),
    },
    QuickOptionItem {
        key: '4',
        section: "Default agent",
        label: "codex",
        hint: "Set codex as the default agent",
        action: QuickOptionAction::Agent(Agent::Codex),
        state: QuickOptionState::Agent("codex"),
    },
    QuickOptionItem {
        key: '5',
        section: "Default agent",
        label: "codex-cli",
        hint: "Set codex-cli as the default agent",
        action: QuickOptionAction::Agent(Agent::CodexCli),
        state: QuickOptionState::Agent("codex-cli"),
    },
    QuickOptionItem {
        key: '6',
        section: "Default agent",
        label: "copilot",
        hint: "Set copilot as the default agent",
        action: QuickOptionAction::Agent(Agent::Copilot),
        state: QuickOptionState::Agent("copilot"),
    },
    QuickOptionItem {
        key: '7',
        section: "Terminal color mode",
        label: "auto",
        hint: "Follow terminal capabilities",
        action: QuickOptionAction::Color(ConfigColorMode::Auto),
        state: QuickOptionState::Color("auto"),
    },
    QuickOptionItem {
        key: '8',
        section: "Terminal color mode",
        label: "always",
        hint: "Always enable colors",
        action: QuickOptionAction::Color(ConfigColorMode::Always),
        state: QuickOptionState::Color("always"),
    },
    QuickOptionItem {
        key: '9',
        section: "Terminal color mode",
        label: "never",
        hint: "Disable colors",
        action: QuickOptionAction::Color(ConfigColorMode::Never),
        state: QuickOptionState::Color("never"),
    },
    QuickOptionItem {
        key: 's',
        section: "Diagnostics and setup",
        label: "show config",
        hint: "Show effective paths and settings",
        action: QuickOptionAction::ConfigShow,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'd',
        section: "Diagnostics and setup",
        label: "config doctor",
        hint: "Validate configuration files",
        action: QuickOptionAction::ConfigDoctor,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'r',
        section: "Diagnostics and setup",
        label: "refresh",
        hint: "Regenerate schemas and agent contexts",
        action: QuickOptionAction::Refresh,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'g',
        section: "Diagnostics and setup",
        label: "quick start",
        hint: "Show the startup path",
        action: QuickOptionAction::Guide,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'a',
        section: "Diagnostics and setup",
        label: "agent doctor",
        hint: "Check installed agents",
        action: QuickOptionAction::AgentDoctor,
        state: QuickOptionState::None,
    },
];

#[cfg(test)]
pub fn quick_option_by_key(key: char) -> Option<QuickOptionAction> {
    QUICK_OPTIONS
        .iter()
        .find(|item| item.key == key)
        .map(|item| item.action)
}

pub fn option_action(root: &str, action: QuickOptionAction) -> TuiAction {
    match action {
        QuickOptionAction::Agent(agent) => TuiAction {
            label: format!("Default agent · {agent}"),
            request: TuiActionRequest::AgentSetDefault {
                root: Some(dw_core::DevWorkflowRoot::from(root)),
                agent,
            },
            description: "Change the default agent".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::Color(mode) => TuiAction {
            label: format!("Color · {mode}"),
            request: TuiActionRequest::ConfigSetColor { mode },
            description: "Change terminal color mode".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::ConfigShow => TuiAction {
            label: "Show configuration".into(),
            request: TuiActionRequest::ConfigShow {
                root: Some(dw_core::DevWorkflowRoot::from(root)),
            },
            description: "Show effective paths and settings".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::ConfigDoctor => TuiAction {
            label: "Configuration doctor".into(),
            request: TuiActionRequest::ConfigDoctor {
                root: Some(dw_core::DevWorkflowRoot::from(root)),
            },
            description: "Validate configuration files".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::Refresh => TuiAction {
            label: "Refresh DevWorkflow".into(),
            request: TuiActionRequest::Refresh(dw_config::command::RefreshCommandArgs {
                root: Some(root.into()),
                profile: "business".into(),
            }),
            description: "Regenerate schemas and agent contexts".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::Guide => TuiAction {
            label: "Quick start".into(),
            request: TuiActionRequest::Guide,
            description: "Show the startup path".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::AgentDoctor => TuiAction {
            label: "Agent doctor".into(),
            request: TuiActionRequest::AgentDoctor { agent: None },
            description: "Check installed agents".into(),
            kind: ActionRisk::Safe,
        },
    }
}

pub fn selected_ado_action(
    snapshot: &TuiSnapshot,
    selected_project: usize,
    selected_item: usize,
    action: AdoItemAction,
) -> Option<TuiAction> {
    let project = snapshot.assigned.get(selected_project)?;
    let item = project.items.get(selected_item)?;
    let request = match action {
        AdoItemAction::StartPreview | AdoItemAction::StartExecute => {
            if snapshot
                .selected_work_item_workspace(selected_project, selected_item)
                .is_some()
            {
                return None;
            }
            TuiActionRequest::TaskStart(dw_task::start::StartArgs {
                work_item_ids: vec![dw_core::WorkItemId::from(item.id.clone())],
                root: Some(dw_core::DevWorkflowRoot::from(snapshot.root.clone())),
                project: Some(dw_core::ProjectKey::from(project.key.clone())),
                task: None,
                type_name: None,
                repositories: Vec::new(),
                slug: None,
                skip_ado: false,
                with_active_children: false,
                create_child_tasks: false,
                mode: dw_core::ExecutionMode::from_execute(action == AdoItemAction::StartExecute),
            })
        }
        AdoItemAction::OpenAgent => {
            let workspace =
                snapshot.selected_work_item_workspace(selected_project, selected_item)?;
            TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                workspace: Some(dw_core::WorkspacePath::from(workspace.path.clone())),
                project: None,
                work_item_ids: Vec::new(),
                pull_request: None,
                r#continue: false,
                repo: None,
                agent: None,
                root: Some(dw_core::DevWorkflowRoot::from(snapshot.root.clone())),
            })
        }
        AdoItemAction::Context => {
            TuiActionRequest::AdoContext(dw_ado_commands::commands::context::ContextArgs {
                ids: vec![dw_core::WorkItemId::from(item.id.clone())],
                root: Some(snapshot.root.clone()),
                project: Some(dw_core::ProjectKey::from(project.key.clone())),
                summary: false,
                comments: 200,
                mode: dw_ado_commands::commands::context::ContextMode::AiContext,
            })
        }
        AdoItemAction::WorkItem => {
            TuiActionRequest::AdoWorkItem(dw_ado_commands::commands::work_item::WorkItemArgs {
                ids: vec![dw_core::WorkItemId::from(item.id.clone())],
                root: Some(snapshot.root.clone()),
                project: Some(dw_core::ProjectKey::from(project.key.clone())),
            })
        }
        AdoItemAction::SetStartState => {
            let state = ado_start_state(snapshot, item)?;
            TuiActionRequest::AdoSetState(dw_ado_commands::commands::set_state::SetStateArgs {
                ids: vec![dw_core::WorkItemId::from(item.id.clone())],
                root: Some(snapshot.root.clone()),
                project: Some(dw_core::ProjectKey::from(project.key.clone())),
                state,
                history: None,
                yes: true,
            })
        }
    };

    Some(TuiAction {
        label: match action {
            AdoItemAction::OpenAgent => {
                format!("Open agent · #{}", item.id)
            }
            AdoItemAction::SetStartState => {
                format!("Move work item to ready state · #{}", item.id)
            }
            _ => format!("Prepare work item · #{}", item.id),
        },
        request,
        description: format!("{} · {}", project.key, item.title),
        kind: match action {
            AdoItemAction::StartExecute | AdoItemAction::SetStartState => ActionRisk::Destructive,
            AdoItemAction::OpenAgent => ActionRisk::OpensExternal,
            _ => ActionRisk::Safe,
        },
    })
}

fn ado_start_state(snapshot: &TuiSnapshot, item: &crate::model::AdoAssignedItem) -> Option<String> {
    let options = dw_workspace::task_start_options(&snapshot.workflow);
    dw_workspace::start_state(Some(&item.kind), &options)
}

pub fn selected_workspace_action(
    snapshot: &TuiSnapshot,
    selected_workspace: usize,
    action: WorkspaceAction,
) -> Option<TuiAction> {
    snapshot
        .workspaces
        .get(selected_workspace)
        .map(|workspace| {
            let mut action = workspace_action(workspace, action);
            action = action.with_root(snapshot.root.clone());
            action
        })
}

pub fn selected_pull_request_action(
    snapshot: &TuiSnapshot,
    selected_pull_request: usize,
    action: PullRequestAction,
) -> Option<TuiAction> {
    let item = snapshot.pull_requests.get(selected_pull_request)?;
    let pull_request_id = item.pull_request_id?;
    let request = match action {
        PullRequestAction::StartPreview | PullRequestAction::StartExecute => {
            if item.workspace.is_some() {
                return None;
            }
            TuiActionRequest::TaskStartPr(dw_task::start::StartPrArgs {
                pull_request_id: dw_core::PullRequestId::from(pull_request_id.to_string()),
                root: Some(dw_core::DevWorkflowRoot::from(snapshot.root.clone())),
                project: dw_core::ProjectKey::from(item.project.clone()),
                repositories: vec![dw_core::WorkspaceRepositoryName::from(
                    item.repository.clone(),
                )],
                type_name: None,
                slug: None,
                mode: dw_core::ExecutionMode::from_execute(
                    action == PullRequestAction::StartExecute,
                ),
            })
        }
        PullRequestAction::OpenAgent => {
            TuiActionRequest::AgentOpen(dw_task::open::OpenWorkspaceArgs {
                workspace: Some(dw_core::WorkspacePath::from(item.workspace.clone()?)),
                project: None,
                work_item_ids: Vec::new(),
                pull_request: None,
                r#continue: false,
                repo: Some(dw_core::WorkspaceRepositoryName::from(
                    item.repository.clone(),
                )),
                agent: None,
                root: Some(dw_core::DevWorkflowRoot::from(snapshot.root.clone())),
            })
        }
        PullRequestAction::FinishPreview | PullRequestAction::FinishExecute => {
            TuiActionRequest::TaskFinish(dw_task::finish::FinishArgs {
                workspace: Some(dw_core::WorkspacePath::from(item.workspace.clone()?)),
                r#continue: false,
                root: Some(dw_core::DevWorkflowRoot::from(snapshot.root.clone())),
                mode: dw_core::ExecutionMode::from_execute(
                    action == PullRequestAction::FinishExecute,
                ),
                yes: action == PullRequestAction::FinishExecute,
                message: None,
                create_pr: true,
                ready: false,
                skip_verify: false,
                skip_ado: false,
            })
        }
        PullRequestAction::DiffPreview => TuiActionRequest::TaskCommit(dw_task::repo::CommitArgs {
            workspace: Some(dw_core::WorkspacePath::from(item.workspace.clone()?)),
            r#continue: false,
            root: Some(dw_core::DevWorkflowRoot::from(snapshot.root.clone())),
            mode: dw_core::ExecutionMode::Preview,
            message: None,
        }),
        PullRequestAction::Changelog => {
            TuiActionRequest::AdoChangelog(dw_ado_commands::commands::changelog::ChangelogArgs {
                source: dw_ado_commands::commands::changelog::ChangelogSource::PullRequests(vec![
                    dw_core::PullRequestId::from(pull_request_id.to_string()),
                ]),
                root: Some(snapshot.root.clone()),
                project: Some(dw_core::ProjectKey::from(item.project.clone())),
                repo: Some(item.ado_repository.clone()),
                group_by_parent: false,
                format: None,
                table: false,
                ids_only: false,
            })
        }
    };

    Some(TuiAction {
        label: match action {
            PullRequestAction::StartPreview => {
                format!("Preview PR workspace · {}", item.repository)
            }
            PullRequestAction::StartExecute => format!("Create PR workspace · {}", item.repository),
            PullRequestAction::OpenAgent => format!("Open agent · {}", item.repository),
            PullRequestAction::FinishPreview => format!("Preview PR finish · {}", item.repository),
            PullRequestAction::FinishExecute => format!("Finish PR · {}", item.repository),
            PullRequestAction::DiffPreview => format!("Inspect local diff · {}", item.repository),
            PullRequestAction::Changelog => format!("Summarize changes · {}", item.repository),
        },
        request,
        description: format!("{} · {}", item.project, item.branch),
        kind: match action {
            PullRequestAction::StartExecute | PullRequestAction::FinishExecute => {
                ActionRisk::Destructive
            }
            PullRequestAction::OpenAgent => ActionRisk::OpensExternal,
            _ => ActionRisk::Safe,
        },
    })
}

pub fn selected_database_action(
    snapshot: &TuiSnapshot,
    selected_database: usize,
    action: DatabaseAction,
) -> Option<TuiAction> {
    snapshot
        .database_entries
        .get(selected_database)
        .map(|database| database_action(database, action))
}

pub fn database_action(database: &TuiDatabase, action: DatabaseAction) -> TuiAction {
    let verb = match action {
        DatabaseAction::Schema => "schema",
    };
    let label = if let Some(project) = database.project.as_deref() {
        format!("DB {verb} · {project}/{}", database.key)
    } else {
        format!("DB {verb} · {}", database.key)
    };

    TuiAction {
        label,
        request: TuiActionRequest::DbSchema(dw_db::commands::SchemaArgs {
            project: database.project.clone(),
            database: Some(database.key.clone()),
            env: None,
        }),
        description: match action {
            DatabaseAction::Schema => "Inspect the read-only schema".into(),
        },
        kind: ActionRisk::Safe,
    }
}

pub fn pull_request_action_error(
    snapshot: &TuiSnapshot,
    selected_pull_request: usize,
    action: PullRequestAction,
) -> String {
    let Some(item) = snapshot.pull_requests.get(selected_pull_request) else {
        return "No PR selected.".into();
    };
    if item.pull_request_id.is_none() {
        return "This row is not an actionable PR.".into();
    }
    match action {
        PullRequestAction::StartPreview | PullRequestAction::StartExecute
            if item.workspace.is_some() =>
        {
            "Action unavailable: a workspace is already linked to this PR. Open the agent instead."
                .into()
        }
        PullRequestAction::OpenAgent if item.workspace.is_none() => {
            "Action unavailable: no local workspace is linked to this PR. Create the workspace first."
                .into()
        }
        PullRequestAction::FinishPreview
        | PullRequestAction::FinishExecute
        | PullRequestAction::DiffPreview
            if item.workspace.is_none() =>
        {
            "Action unavailable: no local workspace is linked to this PR. Press x to create a workspace from the PR.".into()
        }
        _ => "PR action unavailable for this row.".into(),
    }
}

pub fn ado_action_error(
    snapshot: &TuiSnapshot,
    selected_project: usize,
    selected_item: usize,
    action: AdoItemAction,
) -> Option<String> {
    let project = snapshot.assigned.get(selected_project)?;
    let item = project.items.get(selected_item)?;
    match action {
        AdoItemAction::StartPreview | AdoItemAction::StartExecute
            if snapshot
                .selected_work_item_workspace(selected_project, selected_item)
                .is_some() =>
        {
            Some(
                "Action unavailable: a workspace is already linked to this work item. Open the agent instead."
                    .into(),
            )
        }
        AdoItemAction::OpenAgent
            if snapshot
                .selected_work_item_workspace(selected_project, selected_item)
                .is_none() =>
        {
            Some(format!(
                "Action unavailable: no local workspace is linked to work item #{}. Create the workspace first.",
                item.id
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AdoAssignedItem, AdoAssignedProject, TuiDatabase, TuiPullRequest};
    use dw_config::{DatabasesConfig, ProjectsConfig, WorkflowConfig};
    use dw_workspace::TaskListItem;

    #[test]
    fn ado_start_execute_builds_workspace_action() {
        let mut snapshot = snapshot();
        snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "Hommage Agence".into(),
            items: vec![AdoAssignedItem {
                id: "42".into(),
                kind: "User Story".into(),
                state: "Active".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }];

        let action =
            selected_ado_action(&snapshot, 0, 0, AdoItemAction::StartExecute).expect("action");

        match &action.request {
            TuiActionRequest::TaskStart(args) => {
                assert_eq!(args.work_item_ids, vec![dw_core::WorkItemId::from("42")]);
                assert_eq!(
                    args.root.as_ref().map(dw_core::DevWorkflowRoot::as_str),
                    Some("/tmp/dw")
                );
                assert_eq!(
                    args.project.as_ref().map(|project| project.as_str()),
                    Some("ha")
                );
                assert!(args.mode.executes());
            }
            _ => panic!("expected task start request"),
        }
        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn ado_existing_workspace_disables_create_and_opens_agent() {
        let mut snapshot = snapshot();
        snapshot.workspaces = vec![workspace("/tmp/ws-42", "demo")];
        snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "Hommage Agence".into(),
            items: vec![AdoAssignedItem {
                id: "42".into(),
                kind: "User Story".into(),
                state: "Active".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }];

        assert!(selected_ado_action(&snapshot, 0, 0, AdoItemAction::StartExecute).is_none());
        let action =
            selected_ado_action(&snapshot, 0, 0, AdoItemAction::OpenAgent).expect("open action");

        match &action.request {
            TuiActionRequest::AgentOpen(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws-42")
                );
                assert_eq!(
                    args.root.as_ref().map(dw_core::DevWorkflowRoot::as_str),
                    Some("/tmp/dw")
                );
            }
            _ => panic!("expected agent open request"),
        }
        assert!(matches!(action.kind, ActionRisk::OpensExternal));
    }

    #[test]
    fn ado_existing_workspace_matches_child_task_ids() {
        let mut snapshot = snapshot();
        let mut workspace = workspace("/tmp/ws-42", "demo");
        workspace.task_id = Some("456".into());
        workspace.all_known_work_item_ids = vec!["42".into(), "456".into()];
        snapshot.workspaces = vec![workspace];
        snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "Hommage Agence".into(),
            items: vec![AdoAssignedItem {
                id: "456".into(),
                kind: "Task".into(),
                state: "Active".into(),
                title: "Child task".into(),
                url: None,
            }],
            error: None,
        }];

        assert!(selected_ado_action(&snapshot, 0, 0, AdoItemAction::StartExecute).is_none());
        let action =
            selected_ado_action(&snapshot, 0, 0, AdoItemAction::OpenAgent).expect("open action");

        match &action.request {
            TuiActionRequest::AgentOpen(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws-42")
                );
            }
            _ => panic!("expected agent open request"),
        }
    }

    #[test]
    fn ado_set_start_state_uses_workflow_state_and_bypasses_cli_confirmation() {
        let mut snapshot = snapshot();
        snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "Hommage Agence".into(),
            items: vec![AdoAssignedItem {
                id: "42".into(),
                kind: "User Story".into(),
                state: "Nouveau".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }];

        let action =
            selected_ado_action(&snapshot, 0, 0, AdoItemAction::SetStartState).expect("action");

        match &action.request {
            TuiActionRequest::AdoSetState(args) => {
                assert_eq!(args.ids, vec![dw_core::WorkItemId::from("42")]);
                assert_eq!(
                    args.project.as_ref().map(|project| project.as_str()),
                    Some("ha")
                );
                assert_eq!(args.root.as_deref(), Some("/tmp/dw"));
                assert_eq!(args.state, "En réalisation");
                assert!(args.yes);
            }
            _ => panic!("expected ado set-state request"),
        }
        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(action.bypasses_cli_confirmation());
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn pr_start_preview_builds_start_pr_action() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![pull_request(None)];

        let action = selected_pull_request_action(&snapshot, 0, PullRequestAction::StartPreview)
            .expect("action");

        match &action.request {
            TuiActionRequest::TaskStartPr(args) => {
                assert_eq!(args.pull_request_id, dw_core::PullRequestId::from("42"));
                assert_eq!(
                    args.repositories
                        .first()
                        .map(dw_core::WorkspaceRepositoryName::as_str),
                    Some("front")
                );
                assert!(!args.mode.executes());
            }
            _ => panic!("expected PR workspace request"),
        }
    }

    #[test]
    fn pr_existing_workspace_disables_create_and_opens_agent() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![pull_request(Some("/tmp/ws-pr"))];

        assert!(
            selected_pull_request_action(&snapshot, 0, PullRequestAction::StartExecute).is_none()
        );
        let action = selected_pull_request_action(&snapshot, 0, PullRequestAction::OpenAgent)
            .expect("open action");

        match &action.request {
            TuiActionRequest::AgentOpen(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws-pr")
                );
                assert_eq!(
                    args.repo
                        .as_ref()
                        .map(dw_core::WorkspaceRepositoryName::as_str),
                    Some("front")
                );
                assert_eq!(
                    args.root.as_ref().map(dw_core::DevWorkflowRoot::as_str),
                    Some("/tmp/dw")
                );
            }
            _ => panic!("expected agent open request"),
        }
        assert!(matches!(action.kind, ActionRisk::OpensExternal));
    }

    #[test]
    fn pr_start_execute_runs_in_background_after_tui_confirmation() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![pull_request(None)];

        let action = selected_pull_request_action(&snapshot, 0, PullRequestAction::StartExecute)
            .expect("action");

        match &action.request {
            TuiActionRequest::TaskStartPr(args) => {
                assert_eq!(args.pull_request_id, dw_core::PullRequestId::from("42"));
                assert!(args.mode.executes());
            }
            _ => panic!("expected PR workspace request"),
        }
        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn pr_finish_execute_runs_in_background_after_tui_confirmation() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![pull_request(Some("/tmp/ws"))];

        let action = selected_pull_request_action(&snapshot, 0, PullRequestAction::FinishExecute)
            .expect("action");

        match &action.request {
            TuiActionRequest::TaskFinish(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws")
                );
                assert!(args.create_pr);
                assert!(args.mode.executes());
                assert!(args.yes);
            }
            _ => panic!("expected task finish request"),
        }
        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(action.bypasses_cli_confirmation());
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn pr_local_action_explains_missing_workspace() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![pull_request(None)];

        assert_eq!(
            pull_request_action_error(&snapshot, 0, PullRequestAction::FinishExecute),
            "Action unavailable: no local workspace is linked to this PR. Press x to create a workspace from the PR."
        );
    }

    #[test]
    fn option_actions_build_rooted_config_actions() {
        let show = option_action("/tmp/dw", QuickOptionAction::ConfigShow);
        let doctor = option_action("/tmp/dw", QuickOptionAction::ConfigDoctor);
        let refresh = option_action("/tmp/dw", QuickOptionAction::Refresh);

        assert!(matches!(
            show.request,
            TuiActionRequest::ConfigShow { root: Some(ref root) } if root.as_str() == "/tmp/dw"
        ));
        assert!(matches!(
            doctor.request,
            TuiActionRequest::ConfigDoctor { root: Some(ref root) } if root.as_str() == "/tmp/dw"
        ));
        assert!(matches!(
            refresh.request,
            TuiActionRequest::Refresh(ref args) if args.root.as_deref() == Some("/tmp/dw")
        ));
    }

    #[test]
    fn option_actions_build_preferences_actions() {
        let agent = option_action("/tmp/dw", QuickOptionAction::Agent(Agent::Codex));
        let color = option_action("/tmp/dw", QuickOptionAction::Color(ConfigColorMode::Always));

        assert!(matches!(
            agent.request,
            TuiActionRequest::AgentSetDefault { root: Some(ref root), ref agent }
                if root.as_str() == "/tmp/dw" && *agent == Agent::Codex
        ));
        assert!(matches!(
            color.request,
            TuiActionRequest::ConfigSetColor { mode } if mode == ConfigColorMode::Always
        ));
    }

    #[test]
    fn quick_option_catalog_exposes_shortcuts_and_actions() {
        assert_eq!(QUICK_OPTIONS[0].key, '1');
        assert_eq!(
            quick_option_by_key('4'),
            Some(QuickOptionAction::Agent(Agent::Codex))
        );
        assert_eq!(
            quick_option_by_key('5'),
            Some(QuickOptionAction::Agent(Agent::CodexCli))
        );
        assert_eq!(
            quick_option_by_key('d'),
            Some(QuickOptionAction::ConfigDoctor)
        );
        assert!(quick_option_by_key('x').is_none());
    }

    #[test]
    fn quick_option_agents_match_config_choices() {
        let agents = QUICK_OPTIONS
            .iter()
            .filter_map(|item| match item.action {
                QuickOptionAction::Agent(agent) => Some(agent),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            agents.into_iter().map(Agent::as_str).collect::<Vec<_>>(),
            dw_config::AGENT_DEFAULT_CHOICES
        );
    }

    #[test]
    fn database_schema_action_builds_project_or_global_action() {
        let global = database_action(
            &TuiDatabase {
                project: None,
                key: "shared".into(),
            },
            DatabaseAction::Schema,
        );
        let schema = database_action(
            &TuiDatabase {
                project: Some("ha".into()),
                key: "ha-dev".into(),
            },
            DatabaseAction::Schema,
        );

        assert!(matches!(
            global.request,
            TuiActionRequest::DbSchema(ref args)
                if args.project.is_none() && args.database.as_deref() == Some("shared")
        ));
        assert!(matches!(
            schema.request,
            TuiActionRequest::DbSchema(ref args)
                if args.project.as_deref() == Some("ha")
                    && args.database.as_deref() == Some("ha-dev")
        ));
    }

    #[test]
    fn workspace_actions_build_contextual_actions() {
        let workspace = workspace("/tmp/ws", "demo");
        let open = workspace_action(&workspace, WorkspaceAction::Open);
        let preflight = workspace_action(&workspace, WorkspaceAction::Preflight);
        let latest = workspace_action(&workspace, WorkspaceAction::RepoLatest);
        let handoff = workspace_action(&workspace, WorkspaceAction::HandoffValidate);
        let finish = workspace_action(&workspace, WorkspaceAction::FinishPreview);
        let teardown = workspace_action(&workspace, WorkspaceAction::TeardownExecute);

        assert!(matches!(
            open.request,
            TuiActionRequest::AgentOpen(ref args)
                if args.workspace.as_ref().map(dw_core::WorkspacePath::as_str) == Some("/tmp/ws")
        ));
        assert!(matches!(open.kind, ActionRisk::OpensExternal));
        assert_eq!(preflight.workspace_path(), Some("/tmp/ws"));
        assert_eq!(latest.workspace_path(), Some("/tmp/ws"));
        assert_eq!(handoff.workspace_path(), Some("/tmp/ws"));
        assert_eq!(finish.workspace_path(), Some("/tmp/ws"));
        match &teardown.request {
            TuiActionRequest::TaskTeardown(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws")
                );
                assert!(args.mode.executes());
                assert!(args.yes);
            }
            _ => panic!("expected teardown request"),
        }
        assert!(matches!(teardown.kind, ActionRisk::Destructive));
    }

    #[test]
    fn selected_workspace_actions_include_snapshot_root() {
        let mut snapshot = snapshot();
        snapshot.workspaces = vec![workspace("/tmp/ws", "demo")];

        let action = selected_workspace_action(&snapshot, 0, WorkspaceAction::Sync)
            .expect("workspace action");

        match &action.request {
            TuiActionRequest::TaskSync(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws")
                );
                assert_eq!(
                    args.root.as_ref().map(dw_core::DevWorkflowRoot::as_str),
                    Some("/tmp/dw")
                );
            }
            _ => panic!("expected sync request"),
        }
    }

    fn snapshot() -> TuiSnapshot {
        TuiSnapshot {
            root: "/tmp/dw".into(),
            needs_init: false,
            projects: ProjectsConfig::default(),
            workflow: WorkflowConfig::default(),
            databases: DatabasesConfig::default(),
            database_entries: Vec::new(),
            config_doctor: config_doctor_report(),
            workspaces: Vec::new(),
            assigned: Vec::new(),
            assigned_loaded: false,
            pull_requests: Vec::new(),
            pull_requests_loaded: false,
            prune_candidates: 0,
            actions: Vec::new(),
            color_mode: "auto".into(),
        }
    }

    fn config_doctor_report() -> dw_config::ConfigDoctorReport {
        dw_config::ConfigDoctorReport {
            root: "/tmp/dw".into(),
            checks: Vec::new(),
            passed: true,
        }
    }

    fn pull_request(workspace: Option<&str>) -> TuiPullRequest {
        TuiPullRequest {
            workspace: workspace.map(str::to_string),
            project: "ha".into(),
            repository: "front".into(),
            ado_repository: "Front".into(),
            branch: "feature/demo".into(),
            target_branch: "develop".into(),
            pull_request_id: Some(42),
            title: Some("Demo".into()),
            is_draft: false,
            work_item_ids: vec!["123".into()],
            url: None,
            error: None,
        }
    }

    fn workspace(path: &str, slug: &str) -> TaskListItem {
        TaskListItem {
            path: path.into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            display_work_items: "#42 Demo".into(),
            task_id: None,
            all_known_work_item_ids: vec!["42".into()],
            kind: "feature".into(),
            slug: slug.into(),
            branch_name: format!("feature/42-{slug}"),
            created_at: "2026-07-04T00:00:00Z".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Demo".into()),
            work_item_state: Some("Active".into()),
            repositories: vec!["front".into()],
        }
    }
}
