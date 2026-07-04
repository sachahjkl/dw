use crate::model::{
    ActionRisk, TuiAction, TuiActionRequest, TuiDatabase, TuiSnapshot, WorkspaceAction,
    workspace_action,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdoItemAction {
    StartPreview,
    StartExecute,
    Context,
    WorkItem,
    SetStartState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestAction {
    StartPreview,
    StartExecute,
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
    Agent(&'static str),
    Color(&'static str),
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
        section: "Agent par défaut",
        label: "opencode",
        hint: "Définir opencode comme agent par défaut",
        action: QuickOptionAction::Agent("opencode"),
        state: QuickOptionState::Agent("opencode"),
    },
    QuickOptionItem {
        key: '2',
        section: "Agent par défaut",
        label: "cursor",
        hint: "Définir cursor comme agent par défaut",
        action: QuickOptionAction::Agent("cursor"),
        state: QuickOptionState::Agent("cursor"),
    },
    QuickOptionItem {
        key: '3',
        section: "Agent par défaut",
        label: "claude",
        hint: "Définir claude comme agent par défaut",
        action: QuickOptionAction::Agent("claude"),
        state: QuickOptionState::Agent("claude"),
    },
    QuickOptionItem {
        key: '4',
        section: "Agent par défaut",
        label: "codex",
        hint: "Définir codex comme agent par défaut",
        action: QuickOptionAction::Agent("codex"),
        state: QuickOptionState::Agent("codex"),
    },
    QuickOptionItem {
        key: '5',
        section: "Agent par défaut",
        label: "codex-cli",
        hint: "Définir codex-cli comme agent par défaut",
        action: QuickOptionAction::Agent("codex-cli"),
        state: QuickOptionState::Agent("codex-cli"),
    },
    QuickOptionItem {
        key: '6',
        section: "Agent par défaut",
        label: "copilot",
        hint: "Définir copilot comme agent par défaut",
        action: QuickOptionAction::Agent("copilot"),
        state: QuickOptionState::Agent("copilot"),
    },
    QuickOptionItem {
        key: '7',
        section: "Mode couleur terminal",
        label: "auto",
        hint: "Choisir selon le terminal",
        action: QuickOptionAction::Color("auto"),
        state: QuickOptionState::Color("auto"),
    },
    QuickOptionItem {
        key: '8',
        section: "Mode couleur terminal",
        label: "always",
        hint: "Forcer les couleurs",
        action: QuickOptionAction::Color("always"),
        state: QuickOptionState::Color("always"),
    },
    QuickOptionItem {
        key: '9',
        section: "Mode couleur terminal",
        label: "never",
        hint: "Désactiver les couleurs",
        action: QuickOptionAction::Color("never"),
        state: QuickOptionState::Color("never"),
    },
    QuickOptionItem {
        key: 's',
        section: "Diagnostic et onboarding",
        label: "voir config",
        hint: "Afficher chemins et réglages effectifs",
        action: QuickOptionAction::ConfigShow,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'd',
        section: "Diagnostic et onboarding",
        label: "diagnostic config",
        hint: "Valider les fichiers de configuration",
        action: QuickOptionAction::ConfigDoctor,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'r',
        section: "Diagnostic et onboarding",
        label: "rafraîchir",
        hint: "Régénérer schémas et contextes agents",
        action: QuickOptionAction::Refresh,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'g',
        section: "Diagnostic et onboarding",
        label: "guide",
        hint: "Afficher le parcours de démarrage",
        action: QuickOptionAction::Guide,
        state: QuickOptionState::None,
    },
    QuickOptionItem {
        key: 'a',
        section: "Diagnostic et onboarding",
        label: "diagnostic agents",
        hint: "Diagnostiquer les agents installés",
        action: QuickOptionAction::AgentDoctor,
        state: QuickOptionState::None,
    },
];

pub fn quick_option_by_key(key: char) -> Option<QuickOptionAction> {
    QUICK_OPTIONS
        .iter()
        .find(|item| item.key == key)
        .map(|item| item.action)
}

pub fn quick_option_shortcut_hint() -> String {
    let mut numeric = QUICK_OPTIONS
        .iter()
        .map(|item| item.key)
        .filter(|key| key.is_ascii_digit())
        .collect::<Vec<_>>();
    numeric.sort_unstable();

    let numeric_hint = match (numeric.first(), numeric.last()) {
        (Some(first), Some(last)) if first == last => first.to_string(),
        (Some(first), Some(last)) => format!("{first}-{last}"),
        _ => String::new(),
    };

    let named = QUICK_OPTIONS
        .iter()
        .map(|item| item.key)
        .filter(|key| !key.is_ascii_digit())
        .map(|key| key.to_string())
        .collect::<Vec<_>>()
        .join("/");

    match (numeric_hint.is_empty(), named.is_empty()) {
        (false, false) => format!("{numeric_hint}/{named}"),
        (false, true) => numeric_hint,
        (true, false) => named,
        (true, true) => String::new(),
    }
}

pub fn option_action(root: &str, action: QuickOptionAction) -> TuiAction {
    match action {
        QuickOptionAction::Agent(agent) => TuiAction {
            label: format!("Agent par défaut · {agent}"),
            request: TuiActionRequest::AgentSetDefault {
                root: Some(root.into()),
                agent: agent.into(),
            },
            description: "Modifier l'agent par défaut".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::Color(mode) => TuiAction {
            label: format!("Couleur · {mode}"),
            request: TuiActionRequest::ConfigSetColor { mode: mode.into() },
            description: "Modifier le mode couleur terminal".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::ConfigShow => TuiAction {
            label: "Voir configuration".into(),
            request: TuiActionRequest::ConfigShow {
                root: Some(root.into()),
            },
            description: "Afficher les chemins et réglages effectifs".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::ConfigDoctor => TuiAction {
            label: "Diagnostiquer configuration".into(),
            request: TuiActionRequest::ConfigDoctor {
                root: Some(root.into()),
            },
            description: "Valider les fichiers de configuration".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::Refresh => TuiAction {
            label: "Rafraîchir DevWorkflow".into(),
            request: TuiActionRequest::Refresh(dw_config::command::RefreshCommandArgs {
                root: Some(root.into()),
                profile: "business".into(),
            }),
            description: "Régénérer schémas et contextes agents".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::Guide => TuiAction {
            label: "Guide".into(),
            request: TuiActionRequest::Guide,
            description: "Afficher le parcours de démarrage".into(),
            kind: ActionRisk::Safe,
        },
        QuickOptionAction::AgentDoctor => TuiAction {
            label: "Diagnostiquer agents".into(),
            request: TuiActionRequest::AgentDoctor { agent: None },
            description: "Diagnostiquer les agents installés".into(),
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
            TuiActionRequest::TaskStart(dw_task::start::StartArgs {
                work_item_id: Some(item.id.clone()),
                root: Some(snapshot.root.clone()),
                project: Some(project.key.clone()),
                task: None,
                type_name: None,
                only: None,
                slug: None,
                skip_ado: false,
                with_active_children: false,
                create_child_tasks: false,
                mode: dw_core::ExecutionMode::from_execute(action == AdoItemAction::StartExecute),
            })
        }
        AdoItemAction::Context => {
            TuiActionRequest::AdoContext(dw_ado_commands::commands::context::ContextArgs {
                id: item.id.clone(),
                root: Some(snapshot.root.clone()),
                project: Some(project.key.clone()),
                summary: false,
                comments: 200,
                mode: dw_ado_commands::commands::context::ContextMode::AiContext,
            })
        }
        AdoItemAction::WorkItem => {
            TuiActionRequest::AdoWorkItem(dw_ado_commands::commands::work_item::WorkItemArgs {
                id: item.id.clone(),
                root: Some(snapshot.root.clone()),
                project: Some(project.key.clone()),
            })
        }
        AdoItemAction::SetStartState => {
            let state = ado_start_state(snapshot, item)?;
            TuiActionRequest::AdoSetState(dw_ado_commands::commands::set_state::SetStateArgs {
                id: item.id.clone(),
                root: Some(snapshot.root.clone()),
                project: Some(project.key.clone()),
                state,
                history: None,
                yes: true,
            })
        }
    };

    Some(TuiAction {
        label: match action {
            AdoItemAction::SetStartState => format!("Passer à l’état de démarrage · #{}", item.id),
            _ => format!("Préparer work item · #{}", item.id),
        },
        request,
        description: format!("{} · {}", project.key, item.title),
        kind: match action {
            AdoItemAction::StartExecute | AdoItemAction::SetStartState => ActionRisk::Destructive,
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
            TuiActionRequest::TaskStartPr(dw_task::start::StartPrArgs {
                pull_request_id: pull_request_id.to_string(),
                root: Some(snapshot.root.clone()),
                project: item.project.clone(),
                repo: Some(item.repository.clone()),
                type_name: None,
                slug: None,
                mode: dw_core::ExecutionMode::from_execute(
                    action == PullRequestAction::StartExecute,
                ),
            })
        }
        PullRequestAction::FinishPreview | PullRequestAction::FinishExecute => {
            TuiActionRequest::TaskFinish(dw_task::finish::FinishArgs {
                workspace: Some(item.workspace.clone()?),
                r#continue: false,
                root: Some(snapshot.root.clone()),
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
            workspace: Some(item.workspace.clone()?),
            r#continue: false,
            root: Some(snapshot.root.clone()),
            mode: dw_core::ExecutionMode::Preview,
            message: None,
        }),
        PullRequestAction::Changelog => {
            TuiActionRequest::AdoChangelog(dw_ado_commands::commands::changelog::ChangelogArgs {
                ids: pull_request_id.to_string(),
                root: Some(snapshot.root.clone()),
                project: Some(item.project.clone()),
                from_pr: true,
                from_git: false,
                repo: Some(item.ado_repository.clone()),
                group_by_parent: false,
                format: None,
                table: false,
                ids_only: false,
                git_to: None,
            })
        }
    };

    Some(TuiAction {
        label: match action {
            PullRequestAction::StartPreview => {
                format!("Prévisualiser workspace PR · {}", item.repository)
            }
            PullRequestAction::StartExecute => format!("Créer workspace PR · {}", item.repository),
            PullRequestAction::FinishPreview => {
                format!("Prévisualiser finalisation PR · {}", item.repository)
            }
            PullRequestAction::FinishExecute => format!("Finaliser PR · {}", item.repository),
            PullRequestAction::DiffPreview => format!("Inspecter diff local · {}", item.repository),
            PullRequestAction::Changelog => format!("Résumer changements · {}", item.repository),
        },
        request,
        description: format!("{} · {}", item.project, item.branch),
        kind: match action {
            PullRequestAction::StartExecute | PullRequestAction::FinishExecute => {
                ActionRisk::Destructive
            }
            _ => ActionRisk::Safe,
        },
    })
}

pub fn selected_database_schema_action(
    snapshot: &TuiSnapshot,
    selected_database: usize,
) -> Option<TuiAction> {
    snapshot
        .database_entries
        .get(selected_database)
        .map(|database| database_action(database, DatabaseAction::Schema))
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
            DatabaseAction::Schema => "Inspecter le schéma read-only".into(),
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
        return "Aucune PR sélectionnée.".into();
    };
    if item.pull_request_id.is_none() {
        return "Cette ligne ne correspond pas à une PR exploitable.".into();
    }
    match action {
        PullRequestAction::FinishPreview
        | PullRequestAction::FinishExecute
        | PullRequestAction::DiffPreview
            if item.workspace.is_none() =>
        {
            "Action indisponible: aucun workspace local lié à cette PR. Appuyer sur x pour créer le workspace depuis la PR.".into()
        }
        _ => "Action PR indisponible pour cette ligne.".into(),
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
                assert_eq!(args.work_item_id.as_deref(), Some("42"));
                assert_eq!(args.root.as_deref(), Some("/tmp/dw"));
                assert_eq!(args.project.as_deref(), Some("ha"));
                assert!(args.mode.executes());
            }
            _ => panic!("expected task start request"),
        }
        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(!action.runs_attached_in_tui());
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
                assert_eq!(args.id, "42");
                assert_eq!(args.project.as_deref(), Some("ha"));
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
                assert_eq!(args.pull_request_id, "42");
                assert_eq!(args.repo.as_deref(), Some("front"));
                assert!(!args.mode.executes());
            }
            _ => panic!("expected PR workspace request"),
        }
    }

    #[test]
    fn pr_start_execute_runs_in_background_after_tui_confirmation() {
        let mut snapshot = snapshot();
        snapshot.pull_requests = vec![pull_request(None)];

        let action = selected_pull_request_action(&snapshot, 0, PullRequestAction::StartExecute)
            .expect("action");

        match &action.request {
            TuiActionRequest::TaskStartPr(args) => {
                assert_eq!(args.pull_request_id, "42");
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
                assert_eq!(args.workspace.as_deref(), Some("/tmp/ws"));
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
            "Action indisponible: aucun workspace local lié à cette PR. Appuyer sur x pour créer le workspace depuis la PR."
        );
    }

    #[test]
    fn option_actions_build_rooted_config_actions() {
        let show = option_action("/tmp/dw", QuickOptionAction::ConfigShow);
        let doctor = option_action("/tmp/dw", QuickOptionAction::ConfigDoctor);
        let refresh = option_action("/tmp/dw", QuickOptionAction::Refresh);

        assert!(matches!(
            show.request,
            TuiActionRequest::ConfigShow { root: Some(ref root) } if root == "/tmp/dw"
        ));
        assert!(matches!(
            doctor.request,
            TuiActionRequest::ConfigDoctor { root: Some(ref root) } if root == "/tmp/dw"
        ));
        assert!(matches!(
            refresh.request,
            TuiActionRequest::Refresh(ref args) if args.root.as_deref() == Some("/tmp/dw")
        ));
    }

    #[test]
    fn option_actions_build_preferences_actions() {
        let agent = option_action("/tmp/dw", QuickOptionAction::Agent("codex"));
        let color = option_action("/tmp/dw", QuickOptionAction::Color("always"));

        assert!(matches!(
            agent.request,
            TuiActionRequest::AgentSetDefault { root: Some(ref root), ref agent }
                if root == "/tmp/dw" && agent == "codex"
        ));
        assert!(matches!(
            color.request,
            TuiActionRequest::ConfigSetColor { ref mode } if mode == "always"
        ));
    }

    #[test]
    fn quick_option_catalog_exposes_shortcuts_and_actions() {
        assert_eq!(QUICK_OPTIONS[0].key, '1');
        assert_eq!(
            quick_option_by_key('4'),
            Some(QuickOptionAction::Agent("codex"))
        );
        assert_eq!(
            quick_option_by_key('5'),
            Some(QuickOptionAction::Agent("codex-cli"))
        );
        assert_eq!(
            quick_option_by_key('d'),
            Some(QuickOptionAction::ConfigDoctor)
        );
        assert!(quick_option_by_key('x').is_none());
    }

    #[test]
    fn quick_option_shortcut_hint_tracks_catalog_keys() {
        assert_eq!(quick_option_shortcut_hint(), "1-9/s/d/r/g/a");
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

        assert_eq!(agents, dw_config::AGENT_DEFAULT_CHOICES);
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
            TuiActionRequest::AgentOpen(ref args) if args.workspace.as_deref() == Some("/tmp/ws")
        ));
        assert!(matches!(open.kind, ActionRisk::OpensExternal));
        assert_eq!(preflight.workspace_path(), Some("/tmp/ws"));
        assert_eq!(latest.workspace_path(), Some("/tmp/ws"));
        assert_eq!(handoff.workspace_path(), Some("/tmp/ws"));
        assert_eq!(finish.workspace_path(), Some("/tmp/ws"));
        match &teardown.request {
            TuiActionRequest::TaskTeardown(args) => {
                assert_eq!(args.workspace.as_deref(), Some("/tmp/ws"));
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
                assert_eq!(args.workspace.as_deref(), Some("/tmp/ws"));
                assert_eq!(args.root.as_deref(), Some("/tmp/dw"));
            }
            _ => panic!("expected sync request"),
        }
    }

    fn snapshot() -> TuiSnapshot {
        TuiSnapshot {
            root: "/tmp/dw".into(),
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
