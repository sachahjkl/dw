use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::Duration;

use crate::actions::{
    self, AdoItemAction, DatabaseAction, PullRequestAction, QUICK_OPTIONS, QuickOptionAction,
};
use crate::background::{ActionStart, BackgroundJobs, BackgroundKind, BackgroundResult};
use crate::form::{FieldKind, FormMode, FormState, FormTemplate};
use crate::history::{HistoryState, RunHistoryEntry, output_lines, output_preview};
use crate::model::{
    ActionEffect, ActionRisk, AdoAssignedProject, CockpitItem, CockpitSeverity, DetailPanel,
    TuiAction, TuiActionRequest, TuiPullRequest, TuiSnapshot, View, WorkspaceAction,
};
use crate::ui_text::guide_detail_lines;
use crate::{runner, ui};

pub fn run_tui(root: Option<String>) -> Result<()> {
    runner::install_terminal()?;
    let result = run_tui_inner(root);
    runner::restore_terminal()?;
    result
}

fn run_tui_inner(root: Option<String>) -> Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(root);

    while !app.should_quit {
        app.poll_background_loads();
        terminal.draw(|frame| ui::render(frame, &app))?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key, &mut terminal)?;
        }
    }

    Ok(())
}

pub struct App {
    pub snapshot: TuiSnapshot,
    pub root_override: Option<String>,
    pub view: View,
    pub selected_action: usize,
    pub selected_cockpit: usize,
    pub selected_workspace: usize,
    pub selected_ado_project: usize,
    pub selected_ado_item: usize,
    pub selected_pull_request: usize,
    pub selected_database: usize,
    pub selected_option: usize,
    pub filter: String,
    pub filter_active: bool,
    pub confirmation: Option<TuiAction>,
    pub form: Option<FormState>,
    pub action_form: FormState,
    pub options_open: bool,
    pub state_open: bool,
    pub state_scroll: usize,
    pub detail: Option<DetailPanel>,
    pub messages: Vec<String>,
    pub history: HistoryState,
    pub should_quit: bool,
    reload_assigned_after_snapshot: bool,
    reload_pull_requests_after_snapshot: bool,
    reload_after_action_queue: bool,
    background: BackgroundJobs,
}

impl App {
    pub fn new(root: Option<String>) -> Self {
        let snapshot = TuiSnapshot::loading(root.as_deref());
        let mut background = BackgroundJobs::new();
        let _ = background.start_snapshot(root.clone());
        Self::from_snapshot(
            root,
            snapshot,
            background,
            vec![
                "TUI prêt. Entrée lance l'action sélectionnée.".into(),
                "Chargement du snapshot en arrière-plan...".into(),
            ],
        )
    }

    #[cfg(test)]
    pub(crate) fn new_ready(root: Option<String>) -> Self {
        let snapshot = TuiSnapshot::load(root.as_deref());
        Self::from_snapshot(
            root,
            snapshot,
            BackgroundJobs::new(),
            vec!["TUI prêt. Entrée lance l'action sélectionnée.".into()],
        )
    }

    fn from_snapshot(
        root_override: Option<String>,
        snapshot: TuiSnapshot,
        background: BackgroundJobs,
        messages: Vec<String>,
    ) -> Self {
        Self {
            snapshot,
            root_override,
            view: View::Dashboard,
            selected_action: 0,
            selected_cockpit: 0,
            selected_workspace: 0,
            selected_ado_project: 0,
            selected_ado_item: 0,
            selected_pull_request: 0,
            selected_database: 0,
            selected_option: 0,
            filter: String::new(),
            filter_active: false,
            confirmation: None,
            form: None,
            action_form: FormState::selecting(),
            options_open: false,
            state_open: false,
            state_scroll: 0,
            detail: None,
            messages,
            history: HistoryState::default(),
            should_quit: false,
            reload_assigned_after_snapshot: false,
            reload_pull_requests_after_snapshot: false,
            reload_after_action_queue: false,
            background,
        }
    }

    pub fn poll_background_loads(&mut self) {
        for result in self.background.poll() {
            match result {
                BackgroundResult::Snapshot {
                    generation,
                    snapshot,
                } => {
                    if !self.background.accept_snapshot(generation) {
                        continue;
                    }
                    self.accept_snapshot_reload(*snapshot);
                }
                BackgroundResult::Assigned { generation, items } => {
                    if !self.background.accept_assigned(generation) {
                        continue;
                    }
                    self.snapshot.assigned = items;
                    self.snapshot.assigned_loaded = true;
                    self.clamp_ado_item_selection();
                    self.messages
                        .push(assigned_load_summary(&self.snapshot.assigned));
                }
                BackgroundResult::PullRequests { generation, items } => {
                    if !self.background.accept_pull_requests(generation) {
                        continue;
                    }
                    self.snapshot.pull_requests = items;
                    self.snapshot.pull_requests_loaded = true;
                    self.clamp_pull_request_selection();
                    self.messages
                        .push(pull_request_load_summary(&self.snapshot.pull_requests));
                }
                BackgroundResult::ActionOutput {
                    generation,
                    label,
                    line,
                } => {
                    if self.background.accepts_action_output(generation) {
                        self.history.append_running_line(&label, line);
                    }
                }
                BackgroundResult::Action {
                    generation,
                    label,
                    refresh_after_success,
                    open_after_success,
                    effect,
                    result,
                } => {
                    if !self.background.accept_action(generation) {
                        continue;
                    }
                    self.accept_action_result(
                        label,
                        refresh_after_success,
                        open_after_success,
                        effect,
                        result,
                    );
                }
            }
        }
    }

    pub fn assigned_loading(&self) -> bool {
        self.background.is_loading(BackgroundKind::Assigned)
    }

    pub fn pull_requests_loading(&self) -> bool {
        self.background.is_loading(BackgroundKind::PullRequests)
    }

    pub fn action_loading(&self) -> bool {
        self.background.is_loading(BackgroundKind::Action)
    }

    pub fn snapshot_loading(&self) -> bool {
        self.background.is_loading(BackgroundKind::Snapshot)
    }

    pub fn running_action_label(&self) -> Option<&str> {
        self.background.action_label()
    }

    pub fn pending_action_count(&self) -> usize {
        self.background.pending_action_count()
    }

    pub fn pending_action_labels(&self) -> Vec<String> {
        self.background.pending_action_labels()
    }

    pub fn loading_elapsed_label(&self, kind: BackgroundKind) -> Option<String> {
        self.background.elapsed_label(kind)
    }

    pub fn background_status_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(self.background_status_line(
            BackgroundKind::Snapshot,
            "Snapshot",
            "prêt".into(),
        ));
        lines.push(self.background_status_line(
            BackgroundKind::Assigned,
            "Mes work items",
            if self.snapshot.assigned_loaded {
                format!("{} items", self.snapshot.assigned_count())
            } else {
                "non chargé".into()
            },
        ));
        lines.push(self.background_status_line(
            BackgroundKind::PullRequests,
            "PRs",
            if self.snapshot.pull_requests_loaded {
                format!(
                    "{} actives",
                    self.snapshot
                        .pull_requests
                        .iter()
                        .filter(|item| item.pull_request_id.is_some())
                        .count()
                )
            } else {
                "non chargées".into()
            },
        ));
        lines.push(self.action_status_line());
        lines.extend(self.action_queue_status_lines());
        lines
    }

    pub fn action_queue_status_lines(&self) -> Vec<String> {
        let pending = self.pending_action_labels();
        let Some(first) = pending.first() else {
            return Vec::new();
        };
        let mut lines = vec![format!("À suivre: {first}")];
        if pending.len() > 1 {
            lines.push(format!("Puis: {} autre(s) action(s)", pending.len() - 1));
        }
        lines
    }

    fn background_status_line(
        &self,
        kind: BackgroundKind,
        label: &'static str,
        idle: String,
    ) -> String {
        if self.background.is_loading(kind) {
            let elapsed = self
                .background
                .elapsed_label(kind)
                .unwrap_or_else(|| "<1s".into());
            format!("{label}: chargement {elapsed}")
        } else {
            format!("{label}: {idle}")
        }
    }

    fn action_status_line(&self) -> String {
        if let Some(label) = self.background.action_label() {
            let elapsed = self
                .background
                .elapsed_label(BackgroundKind::Action)
                .unwrap_or_else(|| "<1s".into());
            let queued = self.background.pending_action_count();
            if queued > 0 {
                format!("Action: {label} ({elapsed}, file {queued})")
            } else {
                format!("Action: {label} ({elapsed})")
            }
        } else {
            let queued = self.background.pending_action_count();
            if queued > 0 {
                format!("Action: file {queued}")
            } else {
                "Action: aucune".into()
            }
        }
    }

    pub fn visible_actions(&self) -> Vec<(usize, &TuiAction)> {
        let filter = self.filter.trim().to_lowercase();
        self.snapshot
            .actions
            .iter()
            .enumerate()
            .filter(|(_, action)| self.action_matches_current_view(action))
            .filter(|(_, action)| {
                let display_label = action.display_label().to_lowercase();
                filter.is_empty()
                    || display_label.contains(&filter)
                    || action.description.to_lowercase().contains(&filter)
            })
            .collect()
    }

    fn action_matches_current_view(&self, action: &TuiAction) -> bool {
        if self.view != View::Workspaces {
            return action_matches_view(action, self.view);
        }

        let Some(workspace) = self.snapshot.workspaces.get(self.selected_workspace) else {
            return action_matches_view(action, self.view);
        };

        action.is_workspace_action()
            && action
                .workspace_path()
                .is_none_or(|path| path == workspace.path)
    }

    pub fn selected_visible_action(&self) -> Option<(usize, &TuiAction)> {
        let actions = self.visible_actions();
        actions
            .get(self.selected_action.min(actions.len().saturating_sub(1)))
            .copied()
    }

    pub fn cockpit_items(&self) -> Vec<CockpitItem> {
        let mut items = Vec::new();
        if !self.snapshot.config_doctor.passed {
            items.push(CockpitItem {
                section: "Attention",
                title: "Configuration à corriger".into(),
                subtitle: self.snapshot.config_doctor.root.clone(),
                status: "doctor KO".into(),
                severity: CockpitSeverity::Blocked,
                primary_action: actions::option_action(
                    &self.snapshot.root,
                    QuickOptionAction::ConfigDoctor,
                ),
            });
        }
        for project in &self.snapshot.assigned {
            if let Some(error) = project.error.as_ref() {
                items.push(CockpitItem {
                    section: "Attention",
                    title: format!("Mes work items indisponibles · {}", project.key),
                    subtitle: error.clone(),
                    status: "erreur".into(),
                    severity: CockpitSeverity::Attention,
                    primary_action: actions::option_action(
                        &self.snapshot.root,
                        QuickOptionAction::ConfigDoctor,
                    ),
                });
            }
        }
        for pr in self
            .snapshot
            .pull_requests
            .iter()
            .enumerate()
            .filter(|(_, pr)| pr.pull_request_id.is_some() && pr.workspace.is_none())
        {
            if let Some(action) = actions::selected_pull_request_action(
                &self.snapshot,
                pr.0,
                PullRequestAction::StartPreview,
            ) {
                let pr_item = pr.1;
                items.push(CockpitItem {
                    section: "À traiter",
                    title: format!(
                        "Créer workspace PR #{}",
                        pr_item.pull_request_id.unwrap_or_default()
                    ),
                    subtitle: format!(
                        "{} / {} · {}",
                        pr_item.project, pr_item.repository, pr_item.branch
                    ),
                    status: "PR sans workspace".into(),
                    severity: CockpitSeverity::Attention,
                    primary_action: action,
                });
            }
        }
        for pr in self
            .snapshot
            .pull_requests
            .iter()
            .enumerate()
            .filter(|(_, pr)| pr.pull_request_id.is_some() && pr.workspace.is_some())
        {
            if let Some(action) = actions::selected_pull_request_action(
                &self.snapshot,
                pr.0,
                PullRequestAction::FinishPreview,
            ) {
                let pr_item = pr.1;
                items.push(CockpitItem {
                    section: "En cours",
                    title: format!(
                        "Finaliser PR #{}",
                        pr_item.pull_request_id.unwrap_or_default()
                    ),
                    subtitle: pr_item.workspace.clone().unwrap_or_default(),
                    status: "workspace lié".into(),
                    severity: CockpitSeverity::Normal,
                    primary_action: action,
                });
            }
        }
        for (index, workspace) in self.snapshot.workspaces.iter().take(8).enumerate() {
            if let Some(action) = actions::selected_workspace_action(
                &self.snapshot,
                index,
                WorkspaceAction::Preflight,
            ) {
                items.push(CockpitItem {
                    section: "En cours",
                    title: format!("Préflight {}", workspace.display_work_items),
                    subtitle: workspace.path.clone(),
                    status: workspace.kind.clone(),
                    severity: CockpitSeverity::Normal,
                    primary_action: action,
                });
            }
        }
        for (project_index, project) in self.snapshot.assigned.iter().enumerate() {
            for (item_index, item) in project.items.iter().take(5).enumerate() {
                if let Some(action) = actions::selected_ado_action(
                    &self.snapshot,
                    project_index,
                    item_index,
                    AdoItemAction::StartPreview,
                ) {
                    items.push(CockpitItem {
                        section: "À traiter",
                        title: format!("Démarrer #{} · {}", item.id, item.title),
                        subtitle: format!("{} · {} · {}", project.key, item.kind, item.state),
                        status: "assigned".into(),
                        severity: CockpitSeverity::Normal,
                        primary_action: action,
                    });
                }
            }
        }
        if self.snapshot.prune_candidates > 0 {
            items.push(CockpitItem {
                section: "Attention",
                title: format!(
                    "{} workspace(s) éligibles prune",
                    self.snapshot.prune_candidates
                ),
                subtitle: self.snapshot.root.clone(),
                status: "preview".into(),
                severity: CockpitSeverity::Attention,
                primary_action: TuiAction {
                    label: "Prune preview".into(),
                    request: TuiActionRequest::TaskPrune(dw_task::prune::PruneArgs {
                        root: Some(self.snapshot.root.clone()),
                        project: None,
                        work_item: None,
                        mode: dw_core::ExecutionMode::Preview,
                        yes: false,
                        no_sync: true,
                    }),
                    description: "Prévisualiser les workspaces à nettoyer".into(),
                    kind: ActionRisk::DryRun,
                },
            });
        }
        if items.is_empty() {
            items.push(CockpitItem {
                section: "OK",
                title: "Aucune action urgente".into(),
                subtitle: "Utiliser les onglets métier ou le constructeur avancé.".into(),
                status: "idle".into(),
                severity: CockpitSeverity::Normal,
                primary_action: actions::option_action(
                    &self.snapshot.root,
                    QuickOptionAction::Guide,
                ),
            });
        }
        items
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.form.is_some() {
            return self.handle_form_key(key, terminal);
        }

        if self.history.output_open {
            return self.handle_history_output_key(key);
        }

        if self.state_open {
            return self.handle_state_key(key);
        }

        if self.detail.is_some() {
            return self.handle_detail_key(key);
        }

        if self.options_open {
            return self.handle_options_key(key, terminal);
        }

        if self.filter_active {
            return self.handle_filter_key(key);
        }

        if self.confirmation.is_some() {
            return self.handle_confirmation_key(key, terminal);
        }

        if self.view == View::Composer && self.handle_action_builder_key(key, terminal)? {
            return Ok(());
        }

        if self.handle_view_navigation_key(key) {
            return Ok(());
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab | KeyCode::Right => self.next_view(),
            KeyCode::BackTab | KeyCode::Left => self.previous_view(),
            KeyCode::Char('/') => {
                self.filter_active = true;
                self.confirmation = None;
            }
            KeyCode::Char('n') if !matches!(self.view, View::Ado | View::PullRequests) => {
                self.open_form()
            }
            KeyCode::Char('o') if self.view != View::Workspaces => self.open_options(),
            KeyCode::Char('O') => self.open_options(),
            KeyCode::Char('h') => self.open_history_output(),
            KeyCode::Char('i') => self.open_state_modal(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('1') => self.set_view(View::Dashboard),
            KeyCode::Char('2') => self.set_view(View::Workspaces),
            KeyCode::Char('3') => self.set_view(View::Ado),
            KeyCode::Char('4') => self.set_view(View::PullRequests),
            KeyCode::Char('5') => self.set_view(View::Db),
            KeyCode::Char('6') => self.set_view(View::Config),
            KeyCode::Char('7') => self.set_view(View::Composer),
            KeyCode::Char('?') => self.set_view(View::Help),
            _ => {}
        }
        if self.handle_view_action_key(key, terminal)? {
            return Ok(());
        }
        if key.code == KeyCode::Enter {
            if self.view == View::Dashboard {
                self.request_or_run_selected_cockpit_item(terminal)?;
            } else {
                self.request_or_run_selected_action(terminal)?;
            }
        }
        Ok(())
    }

    fn handle_view_navigation_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if self.view == View::Dashboard => {
                self.move_cockpit_up()
            }
            KeyCode::Down | KeyCode::Char('j') if self.view == View::Dashboard => {
                self.move_cockpit_down()
            }
            KeyCode::Up | KeyCode::Char('k') if self.view == View::Ado => self.move_ado_item_up(),
            KeyCode::Down | KeyCode::Char('j') if self.view == View::Ado => {
                self.move_ado_item_down()
            }
            KeyCode::Up | KeyCode::Char('k') if self.view == View::PullRequests => {
                self.move_pull_request_up()
            }
            KeyCode::Down | KeyCode::Char('j') if self.view == View::PullRequests => {
                self.move_pull_request_down()
            }
            KeyCode::Up | KeyCode::Char('k') if self.view == View::Db => self.move_database_up(),
            KeyCode::Down | KeyCode::Char('j') if self.view == View::Db => {
                self.move_database_down()
            }
            KeyCode::Up | KeyCode::Char('k') if self.view == View::Composer => {
                self.move_action_form_up()
            }
            KeyCode::Down | KeyCode::Char('j') if self.view == View::Composer => {
                self.move_action_form_down()
            }
            KeyCode::BackTab if self.view == View::Composer => self.move_action_form_up(),
            KeyCode::Tab if self.view == View::Composer => self.move_action_form_down(),
            KeyCode::Up | KeyCode::Char('k') => self.move_action_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_action_down(),
            KeyCode::Char('K') if self.view == View::Ado => self.move_ado_project_up(),
            KeyCode::Char('J') if self.view == View::Ado => self.move_ado_project_down(),
            KeyCode::Char('K') => self.move_workspace_up(),
            KeyCode::Char('J') => self.move_workspace_down(),
            KeyCode::Char('[') if self.view == View::Ado => self.move_ado_project_up(),
            KeyCode::Char(']') if self.view == View::Ado => self.move_ado_project_down(),
            _ => return false,
        }
        true
    }

    fn handle_view_action_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<bool> {
        match (self.view, key.code) {
            (View::Ado, KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('s')) => {
                self.request_or_run_ado_action(AdoItemAction::StartPreview, terminal)?
            }
            (View::Ado, KeyCode::Char('x')) => {
                self.request_or_run_ado_action(AdoItemAction::StartExecute, terminal)?
            }
            (View::Ado, KeyCode::Char('c')) => {
                self.request_or_run_ado_action(AdoItemAction::Context, terminal)?
            }
            (View::Ado, KeyCode::Char('w')) => {
                self.request_or_run_ado_action(AdoItemAction::WorkItem, terminal)?
            }
            (View::Ado, KeyCode::Char('e')) => {
                self.request_or_run_ado_action(AdoItemAction::SetStartState, terminal)?
            }
            (View::Ado, KeyCode::Char('E')) => self.open_ado_set_state_form(),
            (View::Ado, KeyCode::Char('u')) => self.open_selected_ado_url(),
            (View::Workspaces, KeyCode::Enter | KeyCode::Char('o')) => {
                self.request_or_run_workspace_action(WorkspaceAction::Open, terminal)?
            }
            (View::Workspaces, KeyCode::Char('p')) => {
                self.request_or_run_workspace_action(WorkspaceAction::Preflight, terminal)?
            }
            (View::Workspaces, KeyCode::Char('s')) => {
                self.request_or_run_workspace_action(WorkspaceAction::Sync, terminal)?
            }
            (View::Workspaces, KeyCode::Char('l')) => {
                self.request_or_run_workspace_action(WorkspaceAction::RepoLatest, terminal)?
            }
            (View::Workspaces, KeyCode::Char('v')) => {
                self.request_or_run_workspace_action(WorkspaceAction::HandoffValidate, terminal)?
            }
            (View::Workspaces, KeyCode::Char('c')) => {
                self.request_or_run_workspace_action(WorkspaceAction::CommitPreview, terminal)?
            }
            (View::Workspaces, KeyCode::Char('f')) => {
                self.request_or_run_workspace_action(WorkspaceAction::FinishPreview, terminal)?
            }
            (View::Workspaces, KeyCode::Char('F')) => {
                self.request_or_run_workspace_action(WorkspaceAction::FinishExecute, terminal)?
            }
            (View::Workspaces, KeyCode::Char('t')) => {
                self.request_or_run_workspace_action(WorkspaceAction::TeardownPreview, terminal)?
            }
            (View::Workspaces, KeyCode::Char('x')) => {
                self.request_or_run_workspace_action(WorkspaceAction::TeardownExecute, terminal)?
            }
            (View::PullRequests, KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('s')) => {
                self.request_or_run_pull_request_action(PullRequestAction::StartPreview, terminal)?
            }
            (View::PullRequests, KeyCode::Char('x')) => {
                self.request_or_run_pull_request_action(PullRequestAction::StartExecute, terminal)?
            }
            (View::PullRequests, KeyCode::Char('f')) => {
                self.request_or_run_pull_request_action(PullRequestAction::FinishPreview, terminal)?
            }
            (View::PullRequests, KeyCode::Char('F')) => {
                self.request_or_run_pull_request_action(PullRequestAction::FinishExecute, terminal)?
            }
            (View::PullRequests, KeyCode::Char('c')) => {
                self.request_or_run_pull_request_action(PullRequestAction::Changelog, terminal)?
            }
            (View::PullRequests, KeyCode::Char('d')) => {
                self.request_or_run_pull_request_action(PullRequestAction::DiffPreview, terminal)?
            }
            (View::PullRequests, KeyCode::Char('N')) => self.open_start_pr_form(),
            (View::PullRequests, KeyCode::Char('u')) => self.open_selected_pull_request_url(),
            (View::Db, KeyCode::Enter | KeyCode::Char('s')) => {
                self.request_or_run_db_action(DatabaseAction::Schema, terminal)?
            }
            (View::Db, KeyCode::Char('d')) => self.open_db_describe_form(),
            (View::Db, KeyCode::Char('e')) => self.open_db_query_form(),
            (View::Config, KeyCode::Char('s')) => {
                self.request_or_run_quick_option(QuickOptionAction::ConfigShow, terminal)?
            }
            (View::Config, KeyCode::Char('d')) => {
                self.request_or_run_quick_option(QuickOptionAction::ConfigDoctor, terminal)?
            }
            (View::Config, KeyCode::Char('f')) => {
                self.request_or_run_quick_option(QuickOptionAction::Refresh, terminal)?
            }
            (View::Config, KeyCode::Char('g')) => {
                self.request_or_run_quick_option(QuickOptionAction::Guide, terminal)?
            }
            (View::Config, KeyCode::Char('a')) => {
                self.request_or_run_quick_option(QuickOptionAction::AgentDoctor, terminal)?
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn handle_form_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(form) = self.form.as_mut() else {
            return Ok(());
        };

        match form.mode {
            FormMode::Selecting => match key.code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Esc => {
                    self.form = None;
                    self.messages.push("Formulaire annulé.".into());
                }
                KeyCode::Up | KeyCode::Char('k') => form.move_template_up(),
                KeyCode::Down | KeyCode::Char('j') => form.move_template_down(),
                KeyCode::Enter => form.begin_editing(&self.snapshot),
                _ => {}
            },
            FormMode::Editing => match key.code {
                KeyCode::Esc => {
                    self.form = None;
                    self.messages.push("Formulaire annulé.".into());
                }
                KeyCode::Up | KeyCode::BackTab => form.move_field_up(),
                KeyCode::Down | KeyCode::Tab => form.move_field_down(),
                KeyCode::Backspace => form.backspace(),
                KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    match form.apply_suggestion(&self.snapshot) {
                        Some(value) => self.messages.push(format!("Suggestion appliquée: {value}")),
                        None => self
                            .messages
                            .push("Aucune suggestion disponible pour ce champ.".into()),
                    }
                }
                KeyCode::Char(' ') => form.toggle_selected(),
                KeyCode::Enter => {
                    let action = form.build_action(&self.snapshot.root);
                    self.form = None;
                    match action {
                        Some(action) => self.request_or_run_action(action, terminal)?,
                        None => self
                            .messages
                            .push("Formulaire incomplet: action non générée.".into()),
                    }
                }
                KeyCode::Char(value)
                    if form
                        .fields
                        .get(form.selected_field)
                        .is_some_and(|field| field.kind == FieldKind::Text) =>
                {
                    form.push_char(value);
                }
                _ => {}
            },
        }

        Ok(())
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.filter_active = false;
                self.filter.clear();
                self.selected_action = 0;
            }
            KeyCode::Enter => {
                self.filter_active = false;
                self.clamp_action_selection();
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.selected_action = 0;
            }
            KeyCode::Char(value) => {
                self.filter.push(value);
                self.selected_action = 0;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_history_output_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') => {
                self.history.close_output();
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.history.scroll_output_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.history.scroll_output_down();
            }
            KeyCode::Char('[') => self.history.select_previous_entry(),
            KeyCode::Char(']') => self.history.select_next_entry(),
            KeyCode::Home => self.history.scroll_output_home(),
            KeyCode::End => self.history.scroll_output_end(),
            _ => {}
        }
        Ok(())
    }

    fn handle_state_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('q') => {
                self.state_open = false;
                self.state_scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state_scroll = self.state_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state_scroll = self.state_scroll.saturating_add(1);
            }
            KeyCode::Home => self.state_scroll = 0,
            KeyCode::End => self.state_scroll = usize::MAX,
            _ => {}
        }
        Ok(())
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(detail) = self.detail.as_mut() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                self.detail = None;
            }
            KeyCode::Up | KeyCode::Char('k') => detail.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => detail.scroll_down(),
            KeyCode::Home => detail.scroll_home(),
            KeyCode::End => detail.scroll_end(),
            _ => {}
        }
        Ok(())
    }

    fn handle_options_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc | KeyCode::Char('o') => {
                self.options_open = false;
                self.messages.push("Options fermées.".into());
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_option_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_option_down(),
            KeyCode::Enter => self.run_selected_option_action(terminal)?,
            KeyCode::Char(key) => {
                if let Some(action) = actions::quick_option_by_key(key) {
                    self.run_option_action(action, terminal)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_confirmation_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Some(action) = self.confirmation.take() {
                    self.run_action(action, terminal)?;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirmation = None;
                self.messages.push("Action annulée.".into());
            }
            _ => {}
        }
        Ok(())
    }

    fn request_or_run_selected_action(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some((_, action)) = self.selected_visible_action() else {
            self.messages.push("Aucune action sélectionnée.".into());
            return Ok(());
        };

        self.request_or_run_action(action.clone(), terminal)
    }

    fn request_or_run_selected_cockpit_item(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let items = self.cockpit_items();
        let Some(item) = items.get(self.selected_cockpit.min(items.len().saturating_sub(1))) else {
            self.messages.push("Aucun item cockpit sélectionné.".into());
            return Ok(());
        };
        self.request_or_run_action(item.primary_action.clone(), terminal)
    }

    fn handle_action_builder_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<bool> {
        match self.action_form.mode {
            FormMode::Selecting => match key.code {
                KeyCode::Enter => {
                    self.action_form.begin_editing(&self.snapshot);
                    Ok(true)
                }
                _ => Ok(false),
            },
            FormMode::Editing => match key.code {
                KeyCode::Esc => {
                    self.action_form = FormState::selecting();
                    Ok(true)
                }
                KeyCode::Backspace => {
                    self.action_form.backspace();
                    Ok(true)
                }
                KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    match self.action_form.apply_suggestion(&self.snapshot) {
                        Some(value) => self.messages.push(format!("Suggestion appliquée: {value}")),
                        None => self
                            .messages
                            .push("Aucune suggestion disponible pour ce champ.".into()),
                    }
                    Ok(true)
                }
                KeyCode::Char(' ') => {
                    self.action_form.toggle_selected();
                    Ok(true)
                }
                KeyCode::Enter => {
                    let action = self.action_form.build_action(&self.snapshot.root);
                    match action {
                        Some(action) => self.request_or_run_action(action, terminal)?,
                        None => self
                            .messages
                            .push("Constructeur incomplet: action non générée.".into()),
                    }
                    Ok(true)
                }
                KeyCode::Char(value)
                    if self
                        .action_form
                        .fields
                        .get(self.action_form.selected_field)
                        .is_some_and(|field| field.kind == FieldKind::Text) =>
                {
                    self.action_form.push_char(value);
                    Ok(true)
                }
                _ => Ok(false),
            },
        }
    }

    fn request_or_run_action(
        &mut self,
        action: TuiAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.run_inline_detail_action(&action) {
            return Ok(());
        }

        let requires_confirmation = matches!(
            action.kind,
            ActionRisk::Destructive | ActionRisk::OpensExternal
        );
        let display_label = action.display_label();
        if requires_confirmation {
            self.confirmation = Some(action);
            self.messages
                .push(format!("Confirmation requise: {display_label}"));
            return Ok(());
        }

        self.run_action(action, terminal)
    }

    fn run_inline_detail_action(&mut self, action: &TuiAction) -> bool {
        match &action.request {
            TuiActionRequest::Guide => {
                self.detail = Some(DetailPanel::guide(guide_detail_lines()));
                self.messages.push("Guide affiché.".into());
                true
            }
            TuiActionRequest::ConfigShow { root } => {
                let report = dw_config::config_show(root.as_deref());
                self.detail = Some(DetailPanel::config_show(&report));
                self.messages
                    .push("Configuration affichée depuis le core.".into());
                true
            }
            TuiActionRequest::ConfigDoctor { root } => {
                let report = dw_config::config_doctor(root.as_deref());
                self.snapshot.config_doctor = report.clone();
                self.detail = Some(DetailPanel::config_doctor(&report));
                self.messages
                    .push("Diagnostic configuration exécuté depuis le core.".into());
                true
            }
            TuiActionRequest::AgentDoctor { agent } => {
                match dw_agent::command::agent_doctor(agent.as_deref()) {
                    Ok(report) => {
                        self.detail = Some(DetailPanel::agent_doctor(&report));
                        self.messages
                            .push("Diagnostic agents exécuté depuis le core.".into());
                    }
                    Err(error) => {
                        self.messages
                            .push(format!("Diagnostic agents impossible: {error}"));
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn run_option_action(
        &mut self,
        option: QuickOptionAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        self.options_open = false;
        if self.run_core_quick_option(option) {
            return Ok(());
        }
        self.request_or_run_action(
            actions::option_action(&self.snapshot.root, option),
            terminal,
        )
    }

    fn request_or_run_quick_option(
        &mut self,
        option: QuickOptionAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.run_core_quick_option(option) {
            return Ok(());
        }
        self.request_or_run_action(
            actions::option_action(&self.snapshot.root, option),
            terminal,
        )
    }

    fn run_core_quick_option(&mut self, option: QuickOptionAction) -> bool {
        self.run_inline_detail_action(&actions::option_action(&self.snapshot.root, option))
    }

    fn run_selected_option_action(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(option) = QUICK_OPTIONS.get(self.selected_option) else {
            self.messages.push("Aucune option sélectionnée.".into());
            return Ok(());
        };
        self.run_option_action(option.action, terminal)
    }

    fn request_or_run_ado_action(
        &mut self,
        action: AdoItemAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) = self.selected_ado_action(action) else {
            self.messages.push(self.selected_ado_action_error());
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal)
    }

    fn request_or_run_workspace_action(
        &mut self,
        action: WorkspaceAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) =
            actions::selected_workspace_action(&self.snapshot, self.selected_workspace, action)
        else {
            self.messages.push("Aucun workspace sélectionné.".into());
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal)
    }

    fn request_or_run_pull_request_action(
        &mut self,
        action: PullRequestAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) = self.selected_pull_request_action(action) else {
            self.messages
                .push(self.selected_pull_request_action_error(action));
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal)
    }

    fn open_selected_ado_url(&mut self) {
        let Some(project) = self.snapshot.assigned.get(self.selected_ado_project) else {
            self.messages.push("Aucun projet ADO sélectionné.".into());
            return;
        };
        let Some(item) = project.items.get(self.selected_ado_item) else {
            self.messages.push(self.selected_ado_action_error());
            return;
        };
        let Some(url) = item
            .url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(str::to_string)
        else {
            self.messages.push(format!(
                "Action indisponible: URL absente pour le work item #{}.",
                item.id
            ));
            return;
        };
        self.open_url("work item ADO", &url);
    }

    fn open_selected_pull_request_url(&mut self) {
        let Some(item) = self.snapshot.pull_requests.get(self.selected_pull_request) else {
            self.messages.push("Aucune PR sélectionnée.".into());
            return;
        };
        let Some(url) = item
            .url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(str::to_string)
        else {
            self.messages
                .push("Action indisponible: URL PR absente dans la réponse Azure DevOps.".into());
            return;
        };
        self.open_url("PR ADO", &url);
    }

    fn open_url(&mut self, label: &str, url: &str) {
        match webbrowser::open(url) {
            Ok(_) => self.messages.push(format!("{label} ouverte: {url}")),
            Err(error) => self
                .messages
                .push(format!("Impossible d'ouvrir {label}: {error}. URL: {url}")),
        }
    }

    pub fn selected_ado_action_preview(&self) -> Option<String> {
        actions::selected_ado_action(
            &self.snapshot,
            self.selected_ado_project,
            self.selected_ado_item,
            AdoItemAction::StartPreview,
        )
        .map(|action| action.display_label())
    }

    pub fn selected_ado_set_state_action_preview(&self) -> Option<String> {
        actions::selected_ado_action(
            &self.snapshot,
            self.selected_ado_project,
            self.selected_ado_item,
            AdoItemAction::SetStartState,
        )
        .map(|action| action.display_label())
    }

    fn selected_ado_action(&self, action: AdoItemAction) -> Option<TuiAction> {
        actions::selected_ado_action(
            &self.snapshot,
            self.selected_ado_project,
            self.selected_ado_item,
            action,
        )
    }

    fn selected_ado_action_error(&self) -> String {
        if self.assigned_loading() {
            "Mes work items chargent en arrière-plan; vous pouvez rester dans le TUI.".into()
        } else if !self.snapshot.assigned_loaded {
            "Mes work items pas encore chargés: ouvrir l'onglet ADO ou appuyer sur r pour recharger."
                .into()
        } else if self.snapshot.assigned.is_empty() {
            "Aucun projet ADO configuré ou exploitable pour vos work items.".into()
        } else {
            "Aucun work item ADO sélectionné.".into()
        }
    }

    pub fn selected_pull_request_action_preview(&self) -> Option<String> {
        self.selected_pull_request_action_preview_for(PullRequestAction::StartPreview)
    }

    pub fn selected_pull_request_action_preview_for(
        &self,
        action: PullRequestAction,
    ) -> Option<String> {
        actions::selected_pull_request_action(&self.snapshot, self.selected_pull_request, action)
            .map(|action| action.display_label())
    }

    pub fn selected_database_action_preview(&self) -> Option<String> {
        actions::selected_database_schema_action(&self.snapshot, self.selected_database)
            .map(|action| action.display_label())
    }

    pub fn selected_workspace_action_preview(&self) -> Option<String> {
        self.selected_workspace_action_preview_for(WorkspaceAction::Open)
    }

    pub fn selected_workspace_action_preview_for(&self, action: WorkspaceAction) -> Option<String> {
        actions::selected_workspace_action(&self.snapshot, self.selected_workspace, action)
            .map(|action| action.display_label())
    }

    fn request_or_run_db_action(
        &mut self,
        action: DatabaseAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) =
            actions::selected_database_action(&self.snapshot, self.selected_database, action)
        else {
            self.messages.push("Aucune base DB sélectionnée.".into());
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal)
    }

    fn selected_pull_request_action(&self, action: PullRequestAction) -> Option<TuiAction> {
        actions::selected_pull_request_action(&self.snapshot, self.selected_pull_request, action)
    }

    fn selected_pull_request_action_error(&self, action: PullRequestAction) -> String {
        if self.pull_requests_loading() {
            return "PRs en cours de chargement; vous pouvez continuer à naviguer.".into();
        }
        if !self.snapshot.pull_requests_loaded {
            return "PRs pas encore chargées: ouvrir l'onglet PRs ou appuyer sur r pour recharger."
                .into();
        }
        actions::pull_request_action_error(&self.snapshot, self.selected_pull_request, action)
    }

    fn run_action(
        &mut self,
        action: TuiAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if !action.runs_attached_in_tui() {
            match self.background.start_action(action) {
                ActionStart::Started { label } => {
                    self.history.start_running(label.clone());
                    self.messages
                        .push(format!("Lancement en arrière-plan: {label}"));
                }
                ActionStart::Queued { label, position } => {
                    self.messages
                        .push(format!("Action mise en file #{position}: {label}"));
                }
            }
            return Ok(());
        }

        terminal.show_cursor().ok();
        let result = runner::run_attached(&action)?;
        terminal.clear()?;
        self.history.push(RunHistoryEntry {
            request_label: result.display_label.clone(),
            status: result.status_label.clone(),
            success: result.success,
            output_preview: Vec::new(),
            output_lines: Vec::new(),
        });
        self.messages.push(format!(
            "Dernier lancement: {} -> {}",
            result.display_label, result.status_label
        ));
        if result.success && action.should_refresh_after_success() {
            self.apply_successful_action_effect(action.successful_effect());
            self.reload();
        }
        Ok(())
    }

    fn accept_action_result(
        &mut self,
        label: String,
        refresh_after_success: bool,
        open_after_success: bool,
        effect: Option<ActionEffect>,
        result: std::result::Result<runner::CapturedActionRunResult, String>,
    ) {
        match result {
            Ok(result) => {
                if !self.history.finish_running(
                    &result.display_label,
                    result.status_label.clone(),
                    result.success,
                    &result.output,
                ) {
                    let output_preview = output_preview(&result.output);
                    let output_lines = output_lines(&result.output);
                    self.history.push(RunHistoryEntry {
                        request_label: result.display_label.clone(),
                        status: result.status_label.clone(),
                        success: result.success,
                        output_preview,
                        output_lines,
                    });
                }
                self.messages.push(format!(
                    "Terminé: {} -> {}",
                    result.display_label, result.status_label
                ));
                if open_after_success {
                    self.detail = Some(DetailPanel::operation_result(
                        format!("Résultat · {}", result.display_label),
                        &result.output,
                    ));
                    self.history.output_open = false;
                }
                if result.success && refresh_after_success {
                    self.apply_successful_action_effect(effect);
                    self.reload_after_action_queue = true;
                }
                self.continue_action_queue();
            }
            Err(error) => {
                if !self
                    .history
                    .finish_running(&label, "erreur".into(), false, &error)
                {
                    self.history.push(RunHistoryEntry {
                        request_label: label.clone(),
                        status: "erreur".into(),
                        success: false,
                        output_preview: vec![error.clone()],
                        output_lines: vec![error.clone()],
                    });
                }
                self.messages.push(format!("Échec: {label} -> {error}"));
                self.open_latest_history_output();
                self.continue_action_queue();
            }
        }
    }

    fn apply_successful_action_effect(&mut self, effect: Option<ActionEffect>) {
        match effect {
            Some(ActionEffect::ColorMode(mode)) => {
                self.snapshot.color_mode = mode.clone();
                self.messages
                    .push(format!("Option appliquée dans le cockpit: couleur {mode}"));
            }
            Some(ActionEffect::DefaultAgent(agent)) => {
                let agent = dw_config::normalize_default_agent(&agent)
                    .unwrap_or(agent.as_str())
                    .to_string();
                let agent_options = self
                    .snapshot
                    .workflow
                    .agent
                    .get_or_insert_with(dw_config::AgentOptions::default);
                agent_options.default = agent.clone();
                self.messages
                    .push(format!("Option appliquée dans le cockpit: agent {agent}"));
            }
            Some(ActionEffect::Root(root)) => {
                self.root_override = Some(root.clone());
                self.snapshot.root = root.clone();
                self.messages
                    .push(format!("Option appliquée dans le cockpit: root {root}"));
            }
            None => {}
        }
    }

    fn continue_action_queue(&mut self) {
        if let Some(label) = self.background.start_next_action() {
            self.history.start_running(label.clone());
            self.messages
                .push(format!("Lancement suivant en arrière-plan: {label}"));
        } else if self.reload_after_action_queue {
            self.reload_after_action_queue = false;
            self.reload();
        }
    }

    fn reload(&mut self) {
        let should_load_assigned =
            self.view == View::Ado || self.snapshot.assigned_loaded || self.assigned_loading();
        let should_load_pull_requests = self.view == View::PullRequests
            || self.snapshot.pull_requests_loaded
            || self.pull_requests_loading();
        if self.background.start_snapshot(self.root_override.clone()) {
            self.reload_assigned_after_snapshot = should_load_assigned;
            self.reload_pull_requests_after_snapshot = should_load_pull_requests;
            self.messages
                .push("Rechargement du snapshot en arrière-plan...".into());
        } else {
            self.messages.push("Rechargement déjà en cours.".into());
        }
    }

    fn accept_snapshot_reload(&mut self, snapshot: TuiSnapshot) {
        self.snapshot = snapshot;
        self.clamp_after_snapshot_reload();
        let should_load_assigned = self.reload_assigned_after_snapshot;
        let should_load_pull_requests = self.reload_pull_requests_after_snapshot;
        self.reload_assigned_after_snapshot = false;
        self.reload_pull_requests_after_snapshot = false;
        if should_load_assigned {
            self.restart_assigned_load();
        }
        if should_load_pull_requests {
            self.restart_pull_requests_load();
        }
        self.messages.push(snapshot_reload_summary(&self.snapshot));
    }

    fn clamp_after_snapshot_reload(&mut self) {
        self.clamp_action_selection();
        self.clamp_cockpit_selection();
        self.selected_workspace = self
            .selected_workspace
            .min(self.snapshot.workspaces.len().saturating_sub(1));
        self.selected_ado_project = self
            .selected_ado_project
            .min(self.snapshot.assigned.len().saturating_sub(1));
        self.clamp_ado_item_selection();
        self.clamp_pull_request_selection();
        self.clamp_database_selection();
    }

    fn next_view(&mut self) {
        let index = View::ALL
            .iter()
            .position(|view| *view == self.view)
            .unwrap_or_default();
        self.set_view(View::ALL[(index + 1) % View::ALL.len()]);
    }

    fn previous_view(&mut self) {
        let index = View::ALL
            .iter()
            .position(|view| *view == self.view)
            .unwrap_or_default();
        self.set_view(View::ALL[(index + View::ALL.len() - 1) % View::ALL.len()]);
    }

    fn set_view(&mut self, view: View) {
        self.view = view;
        self.selected_action = 0;
        self.selected_cockpit = self
            .selected_cockpit
            .min(self.cockpit_items().len().saturating_sub(1));
        self.confirmation = None;
        self.form = None;
        self.options_open = false;
        if self.view == View::Ado && !self.snapshot.assigned_loaded {
            self.start_assigned_load();
        }
        if self.view == View::PullRequests && !self.snapshot.pull_requests_loaded {
            self.start_pull_requests_load();
        }
    }

    fn open_form(&mut self) {
        self.form = Some(FormState::selecting());
        self.filter_active = false;
        self.confirmation = None;
        self.options_open = false;
        self.messages.push("Constructeur d’action ouvert.".into());
    }

    fn open_db_query_form(&mut self) {
        self.open_database_form(FormTemplate::DbQuery, "Requête DB guidée ouverte.");
    }

    fn open_db_describe_form(&mut self) {
        self.open_database_form(FormTemplate::DbDescribe, "Describe DB guidé ouvert.");
    }

    fn open_database_form(&mut self, template: FormTemplate, message: &str) {
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|candidate| *candidate == template)
            .unwrap_or_default();
        form.begin_editing(&self.snapshot);
        if let Some(database) = self.snapshot.database_entries.get(self.selected_database) {
            for field in &mut form.fields {
                match field.label.as_str() {
                    "Projet" => field.value = database.project.clone().unwrap_or_default(),
                    "Database" => field.value = database.key.clone(),
                    _ => {}
                }
            }
        }
        self.form = Some(form);
        self.filter_active = false;
        self.confirmation = None;
        self.options_open = false;
        self.messages.push(message.into());
    }

    fn open_start_pr_form(&mut self) {
        let Some(item) = self.snapshot.pull_requests.get(self.selected_pull_request) else {
            self.messages
                .push(self.selected_pull_request_action_error(PullRequestAction::StartPreview));
            return;
        };
        let Some(pull_request_id) = item.pull_request_id else {
            self.messages
                .push(self.selected_pull_request_action_error(PullRequestAction::StartPreview));
            return;
        };

        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::TaskStartPr)
            .unwrap_or_default();
        form.begin_editing(&self.snapshot);
        for field in &mut form.fields {
            match field.label.as_str() {
                "Pull request" => field.value = pull_request_id.to_string(),
                "Projet" => field.value = item.project.clone(),
                "Repository" => field.value = item.repository.clone(),
                _ => {}
            }
        }
        self.form = Some(form);
        self.filter_active = false;
        self.confirmation = None;
        self.options_open = false;
        self.messages.push(format!(
            "Workspace depuis PR guidé ouvert pour #{}.",
            pull_request_id
        ));
    }

    fn open_ado_set_state_form(&mut self) {
        let Some(project) = self.snapshot.assigned.get(self.selected_ado_project) else {
            self.messages.push(self.selected_ado_action_error());
            return;
        };
        let Some(item) = project.items.get(self.selected_ado_item) else {
            self.messages.push(self.selected_ado_action_error());
            return;
        };

        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::AdoSetState)
            .unwrap_or_default();
        form.begin_editing(&self.snapshot);
        let workflow_state = actions::selected_ado_action(
            &self.snapshot,
            self.selected_ado_project,
            self.selected_ado_item,
            AdoItemAction::SetStartState,
        )
        .and_then(|action| match action.request {
            TuiActionRequest::AdoSetState(args) => Some(args.state),
            _ => None,
        });
        for field in &mut form.fields {
            match field.label.as_str() {
                "Work items" => field.value = item.id.clone(),
                "Projet" => field.value = project.key.clone(),
                "State" => {
                    if let Some(state) = workflow_state.clone() {
                        field.value = state;
                    }
                }
                _ => {}
            }
        }
        self.form = Some(form);
        self.filter_active = false;
        self.confirmation = None;
        self.options_open = false;
        self.messages.push(format!(
            "Changement d'état ADO guidé ouvert pour #{}.",
            item.id
        ));
    }

    fn open_options(&mut self) {
        self.options_open = true;
        self.selected_option = self
            .selected_option
            .min(QUICK_OPTIONS.len().saturating_sub(1));
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.state_open = false;
        self.history.close_output();
        self.messages.push("Options ouvertes.".into());
    }

    fn open_state_modal(&mut self) {
        self.state_open = true;
        self.state_scroll = 0;
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.options_open = false;
        self.history.close_output();
    }

    fn open_history_output(&mut self) {
        if !self.history.open_output() {
            self.messages.push("Aucun lancement à afficher.".into());
            return;
        }
        self.close_overlays_for_history();
    }

    fn open_latest_history_output(&mut self) {
        if self.history.open_output() {
            self.close_overlays_for_history();
        }
    }

    fn close_overlays_for_history(&mut self) {
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.options_open = false;
        self.state_open = false;
    }

    fn start_assigned_load(&mut self) {
        if self.background.start_assigned(&mut self.snapshot) {
            self.messages
                .push("Chargement de vos work items en arrière-plan...".into());
        }
    }

    fn restart_assigned_load(&mut self) {
        self.background.restart_assigned(&mut self.snapshot);
        self.messages
            .push("Rechargement de vos work items en arrière-plan...".into());
    }

    fn start_pull_requests_load(&mut self) {
        if self.background.start_pull_requests(&mut self.snapshot) {
            self.messages
                .push("Chargement PRs en arrière-plan...".into());
        }
    }

    fn restart_pull_requests_load(&mut self) {
        self.background.restart_pull_requests(&mut self.snapshot);
        self.messages
            .push("Rechargement PRs en arrière-plan...".into());
    }

    fn move_action_up(&mut self) {
        self.selected_action = self.selected_action.saturating_sub(1);
    }

    fn move_action_down(&mut self) {
        let len = self.visible_actions().len();
        if len > 0 {
            self.selected_action = (self.selected_action + 1).min(len - 1);
        }
    }

    fn move_workspace_up(&mut self) {
        self.selected_workspace = self.selected_workspace.saturating_sub(1);
        self.clamp_action_selection();
    }

    fn move_workspace_down(&mut self) {
        if !self.snapshot.workspaces.is_empty() {
            self.selected_workspace =
                (self.selected_workspace + 1).min(self.snapshot.workspaces.len() - 1);
        }
        self.clamp_action_selection();
    }

    fn move_ado_project_up(&mut self) {
        if !self.snapshot.assigned.is_empty() {
            self.selected_ado_project = (self.selected_ado_project + self.snapshot.assigned.len()
                - 1)
                % self.snapshot.assigned.len();
        }
        self.clamp_ado_item_selection();
    }

    fn move_ado_project_down(&mut self) {
        if !self.snapshot.assigned.is_empty() {
            self.selected_ado_project =
                (self.selected_ado_project + 1) % self.snapshot.assigned.len();
        }
        self.clamp_ado_item_selection();
    }

    fn move_ado_item_up(&mut self) {
        self.selected_ado_item = self.selected_ado_item.saturating_sub(1);
    }

    fn move_ado_item_down(&mut self) {
        if let Some(project) = self.snapshot.assigned.get(self.selected_ado_project)
            && !project.items.is_empty()
        {
            self.selected_ado_item = (self.selected_ado_item + 1).min(project.items.len() - 1);
        }
    }

    fn move_pull_request_up(&mut self) {
        self.selected_pull_request = self.selected_pull_request.saturating_sub(1);
    }

    fn move_pull_request_down(&mut self) {
        if !self.snapshot.pull_requests.is_empty() {
            self.selected_pull_request =
                (self.selected_pull_request + 1).min(self.snapshot.pull_requests.len() - 1);
        }
    }

    fn move_database_up(&mut self) {
        self.selected_database = self.selected_database.saturating_sub(1);
    }

    fn move_database_down(&mut self) {
        if !self.snapshot.database_entries.is_empty() {
            self.selected_database =
                (self.selected_database + 1).min(self.snapshot.database_entries.len() - 1);
        }
    }

    fn move_option_up(&mut self) {
        self.selected_option = self.selected_option.saturating_sub(1);
    }

    fn move_option_down(&mut self) {
        if !QUICK_OPTIONS.is_empty() {
            self.selected_option = (self.selected_option + 1).min(QUICK_OPTIONS.len() - 1);
        }
    }

    fn clamp_action_selection(&mut self) {
        let len = self.visible_actions().len();
        self.selected_action = self.selected_action.min(len.saturating_sub(1));
    }

    fn move_cockpit_up(&mut self) {
        self.selected_cockpit = self.selected_cockpit.saturating_sub(1);
    }

    fn move_cockpit_down(&mut self) {
        let len = self.cockpit_items().len();
        if len > 0 {
            self.selected_cockpit = (self.selected_cockpit + 1).min(len - 1);
        }
    }

    fn clamp_cockpit_selection(&mut self) {
        let len = self.cockpit_items().len();
        self.selected_cockpit = self.selected_cockpit.min(len.saturating_sub(1));
    }

    fn move_action_form_up(&mut self) {
        match self.action_form.mode {
            FormMode::Selecting => self.action_form.move_template_up(),
            FormMode::Editing => self.action_form.move_field_up(),
        }
    }

    fn move_action_form_down(&mut self) {
        match self.action_form.mode {
            FormMode::Selecting => self.action_form.move_template_down(),
            FormMode::Editing => self.action_form.move_field_down(),
        }
    }

    fn clamp_ado_item_selection(&mut self) {
        let len = self
            .snapshot
            .assigned
            .get(self.selected_ado_project)
            .map(|project| project.items.len())
            .unwrap_or_default();
        self.selected_ado_item = self.selected_ado_item.min(len.saturating_sub(1));
    }

    fn clamp_pull_request_selection(&mut self) {
        self.selected_pull_request = self
            .selected_pull_request
            .min(self.snapshot.pull_requests.len().saturating_sub(1));
    }

    fn clamp_database_selection(&mut self) {
        self.selected_database = self
            .selected_database
            .min(self.snapshot.database_entries.len().saturating_sub(1));
    }
}

fn action_matches_view(action: &TuiAction, view: View) -> bool {
    match view {
        View::Dashboard | View::Composer | View::Help => true,
        View::Workspaces => action.is_workspace_action(),
        View::PullRequests => action.is_workspace_action() || action.is_ado_action(),
        View::Ado => action.is_ado_action(),
        View::Db => action.is_db_action(),
        View::Config => action.is_config_action(),
    }
}

fn assigned_load_summary(projects: &[AdoAssignedProject]) -> String {
    let items = projects
        .iter()
        .map(|project| project.items.len())
        .sum::<usize>();
    let errors = projects
        .iter()
        .filter(|project| project.error.is_some())
        .count();
    if errors == 0 {
        format!("Mes work items chargés: {items} work item(s).")
    } else {
        format!("Mes work items chargés: {items} work item(s), {errors} erreur(s) projet.")
    }
}

fn pull_request_load_summary(items: &[TuiPullRequest]) -> String {
    let active = items
        .iter()
        .filter(|item| item.pull_request_id.is_some())
        .count();
    let errors = items.iter().filter(|item| item.error.is_some()).count();
    if errors == 0 {
        format!("contexte chargé: {active} PR active(s).")
    } else {
        format!("contexte chargé: {active} PR active(s), {errors} erreur(s) repository.")
    }
}

fn snapshot_reload_summary(snapshot: &TuiSnapshot) -> String {
    format!(
        "Snapshot rechargé: {} projet(s), {} workspace(s), {} database(s), {} prune.",
        snapshot.project_count(),
        snapshot.workspaces.len(),
        snapshot.database_count(),
        snapshot.prune_candidates
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AdoAssignedProject;

    #[test]
    fn view_filter_keeps_task_actions_in_workspace_view() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.set_view(View::Workspaces);

        assert!(
            app.visible_actions()
                .iter()
                .all(|(_, action)| action.is_workspace_action())
        );
    }

    #[test]
    fn new_starts_with_minimal_snapshot_and_background_load() {
        let app = App::new(Some("/tmp/missing-dw-root".into()));

        assert!(app.snapshot_loading());
        assert_eq!(app.snapshot.root, "/tmp/missing-dw-root");
        assert!(app.snapshot.workspaces.is_empty());
        assert!(
            app.messages
                .iter()
                .any(|message| message.contains("Chargement du snapshot"))
        );
    }

    #[test]
    fn background_status_lines_explain_loading_and_idle_work() {
        let app = App::new(Some("/tmp/missing-dw-root".into()));

        let lines = app.background_status_lines();

        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Snapshot: chargement"))
        );
        assert!(lines.contains(&"Mes work items: non chargé".into()));
        assert!(lines.contains(&"PRs: non chargées".into()));
        assert!(lines.contains(&"Action: aucune".into()));
    }

    #[test]
    fn action_queue_status_lines_show_next_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let first = TuiAction {
            label: "Version".into(),
            request: TuiActionRequest::Version,
            description: "Version".into(),
            kind: ActionRisk::Safe,
        };
        let second = TuiAction {
            label: "Doctor".into(),
            request: TuiActionRequest::Doctor,
            description: "Doctor".into(),
            kind: ActionRisk::Safe,
        };
        let third = TuiAction {
            label: "Guide".into(),
            request: TuiActionRequest::Guide,
            description: "Guide".into(),
            kind: ActionRisk::Safe,
        };

        assert!(matches!(
            app.background.start_action(first),
            ActionStart::Started { .. }
        ));
        assert!(matches!(
            app.background.start_action(second),
            ActionStart::Queued { .. }
        ));
        assert!(matches!(
            app.background.start_action(third),
            ActionStart::Queued { .. }
        ));

        assert_eq!(
            app.action_queue_status_lines(),
            ["À suivre: Doctor", "Puis: 1 autre(s) action(s)"]
        );
    }

    #[test]
    fn background_status_lines_include_loaded_counts() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned_loaded = true;
        app.snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "Hommage Agence".into(),
            items: vec![crate::model::AdoAssignedItem {
                id: "42".into(),
                kind: "Task".into(),
                state: "Active".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }];
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![TuiPullRequest {
            workspace: Some("/tmp/ws".into()),
            project: "ha".into(),
            repository: "front".into(),
            ado_repository: "HA Front".into(),
            branch: "feature/42-demo".into(),
            target_branch: "develop".into(),
            pull_request_id: Some(123),
            title: Some("Demo".into()),
            is_draft: false,
            work_item_ids: vec!["42".into()],
            url: None,
            error: None,
        }];

        let lines = app.background_status_lines();

        assert!(lines.contains(&"Snapshot: prêt".into()));
        assert!(lines.contains(&"Mes work items: 1 items".into()));
        assert!(lines.contains(&"PRs: 1 actives".into()));
    }

    #[test]
    fn workspace_view_filters_actions_to_selected_workspace() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![
            workspace("/tmp/ws-one", "one"),
            workspace("/tmp/ws-two", "two"),
        ];
        app.snapshot.actions = app
            .snapshot
            .workspaces
            .iter()
            .flat_map(crate::model::workspace_actions_for_tui)
            .collect();
        app.set_view(View::Workspaces);
        app.selected_workspace = 1;

        assert!(
            app.visible_actions()
                .iter()
                .all(|(_, action)| action.workspace_path() == Some("/tmp/ws-two"))
        );
    }

    #[test]
    fn workspace_view_hides_global_task_catalog_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];
        app.snapshot.actions.extend(
            app.snapshot
                .workspaces
                .iter()
                .flat_map(crate::model::workspace_actions_for_tui),
        );
        app.set_view(View::Workspaces);

        let labels = app
            .visible_actions()
            .iter()
            .map(|(_, action)| action.display_label())
            .collect::<Vec<_>>();

        assert!(
            app.visible_actions()
                .iter()
                .all(|(_, action)| action.action_kind() != crate::model::ActionKind::TaskPrune)
        );
        assert!(labels.iter().any(|label| label.contains("Vérifier")));
    }

    #[test]
    fn workspace_teardown_shortcut_action_is_rooted_and_confirmed() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];

        let action =
            actions::selected_workspace_action(&app.snapshot, 0, WorkspaceAction::TeardownExecute)
                .expect("teardown action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        match action.request {
            TuiActionRequest::TaskTeardown(args) => {
                assert_eq!(args.workspace.as_deref(), Some("/tmp/ws-one"));
                assert_eq!(args.root.as_deref(), Some("/tmp/missing-dw-root"));
                assert!(args.mode.executes());
                assert!(args.yes);
            }
            _ => panic!("expected teardown request"),
        }
    }

    #[test]
    fn workspace_finish_execute_action_is_rooted_and_confirmed() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];

        let action =
            actions::selected_workspace_action(&app.snapshot, 0, WorkspaceAction::FinishExecute)
                .expect("finish action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        match action.request {
            TuiActionRequest::TaskFinish(args) => {
                assert_eq!(args.workspace.as_deref(), Some("/tmp/ws-one"));
                assert_eq!(args.root.as_deref(), Some("/tmp/missing-dw-root"));
                assert!(args.mode.executes());
                assert!(args.yes);
            }
            _ => panic!("expected finish request"),
        }
    }

    #[test]
    fn text_filter_matches_action_content() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.filter = "doctor".into();

        assert!(
            app.visible_actions()
                .iter()
                .any(|(_, action)| action.display_label().contains("Doctor"))
        );
    }

    #[test]
    fn load_summaries_include_counts_and_partial_errors() {
        let assigned = vec![
            AdoAssignedProject {
                key: "ha".into(),
                label: "HA".into(),
                items: vec![crate::model::AdoAssignedItem {
                    id: "42".into(),
                    kind: "Task".into(),
                    state: "Active".into(),
                    title: "Demo".into(),
                    url: None,
                }],
                error: None,
            },
            AdoAssignedProject {
                key: "ops".into(),
                label: "OPS".into(),
                items: Vec::new(),
                error: Some("boom".into()),
            },
        ];
        let prs = vec![
            crate::model::TuiPullRequest {
                workspace: None,
                project: "ha".into(),
                repository: "front".into(),
                ado_repository: "HA Front".into(),
                branch: "feature/demo".into(),
                target_branch: "develop".into(),
                pull_request_id: Some(12),
                title: None,
                is_draft: false,
                work_item_ids: Vec::new(),
                url: None,
                error: None,
            },
            crate::model::TuiPullRequest {
                workspace: None,
                project: "ha".into(),
                repository: "back".into(),
                ado_repository: "HA Back".into(),
                branch: "-".into(),
                target_branch: "-".into(),
                pull_request_id: None,
                title: None,
                is_draft: false,
                work_item_ids: Vec::new(),
                url: None,
                error: Some("boom".into()),
            },
        ];

        assert_eq!(
            assigned_load_summary(&assigned),
            "Mes work items chargés: 1 work item(s), 1 erreur(s) projet."
        );
        assert_eq!(
            pull_request_load_summary(&prs),
            "contexte chargé: 1 PR active(s), 1 erreur(s) repository."
        );
    }

    #[test]
    fn generated_form_action_can_request_confirmation() {
        let app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let mut form = FormState::selecting();
        form.begin_editing(&app.snapshot);
        form.fields[0].value = "42".into();
        form.fields[6].toggle();
        let action = form.build_action(&app.snapshot.root).expect("action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        assert!(!action.runs_attached_in_tui());
    }

    #[test]
    fn ado_project_tabs_loop_with_shift_navigation() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned = vec![
            assigned_project("one"),
            assigned_project("two"),
            assigned_project("three"),
        ];

        app.move_ado_project_up();
        assert_eq!(app.selected_ado_project, 2);
        app.move_ado_project_down();
        assert_eq!(app.selected_ado_project, 0);
    }

    #[test]
    fn view_navigation_helper_routes_contextual_keys() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned = vec![assigned_project("one"), assigned_project("two")];
        app.view = View::Ado;

        assert!(
            app.handle_view_navigation_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::NONE))
        );
        assert_eq!(app.selected_ado_project, 1);
        assert!(
            !app.handle_view_navigation_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE))
        );
    }

    #[test]
    fn ado_action_error_explains_loading_state() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned_loaded = false;

        assert_eq!(
            app.selected_ado_action_error(),
            "Mes work items pas encore chargés: ouvrir l'onglet ADO ou appuyer sur r pour recharger."
        );

        app.start_assigned_load();
        assert_eq!(
            app.selected_ado_action_error(),
            "Mes work items chargent en arrière-plan; vous pouvez rester dans le TUI."
        );
    }

    #[test]
    fn pull_request_action_error_explains_loading_state() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = false;

        assert_eq!(
            app.selected_pull_request_action_error(PullRequestAction::DiffPreview),
            "PRs pas encore chargées: ouvrir l'onglet PRs ou appuyer sur r pour recharger."
        );

        app.start_pull_requests_load();
        assert_eq!(
            app.selected_pull_request_action_error(PullRequestAction::DiffPreview),
            "PRs en cours de chargement; vous pouvez continuer à naviguer."
        );
    }

    #[test]
    fn dashboard_cockpit_turns_pr_without_workspace_into_start_action() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![pull_request("ha", "front", 42)];

        let items = app.cockpit_items();

        let item = items
            .iter()
            .find(|item| item.title.contains("PR #42"))
            .expect("cockpit PR item");
        assert_eq!(item.section, "À traiter");
        assert!(matches!(
            item.primary_action.request,
            TuiActionRequest::TaskStartPr(_)
        ));
    }

    #[test]
    fn ado_open_url_reports_missing_url() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned = vec![AdoAssignedProject {
            key: "ha".into(),
            label: "HA".into(),
            items: vec![crate::model::AdoAssignedItem {
                id: "42".into(),
                kind: "Task".into(),
                state: "Active".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }];

        app.open_selected_ado_url();

        assert!(
            app.messages
                .iter()
                .any(|message| message.contains("URL absente"))
        );
    }

    #[test]
    fn pull_request_open_url_reports_missing_url() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests = vec![crate::model::TuiPullRequest {
            workspace: None,
            project: "ha".into(),
            repository: "front".into(),
            ado_repository: "HA Front".into(),
            branch: "feature/demo".into(),
            target_branch: "develop".into(),
            pull_request_id: Some(12),
            title: None,
            is_draft: false,
            work_item_ids: Vec::new(),
            url: None,
            error: None,
        }];

        app.open_selected_pull_request_url();

        assert!(
            app.messages
                .iter()
                .any(|message| message.contains("URL PR absente"))
        );
    }

    #[test]
    fn db_query_form_is_prefilled_from_selected_database() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.database_entries = vec![crate::model::TuiDatabase {
            project: Some("ha".into()),
            key: "ha-dev".into(),
        }];

        app.open_db_query_form();

        let form = app.form.expect("form");
        assert_eq!(form.template, FormTemplate::DbQuery);
        assert!(
            form.fields
                .iter()
                .any(|field| field.label == "Projet" && field.value == "ha")
        );
        assert!(
            form.fields
                .iter()
                .any(|field| field.label == "Database" && field.value == "ha-dev")
        );
    }

    #[test]
    fn db_describe_form_is_prefilled_from_selected_database() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.database_entries = vec![crate::model::TuiDatabase {
            project: Some("ha".into()),
            key: "ha-dev".into(),
        }];

        app.open_db_describe_form();

        let form = app.form.expect("form");
        assert_eq!(form.template, FormTemplate::DbDescribe);
        assert!(
            form.fields
                .iter()
                .any(|field| field.label == "Projet" && field.value == "ha")
        );
        assert!(
            form.fields
                .iter()
                .any(|field| field.label == "Database" && field.value == "ha-dev")
        );
        assert!(
            form.fields
                .iter()
                .any(|field| field.label == "Table" && field.value.is_empty())
        );
    }

    #[test]
    fn ado_set_state_form_is_prefilled_from_selected_item() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned = vec![assigned_project_with_item("ha", "User Story")];
        app.snapshot.assigned_loaded = true;

        app.open_ado_set_state_form();

        let form = app.form.expect("form");
        assert_eq!(form.template, FormTemplate::AdoSetState);
        assert_eq!(field_value(&form, "Work items"), Some("42"));
        assert_eq!(field_value(&form, "Projet"), Some("ha"));
        assert_eq!(field_value(&form, "State"), Some("En réalisation"));
    }

    #[test]
    fn start_pr_form_is_prefilled_from_selected_pull_request() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![
            pull_request("ha", "front", 42),
            pull_request("ops", "tools", 77),
        ];
        app.selected_pull_request = 1;

        app.open_start_pr_form();

        let form = app.form.expect("form");
        assert_eq!(form.template, FormTemplate::TaskStartPr);
        assert_eq!(field_value(&form, "Pull request"), Some("77"));
        assert_eq!(field_value(&form, "Projet"), Some("ops"));
        assert_eq!(field_value(&form, "Repository"), Some("tools"));
        assert!(
            app.messages
                .iter()
                .any(|message| message.contains("Workspace depuis PR guidé"))
        );
    }

    #[test]
    fn options_selection_moves_and_clamps() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.selected_option = usize::MAX;

        app.open_options();
        assert_eq!(app.selected_option, QUICK_OPTIONS.len() - 1);

        app.move_option_up();
        assert_eq!(app.selected_option, QUICK_OPTIONS.len() - 2);

        app.selected_option = 0;
        app.move_option_up();
        assert_eq!(app.selected_option, 0);

        app.move_option_down();
        assert_eq!(app.selected_option, 1);
    }

    #[test]
    fn config_show_quick_option_uses_core_detail_panel() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        assert!(app.run_core_quick_option(QuickOptionAction::ConfigShow));

        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Configuration effective");
        let crate::model::DetailPanelContent::ConfigShow(report) = detail.content else {
            panic!("expected config show panel");
        };
        assert_eq!(report.root, "/tmp/missing-dw-root");
        assert!(app.history.entries.is_empty());
    }

    #[test]
    fn config_doctor_quick_option_refreshes_snapshot_and_uses_core_detail_panel() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.config_doctor = dw_config::ConfigDoctorReport {
            root: "/tmp/old".into(),
            passed: true,
            checks: vec![],
        };

        assert!(app.run_core_quick_option(QuickOptionAction::ConfigDoctor));

        assert_eq!(app.snapshot.config_doctor.root, "/tmp/missing-dw-root");
        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Diagnostic configuration");
        let crate::model::DetailPanelContent::ConfigDoctor(report) = detail.content else {
            panic!("expected config doctor panel");
        };
        assert_eq!(report.root, "/tmp/missing-dw-root");
        assert_eq!(report, app.snapshot.config_doctor);
        assert!(app.history.entries.is_empty());
    }

    #[test]
    fn agent_doctor_quick_option_uses_core_detail_panel() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        assert!(app.run_core_quick_option(QuickOptionAction::AgentDoctor));

        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Diagnostic agents");
        let crate::model::DetailPanelContent::AgentDoctor(report) = detail.content else {
            panic!("expected agent doctor panel");
        };
        assert!(!report.checks.is_empty());
        assert!(app.history.entries.is_empty());
    }

    #[test]
    fn guide_action_uses_detail_panel_not_history() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let action = actions::option_action(&app.snapshot.root, QuickOptionAction::Guide);

        assert!(app.run_inline_detail_action(&action));

        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Guide DevWorkflow");
        let crate::model::DetailPanelContent::Guide(lines) = detail.content else {
            panic!("expected guide panel");
        };
        assert!(lines.iter().any(|line| line.contains("Onglet Composer")));
        assert!(app.history.entries.is_empty());
    }

    #[test]
    fn history_output_modal_opens_scrolls_and_closes() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.push(RunHistoryEntry {
            request_label: "Doctor".into(),
            status: "exit 0".into(),
            success: true,
            output_preview: vec!["three".into()],
            output_lines: vec!["one".into(), "two".into(), "three".into()],
        });
        app.history.push(RunHistoryEntry {
            request_label: "Version".into(),
            status: "exit 0".into(),
            success: true,
            output_preview: vec!["version".into()],
            output_lines: vec!["version".into()],
        });

        app.open_history_output();
        assert!(app.history.output_open);
        assert_eq!(app.history.selected_entry, 1);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("down");
        assert_eq!(app.history.output_scroll, 0);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE))
            .expect("previous");
        assert_eq!(app.history.selected_entry, 0);

        app.handle_history_output_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE))
            .expect("end");
        assert_eq!(app.history.output_scroll, 2);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE))
            .expect("next");
        assert_eq!(app.history.selected_entry, 1);
        assert_eq!(app.history.output_scroll, 0);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("esc");
        assert!(!app.history.output_open);
        assert_eq!(app.history.output_scroll, 0);
    }

    #[test]
    fn action_result_finishes_streaming_history_entry() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.start_running("Version".into());
        app.history
            .append_running_line("Version", "Chargement...".into());

        app.accept_action_result(
            "Version".into(),
            false,
            false,
            None,
            Ok(runner::CapturedActionRunResult {
                display_label: "Version".into(),
                status_label: "exit 0".into(),
                success: true,
                output: "Chargement...\nDev Workflow 2026.07.04".into(),
            }),
        );

        assert_eq!(app.history.entries.len(), 1);
        let entry = app.history.selected_entry().expect("entry");
        assert_eq!(entry.status, "exit 0");
        assert_eq!(
            entry.output_preview,
            ["Chargement...", "Dev Workflow 2026.07.04"]
        );
    }

    #[test]
    fn report_action_result_opens_detail_panel_and_keeps_run_log() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.start_running("Mes work items · ha".into());

        app.accept_action_result(
            "Mes work items · ha".into(),
            false,
            true,
            None,
            Ok(runner::CapturedActionRunResult {
                display_label: "Mes work items · ha".into(),
                status_label: "ok".into(),
                success: true,
                output: "Work items assignés\n#55264 Transmission automatique".into(),
            }),
        );

        assert!(!app.history.output_open);
        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Résultat · Mes work items · ha");
        let crate::model::DetailPanelContent::OperationResult { lines, .. } = detail.content else {
            panic!("expected operation result panel");
        };
        assert!(lines.iter().any(|line| line.contains("#55264")));
        let entry = app.history.selected_entry().expect("entry");
        assert_eq!(entry.request_label, "Mes work items · ha");
        assert!(
            entry
                .output_lines
                .iter()
                .any(|line| line.contains("#55264"))
        );
    }

    #[test]
    fn successful_option_action_updates_visible_snapshot_immediately() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            "Couleur · always".into(),
            true,
            false,
            Some(ActionEffect::ColorMode("always".into())),
            Ok(runner::CapturedActionRunResult {
                display_label: "Couleur · always".into(),
                status_label: "exit 0".into(),
                success: true,
                output: "Couleur   : always".into(),
            }),
        );

        assert_eq!(app.snapshot.color_mode, "always");
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Option appliquée dans le cockpit: couleur always")
        );
    }

    #[test]
    fn successful_agent_action_updates_visible_snapshot_immediately() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            "Agent par défaut · codex".into(),
            true,
            false,
            Some(ActionEffect::DefaultAgent("codex".into())),
            Ok(runner::CapturedActionRunResult {
                display_label: "Agent par défaut · codex".into(),
                status_label: "exit 0".into(),
                success: true,
                output: "Agent par défaut: codex".into(),
            }),
        );

        assert_eq!(app.snapshot.default_agent(), "codex");
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Option appliquée dans le cockpit: agent codex")
        );
    }

    #[test]
    fn successful_agent_effect_normalizes_visible_agent() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            "Agent par défaut · CODEX-CLI".into(),
            true,
            false,
            Some(ActionEffect::DefaultAgent("CODEX-CLI".into())),
            Ok(runner::CapturedActionRunResult {
                display_label: "Agent par défaut · CODEX-CLI".into(),
                status_label: "exit 0".into(),
                success: true,
                output: "Agent par défaut: codex-cli".into(),
            }),
        );

        assert_eq!(app.snapshot.default_agent(), "codex-cli");
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Option appliquée dans le cockpit: agent codex-cli")
        );
    }

    #[test]
    fn successful_set_root_action_updates_visible_root_immediately() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            "Root · /tmp/new-root".into(),
            true,
            false,
            Some(ActionEffect::Root("/tmp/new-root".into())),
            Ok(runner::CapturedActionRunResult {
                display_label: "Root · /tmp/new-root".into(),
                status_label: "exit 0".into(),
                success: true,
                output: "Root      : /tmp/new-root".into(),
            }),
        );

        assert_eq!(app.root_override.as_deref(), Some("/tmp/new-root"));
        assert_eq!(app.snapshot.root, "/tmp/new-root");
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Option appliquée dans le cockpit: root /tmp/new-root")
        );
    }

    #[test]
    fn mutating_action_result_triggers_snapshot_reload_after_queue_drains() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            "Task sync · /tmp/ws".into(),
            true,
            false,
            None,
            Ok(runner::CapturedActionRunResult {
                display_label: "Task sync · /tmp/ws".into(),
                status_label: "exit 0".into(),
                success: true,
                output: "Workspace synchronisé.".into(),
            }),
        );

        assert!(
            app.messages
                .iter()
                .any(|message| message == "Rechargement du snapshot en arrière-plan...")
        );
        assert!(!app.reload_after_action_queue);
    }

    #[test]
    fn snapshot_reload_acceptance_replaces_data_and_clamps_selection() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];
        app.snapshot.database_entries = vec![crate::model::TuiDatabase {
            project: None,
            key: "shared".into(),
        }];
        app.selected_workspace = 4;
        app.selected_database = 3;
        app.reload_assigned_after_snapshot = false;
        app.reload_pull_requests_after_snapshot = false;
        let mut snapshot = app.snapshot.clone();
        snapshot.workspaces = Vec::new();
        snapshot.database_entries = Vec::new();

        app.accept_snapshot_reload(snapshot);

        assert_eq!(app.selected_workspace, 0);
        assert_eq!(app.selected_database, 0);
        assert!(!app.reload_assigned_after_snapshot);
        assert!(!app.reload_pull_requests_after_snapshot);
        assert!(
            app.messages
                .iter()
                .any(|message| message.starts_with("Snapshot rechargé:"))
        );
    }

    #[test]
    fn snapshot_reload_summary_includes_operational_counts() {
        let mut snapshot = TuiSnapshot::load(Some("/tmp/missing-dw-root"));
        snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];
        snapshot
            .databases
            .globals
            .insert("shared".into(), serde_json::json!({}));
        snapshot.database_entries = crate::model::database_entries_for_tui(&snapshot.databases);
        snapshot.prune_candidates = 1;

        let summary = snapshot_reload_summary(&snapshot);

        assert!(summary.contains("1 workspace(s)"));
        assert!(summary.contains("1 database(s)"));
        assert!(summary.contains("1 prune"));
    }

    fn assigned_project(key: &str) -> AdoAssignedProject {
        AdoAssignedProject {
            key: key.into(),
            label: key.into(),
            items: Vec::new(),
            error: None,
        }
    }

    fn assigned_project_with_item(key: &str, kind: &str) -> AdoAssignedProject {
        AdoAssignedProject {
            key: key.into(),
            label: key.into(),
            items: vec![crate::model::AdoAssignedItem {
                id: "42".into(),
                kind: kind.into(),
                state: "Nouveau".into(),
                title: "Demo".into(),
                url: None,
            }],
            error: None,
        }
    }

    fn field_value<'a>(form: &'a FormState, label: &str) -> Option<&'a str> {
        form.fields
            .iter()
            .find(|field| field.label == label)
            .map(|field| field.value.as_str())
    }

    fn pull_request(project: &str, repository: &str, pull_request_id: i64) -> TuiPullRequest {
        TuiPullRequest {
            workspace: None,
            project: project.into(),
            repository: repository.into(),
            ado_repository: repository.into(),
            branch: format!("feature/{pull_request_id}-demo"),
            target_branch: "develop".into(),
            pull_request_id: Some(pull_request_id),
            title: Some("Demo".into()),
            is_draft: false,
            work_item_ids: vec![pull_request_id.to_string()],
            url: None,
            error: None,
        }
    }

    fn workspace(path: &str, slug: &str) -> dw_workspace::TaskListItem {
        dw_workspace::TaskListItem {
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
