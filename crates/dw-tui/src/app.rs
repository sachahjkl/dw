use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use dw_core::{InputRequest, InputResponse, PromptChoiceValue};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::collections::BTreeSet;
use std::io;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::actions::{
    self, AdoItemAction, DatabaseAction, PullRequestAction, QUICK_OPTIONS, QuickOptionAction,
    QuickOptionItem,
};
use crate::background::{ActionStart, BackgroundJobs, BackgroundKind, BackgroundResult};
use crate::form::{FieldKind, FormMode, FormState, FormTemplate};
use crate::history::{
    ActionRunId, ActionRunLabel, ActionRunRecord, ActionRunStatus, HistoryState, JournalLogLevel,
    RunHistoryEntry,
};
use crate::model::{
    ActionEffect, ActionRisk, AdoAssignedProject, CockpitItem, CockpitSeverity, DetailPanel,
    TuiAction, TuiActionRequest, TuiPullRequest, TuiSnapshot, View, WorkspaceAction,
};
use crate::ui_text::history_journal_lines;
use crate::{runner, ui};

pub const MENU_SECTIONS: &[MenuSection] = &[
    MenuSection::Information,
    MenuSection::Configuration,
    MenuSection::DefaultAgent,
    MenuSection::TerminalColor,
];

pub async fn run_tui(root: Option<String>) -> Result<()> {
    runner::install_terminal()?;
    let result = run_tui_inner(root).await;
    runner::restore_terminal()?;
    result
}

async fn run_tui_inner(root: Option<String>) -> Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(root);

    while !app.should_quit {
        app.poll_background_loads();
        terminal.draw(|frame| ui::render(frame, &app))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if should_handle_key_event(key) => {
                    app.handle_key(key, &mut terminal).await?
                }
                Event::Key(_) => {}
                Event::Mouse(mouse) => app.handle_mouse(mouse.kind),
                _ => {}
            }
        }
    }

    Ok(())
}

fn should_handle_key_event(key: KeyEvent) -> bool {
    !matches!(key.kind, KeyEventKind::Release)
}

fn action_blocks_until_done(action: &TuiAction) -> bool {
    match &action.request {
        TuiActionRequest::TaskStart(args) => args.mode.executes(),
        TuiActionRequest::TaskStartPr(args) => args.mode.executes(),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    Menu,
    MenuSection,
    Help,
    State,
    History,
    Detail,
    ActionProgress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSection {
    Information,
    Configuration,
    DefaultAgent,
    TerminalColor,
}

impl MenuSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Information => "Information",
            Self::Configuration => "Configuration",
            Self::DefaultAgent => "Default agent",
            Self::TerminalColor => "Terminal color",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Information => "Journal, state and help.",
            Self::Configuration => "Config, diagnostics and setup.",
            Self::DefaultAgent => "Choose the agent used by default.",
            Self::TerminalColor => "Choose terminal color behavior.",
        }
    }
}

pub struct TuiInputPrompt {
    pub run_id: ActionRunId,
    pub request: InputRequest,
    pub value: String,
    pub selected: usize,
    pub selected_many: BTreeSet<usize>,
    response: Option<Sender<InputResponse>>,
}

impl TuiInputPrompt {
    fn new(run_id: ActionRunId, request: InputRequest, response: Sender<InputResponse>) -> Self {
        Self {
            run_id,
            request,
            value: String::new(),
            selected: 0,
            selected_many: BTreeSet::new(),
            response: Some(response),
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        let max = match &self.request {
            InputRequest::SelectOne { choices, .. } | InputRequest::SelectMany { choices, .. } => {
                choices.len().saturating_sub(1)
            }
            _ => 0,
        };
        self.selected = (self.selected + 1).min(max);
    }

    fn toggle_selected(&mut self) {
        if matches!(self.request, InputRequest::SelectMany { .. })
            && !self.selected_many.remove(&self.selected)
        {
            self.selected_many.insert(self.selected);
        }
    }

    fn input_response(&self, accepted: bool) -> InputResponse {
        match &self.request {
            InputRequest::Confirm { .. } => InputResponse::Confirm { accepted },
            InputRequest::SelectOne { choices, .. } => InputResponse::SelectOne {
                value: choices
                    .get(self.selected)
                    .map(|choice| choice.value.clone())
                    .unwrap_or_else(|| PromptChoiceValue::from("")),
            },
            InputRequest::SelectMany { choices, .. } => InputResponse::SelectMany {
                values: self
                    .selected_many
                    .iter()
                    .filter_map(|index| choices.get(*index))
                    .map(|choice| choice.value.clone())
                    .collect(),
            },
            InputRequest::Text { .. } => InputResponse::Text {
                value: self.value.clone(),
            },
            InputRequest::Secret { .. } => InputResponse::Secret {
                value: self.value.clone(),
            },
        }
    }

    fn answer(&mut self, accepted: bool) -> Result<()> {
        let message = self.input_response(accepted);
        let Some(response) = self.response.take() else {
            return Ok(());
        };
        response
            .send(message)
            .map_err(|_| anyhow::anyhow!("Action stopped before receiving input"))
    }

    fn cancel(&mut self) {
        self.response.take();
    }
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
    pub selected_menu_section: usize,
    pub selected_option: usize,
    pub filter: String,
    pub filter_active: bool,
    pub confirmation: Option<TuiAction>,
    pub input_prompt: Option<TuiInputPrompt>,
    pub form: Option<FormState>,
    pub action_form: FormState,
    pub options_open: bool,
    pub help_open: bool,
    pub state_open: bool,
    pub state_scroll: usize,
    pub detail: Option<DetailPanel>,
    pub action_progress: Option<ActionRunId>,
    pub modal_stack: Vec<ModalKind>,
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
        let needs_init = snapshot.needs_init;
        let mut messages = vec!["TUI ready. Enter runs the selected operation.".into()];
        if needs_init {
            messages.push("DevWorkflow root is not initialized. Init is required.".into());
        } else {
            let _ = background.start_snapshot(root.clone());
            messages.push("Loading snapshot, work items and PRs in the background...".into());
        }
        let mut app = Self::from_snapshot(root, snapshot, background, messages);
        if !needs_init {
            app.reload_assigned_after_snapshot = true;
            app.reload_pull_requests_after_snapshot = true;
        }
        app
    }

    #[cfg(test)]
    pub(crate) fn new_ready(root: Option<String>) -> Self {
        let mut snapshot = TuiSnapshot::loading(root.as_deref());
        snapshot.needs_init = false;
        Self::from_snapshot(
            root,
            snapshot,
            BackgroundJobs::new(),
            vec!["TUI ready. Enter runs the selected operation.".into()],
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
            selected_menu_section: 0,
            selected_option: 0,
            filter: String::new(),
            filter_active: false,
            confirmation: None,
            input_prompt: None,
            form: None,
            action_form: FormState::selecting(),
            options_open: false,
            help_open: false,
            state_open: false,
            state_scroll: 0,
            detail: None,
            action_progress: None,
            modal_stack: Vec::new(),
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
                BackgroundResult::ActionEvent {
                    generation,
                    run_id,
                    event,
                } => {
                    if self.background.accepts_action_output(generation) {
                        self.history.append_running_event(run_id, event);
                    }
                }
                BackgroundResult::ActionInput {
                    generation,
                    run_id,
                    request,
                    response,
                } => {
                    if self.background.accepts_action_output(generation) {
                        self.messages
                            .push(format!("Input required: {}", request.id()));
                        self.input_prompt = Some(TuiInputPrompt::new(run_id, request, response));
                    }
                }
                BackgroundResult::Action {
                    generation,
                    run_id,
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
                        run_id,
                        label,
                        refresh_after_success,
                        open_after_success,
                        effect,
                        *result,
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

    #[cfg(test)]
    pub fn running_action_label(&self) -> Option<&ActionRunLabel> {
        self.background.action_label()
    }

    pub fn latest_action_event_line(&self) -> Option<String> {
        self.history
            .current_running_entry()
            .and_then(RunHistoryEntry::latest_event)
            .map(dw_ui::action_event_line)
    }

    pub fn active_action_status_text(&self) -> Option<String> {
        let entry = self.history.current_running_entry()?;
        let label = &entry.request_label;
        let status = entry
            .latest_event()
            .map(dw_ui::action_event_line)
            .unwrap_or_else(|| "waiting for first event".into());
        Some(format!("{label} -> {status}"))
    }

    pub fn pending_action_count(&self) -> usize {
        self.background.pending_action_count()
    }

    pub fn pending_action_labels(&self) -> Vec<ActionRunLabel> {
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
            "ready".into(),
        ));
        lines.push(self.background_status_line(
            BackgroundKind::Assigned,
            "My work items",
            if self.snapshot.assigned_loaded {
                format!("{} items", self.snapshot.assigned_count())
            } else {
                "not loaded".into()
            },
        ));
        lines.push(self.background_status_line(
            BackgroundKind::PullRequests,
            "PRs",
            if self.snapshot.pull_requests_loaded {
                format!(
                    "{} active",
                    self.snapshot
                        .pull_requests
                        .iter()
                        .filter(|item| item.pull_request_id.is_some())
                        .count()
                )
            } else {
                "not loaded".into()
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
        let mut lines = vec![format!("Next: {first}")];
        if pending.len() > 1 {
            lines.push(format!("Then: {} other action(s)", pending.len() - 1));
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
            format!("{label}: loading {elapsed}")
        } else {
            format!("{label}: {idle}")
        }
    }

    fn action_status_line(&self) -> String {
        let queued = self.background.pending_action_count();
        if let Some(label) = self.background.action_label() {
            let elapsed = self
                .background
                .elapsed_label(BackgroundKind::Action)
                .unwrap_or_else(|| "<1s".into());
            let status = self
                .active_action_status_text()
                .unwrap_or_else(|| label.to_string());
            if queued > 0 {
                format!("Action: {status} ({elapsed}, queue {queued})")
            } else {
                format!("Action: {status} ({elapsed})")
            }
        } else if let Some(status) = self.active_action_status_text() {
            if queued > 0 {
                format!("Action: {status} (queue {queued})")
            } else {
                format!("Action: {status}")
            }
        } else if queued > 0 {
            format!("Action: queue {queued}")
        } else {
            "Action: none".into()
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
                .is_none_or(|path| path == workspace.path.as_str())
    }

    pub fn selected_visible_action(&self) -> Option<(usize, &TuiAction)> {
        let actions = self.visible_actions();
        actions
            .get(self.selected_action.min(actions.len().saturating_sub(1)))
            .copied()
    }

    pub fn cockpit_items(&self) -> Vec<CockpitItem> {
        let mut items = Vec::new();
        if !self.snapshot.config_doctor.passed && !self.snapshot.config_doctor.checks.is_empty() {
            items.push(CockpitItem {
                section: "Attention",
                title: "Configuration needs attention".into(),
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
                    title: format!("My work items unavailable · {}", project.key),
                    subtitle: error.clone(),
                    status: "error".into(),
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
                    section: "To do",
                    title: format!(
                        "Create PR workspace #{}",
                        pr_item
                            .pull_request_id
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    ),
                    subtitle: format!(
                        "{} / {} · {}",
                        pr_item.project, pr_item.repository, pr_item.branch
                    ),
                    status: "PR without workspace".into(),
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
                    section: "In progress",
                    title: format!(
                        "Finish PR #{}",
                        pr_item
                            .pull_request_id
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    ),
                    subtitle: pr_item.workspace.clone().unwrap_or_default(),
                    status: "workspace linked".into(),
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
                    section: "In progress",
                    title: format!(
                        "Preflight {}",
                        workspace
                            .work_items
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    subtitle: workspace.path.to_string(),
                    status: workspace.kind.to_string(),
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
                        section: "To do",
                        title: format!("Start #{} · {}", item.id, item.title),
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
                    "{} workspace(s) eligible for pruning",
                    self.snapshot.prune_candidates
                ),
                subtitle: self.snapshot.root.clone(),
                status: "preview".into(),
                severity: CockpitSeverity::Attention,
                primary_action: TuiAction {
                    label: "Prune preview".into(),
                    request: TuiActionRequest::TaskPrune(dw_task::prune::PruneArgs {
                        root: Some(dw_core::DevWorkflowRoot::from(self.snapshot.root.clone())),
                        project: None,
                        work_item_ids: Vec::new(),
                        selected_workspaces: None,
                        mode: dw_core::ExecutionMode::Preview,
                        yes: false,
                        no_sync: true,
                    }),
                    description: "Preview workspaces that can be cleaned".into(),
                    kind: ActionRisk::DryRun,
                },
            });
        }
        if items.is_empty() {
            items.push(CockpitItem {
                section: "OK",
                title: "No urgent decision".into(),
                subtitle: "Use the domain tabs or the advanced composer.".into(),
                status: "idle".into(),
                severity: CockpitSeverity::Normal,
                primary_action: actions::option_action(
                    &self.snapshot.root,
                    QuickOptionAction::Refresh,
                ),
            });
        }
        items
    }

    pub async fn handle_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.snapshot.needs_init {
            return self.handle_init_required_key(key, terminal).await;
        }

        if self.input_prompt.is_some() {
            return self.handle_input_prompt_key(key);
        }

        if self.form.is_some() {
            return self.handle_form_key(key, terminal).await;
        }

        if let Some(modal) = self.modal_stack.last().copied() {
            return match modal {
                ModalKind::Menu => self.handle_options_key(key, terminal),
                ModalKind::MenuSection => self.handle_menu_section_key(key, terminal).await,
                ModalKind::Help => self.handle_help_key(key),
                ModalKind::State => self.handle_state_key(key),
                ModalKind::History => self.handle_history_output_key(key),
                ModalKind::Detail => self.handle_detail_key(key),
                ModalKind::ActionProgress => self.handle_action_progress_key(key),
            };
        }

        if self.detail.is_some() {
            return self.handle_detail_key(key);
        }

        if self.filter_active {
            return self.handle_filter_key(key);
        }

        if self.confirmation.is_some() {
            return self.handle_confirmation_key(key, terminal).await;
        }

        if self.view == View::Composer && self.handle_action_builder_key(key, terminal).await? {
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
            KeyCode::Char('m') => self.open_options(),
            KeyCode::Char('r') => self.reload(),
            KeyCode::Char('1') => self.set_view(View::Dashboard),
            KeyCode::Char('2') => self.set_view(View::Workspaces),
            KeyCode::Char('3') => self.set_view(View::Ado),
            KeyCode::Char('4') => self.set_view(View::PullRequests),
            KeyCode::Char('5') => self.set_view(View::Db),
            KeyCode::Char('6') => self.set_view(View::Composer),
            KeyCode::Char('?') => self.open_help_modal(),
            _ => {}
        }
        if self.handle_view_action_key(key, terminal).await? {
            return Ok(());
        }
        if key.code == KeyCode::Enter {
            if self.view == View::Dashboard {
                self.request_or_run_selected_cockpit_item(terminal).await?;
            } else if self.view == View::Workspaces {
                self.request_or_run_workspace_action(WorkspaceAction::Open, terminal)
                    .await?;
            } else {
                self.request_or_run_selected_action(terminal).await?;
            }
        }
        Ok(())
    }

    fn handle_input_prompt_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(prompt) = self.input_prompt.as_mut() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => {
                prompt.cancel();
                self.input_prompt = None;
                self.messages.push("Input canceled.".into());
            }
            KeyCode::Enter => {
                prompt.answer(true)?;
                self.input_prompt = None;
                self.messages.push("Input sent.".into());
            }
            KeyCode::Char('y') | KeyCode::Char('Y')
                if matches!(prompt.request, InputRequest::Confirm { .. }) =>
            {
                prompt.answer(true)?;
                self.input_prompt = None;
                self.messages.push("Confirmation sent.".into());
            }
            KeyCode::Char('n') | KeyCode::Char('N')
                if matches!(prompt.request, InputRequest::Confirm { .. }) =>
            {
                prompt.answer(false)?;
                self.input_prompt = None;
                self.messages.push("Confirmation declined.".into());
            }
            KeyCode::Up | KeyCode::Char('k') => prompt.move_up(),
            KeyCode::Down | KeyCode::Char('j') => prompt.move_down(),
            KeyCode::Char(' ') => prompt.toggle_selected(),
            KeyCode::Backspace => {
                prompt.value.pop();
            }
            KeyCode::Char(value)
                if matches!(
                    prompt.request,
                    InputRequest::Text { .. } | InputRequest::Secret { .. }
                ) =>
            {
                prompt.value.push(value);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_init_required_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Enter | KeyCode::Char('i') => {
                if self.action_loading() {
                    self.messages.push("Init already running.".into());
                } else {
                    self.run_action(self.init_required_action(), terminal)
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn init_required_action(&self) -> TuiAction {
        TuiAction {
            label: "Initialize DevWorkflow root".into(),
            request: TuiActionRequest::ConfigInit(dw_config::command::InitCommandArgs {
                root: Some(self.snapshot.root.clone()),
                profile: "business".into(),
                dry_run: false,
                no_save: false,
            }),
            description: "Create the root config, schemas, cache and project directories".into(),
            kind: ActionRisk::Destructive,
        }
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
            KeyCode::Up | KeyCode::Char('k') if self.view == View::Workspaces => {
                self.move_workspace_up()
            }
            KeyCode::Down | KeyCode::Char('j') if self.view == View::Workspaces => {
                self.move_workspace_down()
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

    fn handle_mouse(&mut self, kind: MouseEventKind) {
        match kind {
            MouseEventKind::ScrollDown => self.scroll_current_context_down(),
            MouseEventKind::ScrollUp => self.scroll_current_context_up(),
            _ => {}
        }
    }

    fn scroll_current_context_down(&mut self) {
        if self.history.output_open {
            self.scroll_history_output_down();
        } else if let Some(detail) = self.detail.as_mut() {
            detail.scroll_down();
        } else if self.options_open {
            self.move_option_down();
        } else if self.help_open {
            // Help content currently fits in its modal; keep mouse wheel local to the modal.
        } else if self.state_open {
            self.state_scroll = self.state_scroll.saturating_add(1);
        } else {
            match self.view {
                View::Dashboard => self.move_cockpit_down(),
                View::Ado => self.move_ado_item_down(),
                View::PullRequests => self.move_pull_request_down(),
                View::Db => self.move_database_down(),
                View::Composer => self.move_action_form_down(),
                View::Workspaces => self.move_workspace_down(),
            }
        }
    }

    fn scroll_current_context_up(&mut self) {
        if self.history.output_open {
            self.history.scroll_output_up();
        } else if let Some(detail) = self.detail.as_mut() {
            detail.scroll_up();
        } else if self.options_open {
            self.move_option_up();
        } else if self.help_open {
            // Help content currently fits in its modal; keep mouse wheel local to the modal.
        } else if self.state_open {
            self.state_scroll = self.state_scroll.saturating_sub(1);
        } else {
            match self.view {
                View::Dashboard => self.move_cockpit_up(),
                View::Ado => self.move_ado_item_up(),
                View::PullRequests => self.move_pull_request_up(),
                View::Db => self.move_database_up(),
                View::Composer => self.move_action_form_up(),
                View::Workspaces => self.move_workspace_up(),
            }
        }
    }

    async fn handle_view_action_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<bool> {
        match (self.view, key.code) {
            (View::Ado, KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('s')) => {
                self.request_or_run_ado_action(AdoItemAction::StartPreview, terminal)
                    .await?
            }
            (View::Ado, KeyCode::Char('x')) => {
                self.request_or_run_ado_action(AdoItemAction::StartExecute, terminal)
                    .await?
            }
            (View::Ado, KeyCode::Char('c')) => {
                self.request_or_run_ado_action(AdoItemAction::Context, terminal)
                    .await?
            }
            (View::Ado, KeyCode::Char('w')) => {
                self.request_or_run_ado_action(AdoItemAction::WorkItem, terminal)
                    .await?
            }
            (View::Ado, KeyCode::Char('e')) => {
                self.request_or_run_ado_action(AdoItemAction::SetStartState, terminal)
                    .await?
            }
            (View::Ado, KeyCode::Char('E')) => self.open_ado_set_state_form(),
            (View::Ado, KeyCode::Char('o')) => {
                self.request_or_run_ado_action(AdoItemAction::OpenAgent, terminal)
                    .await?
            }
            (View::Ado, KeyCode::Char('u')) => self.open_selected_ado_url(),
            (View::Workspaces, KeyCode::Enter | KeyCode::Char('o')) => {
                self.request_or_run_workspace_action(WorkspaceAction::Open, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('p')) => {
                self.request_or_run_workspace_action(WorkspaceAction::Preflight, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('s')) => {
                self.request_or_run_workspace_action(WorkspaceAction::Sync, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('l')) => {
                self.request_or_run_workspace_action(WorkspaceAction::RepoLatest, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('v')) => {
                self.request_or_run_workspace_action(WorkspaceAction::HandoffValidate, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('c')) => {
                self.request_or_run_workspace_action(WorkspaceAction::CommitPreview, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('f')) => {
                self.request_or_run_workspace_action(WorkspaceAction::FinishPreview, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('F')) => {
                self.request_or_run_workspace_action(WorkspaceAction::FinishExecute, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('t')) => {
                self.request_or_run_workspace_action(WorkspaceAction::TeardownPreview, terminal)
                    .await?
            }
            (View::Workspaces, KeyCode::Char('x')) => {
                self.request_or_run_workspace_action(WorkspaceAction::TeardownExecute, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('s')) => {
                self.request_or_run_pull_request_action(PullRequestAction::StartPreview, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('x')) => {
                self.request_or_run_pull_request_action(PullRequestAction::StartExecute, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('f')) => {
                self.request_or_run_pull_request_action(PullRequestAction::FinishPreview, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('F')) => {
                self.request_or_run_pull_request_action(PullRequestAction::FinishExecute, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('c')) => {
                self.request_or_run_pull_request_action(PullRequestAction::Changelog, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('d')) => {
                self.request_or_run_pull_request_action(PullRequestAction::DiffPreview, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('o')) => {
                self.request_or_run_pull_request_action(PullRequestAction::OpenAgent, terminal)
                    .await?
            }
            (View::PullRequests, KeyCode::Char('N')) => self.open_start_pr_form(),
            (View::PullRequests, KeyCode::Char('u')) => self.open_selected_pull_request_url(),
            (View::Db, KeyCode::Enter | KeyCode::Char('s')) => {
                self.request_or_run_db_action(DatabaseAction::Schema, terminal)
                    .await?
            }
            (View::Db, KeyCode::Char('d')) => self.open_db_describe_form(),
            (View::Db, KeyCode::Char('e')) => self.open_db_query_form(),
            _ => return Ok(false),
        }
        Ok(true)
    }

    async fn handle_form_key(
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
                    self.messages.push("Form canceled.".into());
                }
                KeyCode::Up | KeyCode::Char('k') => form.move_template_up(),
                KeyCode::Down | KeyCode::Char('j') => form.move_template_down(),
                KeyCode::Enter => form.begin_editing(&self.snapshot),
                _ => {}
            },
            FormMode::Editing => match key.code {
                KeyCode::Esc => {
                    self.form = None;
                    self.messages.push("Form canceled.".into());
                }
                KeyCode::Up | KeyCode::BackTab => form.move_field_up(),
                KeyCode::Down | KeyCode::Tab => form.move_field_down(),
                KeyCode::Backspace => form.backspace(),
                KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    match form.apply_suggestion(&self.snapshot) {
                        Some(value) => self.messages.push(format!("Suggestion applied: {value}")),
                        None => self
                            .messages
                            .push("No suggestion available for this field.".into()),
                    }
                }
                KeyCode::Char(' ') => form.toggle_selected(),
                KeyCode::Enter => {
                    let action = form.build_action(&self.snapshot.root);
                    self.form = None;
                    match action {
                        Some(action) => self.request_or_run_action(action, terminal).await?,
                        None => self
                            .messages
                            .push("Incomplete form: no action generated.".into()),
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
                self.close_top_modal();
            }
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Up | KeyCode::Char('k') => {
                self.history.scroll_output_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_history_output_down();
            }
            KeyCode::Left | KeyCode::Char('[') => self.history.select_previous_entry(),
            KeyCode::Right | KeyCode::Char(']') => self.history.select_next_entry(),
            KeyCode::Home => self.history.scroll_output_home(),
            KeyCode::End => self.scroll_history_output_end(),
            KeyCode::Char('f') => self.history.toggle_output_fullscreen(),
            KeyCode::Char('a') => self.history.enable_all_log_levels(),
            KeyCode::Char('e') => self.history.toggle_log_level(JournalLogLevel::Error),
            KeyCode::Char('w') => self.history.toggle_log_level(JournalLogLevel::Warn),
            KeyCode::Char('i') => self.history.toggle_log_level(JournalLogLevel::Info),
            KeyCode::Char('d') => self.history.toggle_log_level(JournalLogLevel::Debug),
            KeyCode::Char('o') => self.history.toggle_log_level(JournalLogLevel::Other),
            _ => {}
        }
        Ok(())
    }

    fn scroll_history_output_down(&mut self) {
        self.history.output_scroll =
            (self.history.output_scroll + 1).min(self.history_output_max_scroll());
    }

    fn scroll_history_output_end(&mut self) {
        self.history.output_scroll = self.history_output_max_scroll();
    }

    fn history_output_max_scroll(&self) -> usize {
        history_journal_lines(self).len().saturating_sub(1)
    }

    fn handle_state_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('q') => {
                self.close_top_modal();
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
                self.close_top_modal();
            }
            KeyCode::Up | KeyCode::Char('k') => detail.scroll_up(),
            KeyCode::Down | KeyCode::Char('j') => detail.scroll_down(),
            KeyCode::Home => detail.scroll_home(),
            KeyCode::End => detail.scroll_end(),
            _ => {}
        }
        Ok(())
    }

    fn handle_action_progress_key(&mut self, key: KeyEvent) -> Result<()> {
        if matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
        }
        Ok(())
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter => {
                self.close_top_modal();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_options_key(
        &mut self,
        key: KeyEvent,
        _terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc | KeyCode::Char('m') => {
                self.close_top_modal();
                self.messages.push("Menu closed.".into());
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_option_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_option_down(),
            KeyCode::Enter => self.open_selected_menu_section(),
            KeyCode::Char('?') => self.open_help_modal(),
            _ => {}
        }
        Ok(())
    }

    async fn handle_menu_section_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => self.close_top_modal(),
            KeyCode::Char('m') => self.close_menu_modals(),
            KeyCode::Up | KeyCode::Char('k') => self.move_option_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_option_down(),
            KeyCode::Enter => self.run_selected_option_action(terminal).await?,
            KeyCode::Char('?') => self.open_help_modal(),
            KeyCode::Char(key) => {
                if let Some(action) = self.current_menu_option_by_key(key) {
                    self.run_option_action(action, terminal).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_confirmation_key(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Some(action) = self.confirmation.take() {
                    self.run_action(action, terminal).await?;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.confirmation = None;
                self.messages.push("Action canceled.".into());
            }
            _ => {}
        }
        Ok(())
    }

    async fn request_or_run_selected_action(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some((_, action)) = self.selected_visible_action() else {
            self.messages.push("No operation selected.".into());
            return Ok(());
        };

        self.request_or_run_action(action.clone(), terminal).await
    }

    async fn request_or_run_selected_cockpit_item(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let items = self.cockpit_items();
        let Some(item) = items.get(self.selected_cockpit.min(items.len().saturating_sub(1))) else {
            self.messages.push("No cockpit item selected.".into());
            return Ok(());
        };
        self.request_or_run_action(item.primary_action.clone(), terminal)
            .await
    }

    async fn handle_action_builder_key(
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
                        Some(value) => self.messages.push(format!("Suggestion applied: {value}")),
                        None => self
                            .messages
                            .push("No suggestion available for this field.".into()),
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
                        Some(action) => self.request_or_run_action(action, terminal).await?,
                        None => self
                            .messages
                            .push("Incomplete composer: no action generated.".into()),
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

    async fn request_or_run_action(
        &mut self,
        action: TuiAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.run_inline_detail_action(&action).await? {
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
                .push(format!("Confirmation required: {display_label}"));
            return Ok(());
        }

        self.run_action(action, terminal).await
    }

    async fn run_inline_detail_action(&mut self, action: &TuiAction) -> Result<bool> {
        match &action.request {
            TuiActionRequest::Guide => {
                self.open_detail_panel(DetailPanel::guide());
                self.messages.push("Quick start opened.".into());
                Ok(true)
            }
            TuiActionRequest::ConfigShow { .. } => {
                let result = match runner::run_captured_streaming(action, |_| {}).await {
                    Ok(result) => result,
                    Err(error) => {
                        self.accept_inline_action_error(error);
                        return Ok(true);
                    }
                };
                match result.result {
                    dw_app::DwActionResult::Config(dw_app::ConfigActionResult::Show(report)) => {
                        self.open_detail_panel(DetailPanel::config_show(&report));
                        self.messages.push("Configuration loaded from core.".into());
                        Ok(true)
                    }
                    result => anyhow::bail!("Unexpected config show result: {result:?}"),
                }
            }
            TuiActionRequest::ConfigDoctor { .. } => {
                let result = match runner::run_captured_streaming(action, |_| {}).await {
                    Ok(result) => result,
                    Err(error) => {
                        self.accept_inline_action_error(error);
                        return Ok(true);
                    }
                };
                match result.result {
                    dw_app::DwActionResult::Config(dw_app::ConfigActionResult::Doctor(report)) => {
                        self.snapshot.config_doctor = report.clone();
                        self.open_detail_panel(DetailPanel::config_doctor(&report));
                        self.messages
                            .push("Configuration doctor completed from core.".into());
                        Ok(true)
                    }
                    result => anyhow::bail!("Unexpected config doctor result: {result:?}"),
                }
            }
            TuiActionRequest::AgentDoctor { .. } => {
                let result = match runner::run_captured_streaming(action, |_| {}).await {
                    Ok(result) => result,
                    Err(error) => {
                        self.accept_inline_action_error(error);
                        return Ok(true);
                    }
                };
                match result.result {
                    dw_app::DwActionResult::Agent(dw_app::AgentActionResult::Doctor(report)) => {
                        self.open_detail_panel(DetailPanel::agent_doctor(&report));
                        self.messages
                            .push("Agent doctor completed from core.".into());
                        Ok(true)
                    }
                    result => anyhow::bail!("Unexpected agent doctor result: {result:?}"),
                }
            }
            _ => Ok(false),
        }
    }

    fn accept_inline_action_error(&mut self, error: runner::CapturedActionRunError) {
        let label = error.display_label.clone();
        let message = error.message.clone();
        self.history.push(RunHistoryEntry {
            id: ActionRunId::new(0),
            request_label: ActionRunLabel::new(label.clone()),
            status: ActionRunStatus::Failed,
            record: ActionRunRecord::failed(error.events, error.message),
        });
        self.messages.push(format!("Failed: {label} -> {message}"));
        self.open_latest_history_output();
    }

    async fn run_option_action(
        &mut self,
        option: QuickOptionAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if self.run_core_quick_option(option).await? {
            return Ok(());
        }
        self.request_or_run_action(
            actions::option_action(&self.snapshot.root, option),
            terminal,
        )
        .await
    }

    async fn run_core_quick_option(&mut self, option: QuickOptionAction) -> Result<bool> {
        self.run_inline_detail_action(&actions::option_action(&self.snapshot.root, option))
            .await
    }

    async fn run_selected_option_action(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        match self.selected_menu_section() {
            MenuSection::Information => match self.selected_option {
                0 => {
                    self.open_history_output();
                    return Ok(());
                }
                1 => {
                    self.open_state_modal();
                    return Ok(());
                }
                2 => {
                    self.open_help_modal();
                    return Ok(());
                }
                _ => {}
            },
            _ => {
                let Some(option) = self.current_menu_option(self.selected_option) else {
                    self.messages.push("No menu option selected.".into());
                    return Ok(());
                };
                self.run_option_action(option.action, terminal).await?;
            }
        }
        Ok(())
    }

    fn open_selected_menu_section(&mut self) {
        self.selected_option = self
            .selected_option
            .min(self.current_menu_item_count().saturating_sub(1));
        self.push_modal(ModalKind::MenuSection);
    }

    pub(crate) fn selected_menu_section(&self) -> MenuSection {
        MENU_SECTIONS
            .get(self.selected_menu_section)
            .copied()
            .unwrap_or(MenuSection::Information)
    }

    pub(crate) fn current_menu_item_count(&self) -> usize {
        match self.selected_menu_section() {
            MenuSection::Information => 3,
            section => self.quick_options_for_menu_section(section).len(),
        }
    }

    pub(crate) fn quick_options_for_menu_section(
        &self,
        section: MenuSection,
    ) -> Vec<&'static QuickOptionItem> {
        let quick_section = match section {
            MenuSection::Information => return Vec::new(),
            MenuSection::Configuration => "Diagnostics and setup",
            MenuSection::DefaultAgent => "Default agent",
            MenuSection::TerminalColor => "Terminal color mode",
        };
        QUICK_OPTIONS
            .iter()
            .filter(|item| item.section == quick_section)
            .collect()
    }

    fn current_menu_option(&self, index: usize) -> Option<&'static QuickOptionItem> {
        self.quick_options_for_menu_section(self.selected_menu_section())
            .get(index)
            .copied()
    }

    fn current_menu_option_by_key(&self, key: char) -> Option<QuickOptionAction> {
        self.quick_options_for_menu_section(self.selected_menu_section())
            .into_iter()
            .find(|item| item.key == key)
            .map(|item| item.action)
    }

    fn close_menu_modals(&mut self) {
        self.options_open = false;
        self.sync_closed_modal(ModalKind::Menu);
        self.sync_closed_modal(ModalKind::MenuSection);
    }

    async fn request_or_run_ado_action(
        &mut self,
        action: AdoItemAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) = self.selected_ado_action(action) else {
            self.messages.push(
                actions::ado_action_error(
                    &self.snapshot,
                    self.selected_ado_project,
                    self.selected_ado_item,
                    action,
                )
                .unwrap_or_else(|| self.selected_ado_action_error()),
            );
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal).await
    }

    async fn request_or_run_workspace_action(
        &mut self,
        action: WorkspaceAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) =
            actions::selected_workspace_action(&self.snapshot, self.selected_workspace, action)
        else {
            self.messages.push("No workspace selected.".into());
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal).await
    }

    async fn request_or_run_pull_request_action(
        &mut self,
        action: PullRequestAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) = self.selected_pull_request_action(action) else {
            self.messages
                .push(self.selected_pull_request_action_error(action));
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal).await
    }

    fn open_selected_ado_url(&mut self) {
        let Some(project) = self.snapshot.assigned.get(self.selected_ado_project) else {
            self.messages.push("No ADO project selected.".into());
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
                "Action unavailable: missing URL for work item #{}.",
                item.id
            ));
            return;
        };
        self.open_url("work item ADO", &url);
    }

    fn open_selected_pull_request_url(&mut self) {
        let Some(item) = self.snapshot.pull_requests.get(self.selected_pull_request) else {
            self.messages.push("No PR selected.".into());
            return;
        };
        let Some(url) = item
            .url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .map(str::to_string)
        else {
            self.messages
                .push("Action unavailable: missing PR URL in the Azure DevOps response.".into());
            return;
        };
        self.open_url("PR ADO", &url);
    }

    fn open_url(&mut self, label: &str, url: &str) {
        match webbrowser::open(url) {
            Ok(_) => self.messages.push(format!("{label} opened: {url}")),
            Err(error) => self
                .messages
                .push(format!("Could not open {label}: {error}. URL: {url}")),
        }
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
            "My work items are loading in the background; you can keep using the TUI.".into()
        } else if !self.snapshot.assigned_loaded {
            "My work items are not loaded yet: wait for preload or reload.".into()
        } else if self.snapshot.assigned.is_empty() {
            "No ADO project configured or usable for your work items.".into()
        } else {
            "No ADO work item selected.".into()
        }
    }

    pub fn selected_pull_request_action_preview_for(
        &self,
        action: PullRequestAction,
    ) -> Option<String> {
        actions::selected_pull_request_action(&self.snapshot, self.selected_pull_request, action)
            .map(|action| action.display_label())
    }

    pub fn selected_workspace_action_preview_for(&self, action: WorkspaceAction) -> Option<String> {
        actions::selected_workspace_action(&self.snapshot, self.selected_workspace, action)
            .map(|action| action.display_label())
    }

    async fn request_or_run_db_action(
        &mut self,
        action: DatabaseAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let Some(selected_action) =
            actions::selected_database_action(&self.snapshot, self.selected_database, action)
        else {
            self.messages.push("No DB entry selected.".into());
            return Ok(());
        };

        self.request_or_run_action(selected_action, terminal).await
    }

    fn selected_pull_request_action(&self, action: PullRequestAction) -> Option<TuiAction> {
        actions::selected_pull_request_action(&self.snapshot, self.selected_pull_request, action)
    }

    fn selected_pull_request_action_error(&self, action: PullRequestAction) -> String {
        if self.pull_requests_loading() {
            return "PRs are loading; you can keep navigating.".into();
        }
        if !self.snapshot.pull_requests_loaded {
            return "PRs are not loaded yet: wait for preload or reload.".into();
        }
        actions::pull_request_action_error(&self.snapshot, self.selected_pull_request, action)
    }

    async fn run_action(
        &mut self,
        action: TuiAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        if !action.runs_attached_in_tui() {
            let blocks_until_done = action_blocks_until_done(&action);
            match self.background.start_action(action) {
                ActionStart::Started { run_id, label } => {
                    self.history.start_running(run_id, label.clone());
                    if blocks_until_done {
                        self.open_action_progress_modal(run_id);
                    }
                    self.messages.push(format!("Background launch: {label}"));
                }
                ActionStart::Queued { label, position } => {
                    self.messages
                        .push(format!("Action queued #{position}: {label}"));
                }
            }
            return Ok(());
        }

        terminal.show_cursor().ok();
        let result = runner::run_attached(&action).await?;
        terminal.clear()?;
        self.history.push(RunHistoryEntry {
            id: ActionRunId::new(0),
            request_label: ActionRunLabel::new(result.display_label.clone()),
            status: ActionRunStatus::Succeeded,
            record: ActionRunRecord::ExternalLaunch {
                plan: Box::new(result.launch.clone()),
            },
        });
        self.messages.push(format!(
            "Last launch: {} -> {}",
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
        run_id: ActionRunId,
        label: ActionRunLabel,
        refresh_after_success: bool,
        open_after_success: bool,
        effect: Option<ActionEffect>,
        result: std::result::Result<
            runner::CapturedActionRunResult,
            runner::CapturedActionRunError,
        >,
    ) {
        match result {
            Ok(result) => {
                self.close_action_progress_modal(run_id);
                let events = self
                    .history
                    .record_events_for(run_id, result.events.clone());
                let record = ActionRunRecord::Completed {
                    events,
                    result: Box::new(result.result.clone()),
                };
                if !self
                    .history
                    .finish_running(run_id, ActionRunStatus::Succeeded, record.clone())
                {
                    self.history.push(RunHistoryEntry {
                        id: run_id,
                        request_label: label.clone(),
                        status: ActionRunStatus::Succeeded,
                        record: record.clone(),
                    });
                }
                self.messages.push(format!(
                    "Done: {} -> {}",
                    result.display_label, result.status_label
                ));
                if open_after_success {
                    self.open_detail_panel(DetailPanel::action_result(
                        format!("Result · {}", result.display_label),
                        result.events.clone(),
                        result.result.clone(),
                    ));
                    self.history.close_output();
                    self.sync_closed_modal(ModalKind::History);
                }
                if result.success && refresh_after_success {
                    self.apply_successful_action_effect(effect);
                    self.reload_after_action_queue = true;
                }
                self.continue_action_queue();
            }
            Err(error) => {
                self.close_action_progress_modal(run_id);
                let record = ActionRunRecord::Failed {
                    events: self.history.record_events_for(run_id, error.events.clone()),
                    error: error.message.clone(),
                };
                if !self
                    .history
                    .finish_running(run_id, ActionRunStatus::Failed, record.clone())
                {
                    self.history.push(RunHistoryEntry {
                        id: run_id,
                        request_label: label.clone(),
                        status: ActionRunStatus::Failed,
                        record,
                    });
                }
                self.messages
                    .push(format!("Failed: {label} -> {}", error.message));
                self.open_latest_history_output();
                self.continue_action_queue();
            }
        }
    }

    fn apply_successful_action_effect(&mut self, effect: Option<ActionEffect>) {
        match effect {
            Some(ActionEffect::ColorMode(mode)) => {
                self.snapshot.color_mode = mode;
                self.messages
                    .push(format!("Cockpit option applied: color {mode}"));
            }
            Some(ActionEffect::DefaultAgent(agent)) => {
                let agent_options = self
                    .snapshot
                    .workflow
                    .agent
                    .get_or_insert_with(dw_config::AgentOptions::default);
                agent_options.default = agent.to_string();
                self.messages
                    .push(format!("Cockpit option applied: agent {agent}"));
            }
            Some(ActionEffect::Root(root)) => {
                self.root_override = Some(root.clone());
                self.snapshot.root = root.clone();
                self.messages
                    .push(format!("Cockpit option applied: root {root}"));
            }
            Some(ActionEffect::InitializedRoot(root)) => {
                self.root_override = Some(root.clone());
                self.snapshot.root = root.clone();
                self.snapshot.needs_init = false;
                self.messages
                    .push(format!("DevWorkflow root initialized: {root}"));
            }
            None => {}
        }
    }

    fn continue_action_queue(&mut self) {
        if let Some((run_id, label)) = self.background.start_next_action() {
            self.history.start_running(run_id, label.clone());
            self.messages
                .push(format!("Next background launch: {label}"));
        } else if self.reload_after_action_queue {
            self.reload_after_action_queue = false;
            self.reload();
        }
    }

    fn reload(&mut self) {
        if self.snapshot.needs_init {
            self.messages
                .push("Init required before data can be loaded.".into());
            return;
        }
        if self.background.start_snapshot(self.root_override.clone()) {
            self.reload_assigned_after_snapshot = true;
            self.reload_pull_requests_after_snapshot = true;
            self.messages
                .push("Reloading snapshot, work items and PRs in the background...".into());
        } else {
            self.messages.push("Reload already running.".into());
        }
    }

    fn accept_snapshot_reload(&mut self, snapshot: TuiSnapshot) {
        self.snapshot = snapshot;
        self.clamp_after_snapshot_reload();
        let should_load_assigned = self.reload_assigned_after_snapshot;
        let should_load_pull_requests = self.reload_pull_requests_after_snapshot;
        self.reload_assigned_after_snapshot = false;
        self.reload_pull_requests_after_snapshot = false;
        if self.snapshot.needs_init {
            self.messages
                .push("DevWorkflow root is not initialized. Init is required.".into());
            return;
        }
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
        self.close_menu_help_modals();
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
        self.close_menu_help_modals();
        self.messages.push("Action composer opened.".into());
    }

    fn open_db_query_form(&mut self) {
        self.open_database_form(FormTemplate::DbQuery, "Guided DB query opened.");
    }

    fn open_db_describe_form(&mut self) {
        self.open_database_form(FormTemplate::DbDescribe, "Guided DB describe opened.");
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
                    "Project" => {
                        field.value = database
                            .project
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    }
                    "Database" => field.value = database.key.to_string(),
                    _ => {}
                }
            }
        }
        self.form = Some(form);
        self.filter_active = false;
        self.confirmation = None;
        self.close_menu_help_modals();
        self.messages.push(message.into());
    }

    fn open_start_pr_form(&mut self) {
        let Some(item) = self.snapshot.pull_requests.get(self.selected_pull_request) else {
            self.messages
                .push(self.selected_pull_request_action_error(PullRequestAction::StartPreview));
            return;
        };
        let Some(pull_request_id) = item.pull_request_id.clone() else {
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
                "Project" => field.value = item.project.to_string(),
                "Repository" => field.value = item.repository.to_string(),
                _ => {}
            }
        }
        self.form = Some(form);
        self.filter_active = false;
        self.confirmation = None;
        self.close_menu_help_modals();
        self.messages.push(format!(
            "Guided PR workspace form opened for #{}.",
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
        let work_item_id = item.id.clone();

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
            TuiActionRequest::AdoSetState(args) => Some(args.state.to_string()),
            _ => None,
        });
        for field in &mut form.fields {
            match field.label.as_str() {
                "Work item IDs" => field.value = item.id.to_string(),
                "Project" => field.value = project.key.to_string(),
                "Destination state" => {
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
        self.close_menu_help_modals();
        self.messages.push(format!(
            "Guided ADO state change opened for #{work_item_id}."
        ));
    }

    fn push_modal(&mut self, modal: ModalKind) {
        self.modal_stack.retain(|existing| *existing != modal);
        self.modal_stack.push(modal);
    }

    fn close_top_modal(&mut self) {
        let Some(modal) = self.modal_stack.pop() else {
            if self.detail.is_some() {
                self.detail = None;
            } else if self.history.output_open {
                self.history.close_output();
            } else if self.state_open {
                self.state_open = false;
                self.state_scroll = 0;
            } else if self.help_open {
                self.help_open = false;
            } else if self.options_open {
                self.options_open = false;
            }
            return;
        };
        match modal {
            ModalKind::Menu => self.options_open = false,
            ModalKind::MenuSection => {}
            ModalKind::Help => self.help_open = false,
            ModalKind::State => {
                self.state_open = false;
                self.state_scroll = 0;
            }
            ModalKind::History => self.history.close_output(),
            ModalKind::Detail => self.detail = None,
            ModalKind::ActionProgress => self.action_progress = None,
        }
    }

    fn sync_closed_modal(&mut self, modal: ModalKind) {
        self.modal_stack.retain(|existing| *existing != modal);
    }

    fn close_menu_help_modals(&mut self) {
        self.options_open = false;
        self.help_open = false;
        self.sync_closed_modal(ModalKind::Menu);
        self.sync_closed_modal(ModalKind::MenuSection);
        self.sync_closed_modal(ModalKind::Help);
    }

    #[cfg(test)]
    pub(crate) fn modal_stack_labels(&self) -> Vec<&'static str> {
        self.modal_stack
            .iter()
            .map(|modal| match modal {
                ModalKind::Menu => "menu",
                ModalKind::MenuSection => "menu-section",
                ModalKind::Help => "help",
                ModalKind::State => "state",
                ModalKind::History => "history",
                ModalKind::Detail => "detail",
                ModalKind::ActionProgress => "action-progress",
            })
            .collect()
    }

    fn open_detail_panel(&mut self, detail: DetailPanel) {
        self.detail = Some(detail);
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.push_modal(ModalKind::Detail);
    }

    fn open_action_progress_modal(&mut self, run_id: ActionRunId) {
        self.action_progress = Some(run_id);
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.push_modal(ModalKind::ActionProgress);
    }

    fn close_action_progress_modal(&mut self, run_id: ActionRunId) {
        if self.action_progress == Some(run_id) {
            self.action_progress = None;
            self.sync_closed_modal(ModalKind::ActionProgress);
        }
    }

    pub(crate) fn open_options(&mut self) {
        self.options_open = true;
        self.selected_menu_section = self
            .selected_menu_section
            .min(MENU_SECTIONS.len().saturating_sub(1));
        self.selected_option = self
            .selected_option
            .min(self.current_menu_item_count().saturating_sub(1));
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.push_modal(ModalKind::Menu);
        self.messages.push("Menu opened.".into());
    }

    fn open_help_modal(&mut self) {
        self.help_open = true;
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.push_modal(ModalKind::Help);
    }

    fn open_state_modal(&mut self) {
        self.state_open = true;
        self.state_scroll = 0;
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
        self.push_modal(ModalKind::State);
    }

    fn open_history_output(&mut self) {
        self.history.open_output();
        self.close_overlays_for_history();
        self.push_modal(ModalKind::History);
    }

    fn open_latest_history_output(&mut self) {
        self.history.open_output();
        self.close_overlays_for_history();
        self.push_modal(ModalKind::History);
    }

    fn close_overlays_for_history(&mut self) {
        self.filter_active = false;
        self.confirmation = None;
        self.form = None;
    }

    fn start_assigned_load(&mut self) {
        if self.background.start_assigned(&mut self.snapshot) {
            self.messages
                .push("Loading your work items in the background...".into());
        }
    }

    fn restart_assigned_load(&mut self) {
        self.background.restart_assigned(&mut self.snapshot);
        self.messages
            .push("Reloading your work items in the background...".into());
    }

    fn start_pull_requests_load(&mut self) {
        if self.background.start_pull_requests(&mut self.snapshot) {
            self.messages
                .push("Loading PRs in the background...".into());
        }
    }

    fn restart_pull_requests_load(&mut self) {
        self.background.restart_pull_requests(&mut self.snapshot);
        self.messages
            .push("Reloading PRs in the background...".into());
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
        if matches!(self.modal_stack.last(), Some(ModalKind::Menu)) {
            self.selected_menu_section = self.selected_menu_section.saturating_sub(1);
            self.selected_option = self
                .selected_option
                .min(self.current_menu_item_count().saturating_sub(1));
        } else {
            self.selected_option = self.selected_option.saturating_sub(1);
        }
    }

    fn move_option_down(&mut self) {
        if matches!(self.modal_stack.last(), Some(ModalKind::Menu)) {
            if !MENU_SECTIONS.is_empty() {
                self.selected_menu_section =
                    (self.selected_menu_section + 1).min(MENU_SECTIONS.len() - 1);
                self.selected_option = self
                    .selected_option
                    .min(self.current_menu_item_count().saturating_sub(1));
            }
        } else if self.current_menu_item_count() > 0 {
            self.selected_option =
                (self.selected_option + 1).min(self.current_menu_item_count() - 1);
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
        View::Dashboard | View::Composer => true,
        View::Workspaces => action.is_workspace_action(),
        View::PullRequests => action.is_workspace_action() || action.is_ado_action(),
        View::Ado => action.is_ado_action(),
        View::Db => action.is_db_action(),
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
        format!("My work items loaded: {items} work item(s).")
    } else {
        format!("My work items loaded: {items} work item(s), {errors} project error(s).")
    }
}

fn pull_request_load_summary(items: &[TuiPullRequest]) -> String {
    let active = items
        .iter()
        .filter(|item| item.pull_request_id.is_some())
        .count();
    let errors = items.iter().filter(|item| item.error.is_some()).count();
    if errors == 0 {
        format!("PR context loaded: {active} active PR(s).")
    } else {
        format!("PR context loaded: {active} active PR(s), {errors} repository error(s).")
    }
}

fn snapshot_reload_summary(snapshot: &TuiSnapshot) -> String {
    format!(
        "Snapshot reloaded: {} project(s), {} workspace(s), {} database(s), {} prune.",
        snapshot.project_count(),
        snapshot.workspaces.len(),
        snapshot.database_count(),
        snapshot.prune_candidates
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::RecordedActionEvent;
    use crate::model::AdoAssignedProject;
    use dw_app::{AdoActionResult, AppActionResult, DwActionResult};
    use dw_core::{DwActionEvent, PullRequestId, TaskActionEvent};
    use std::fs;

    fn initialized_root() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().join("config");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(config_dir.join("projects.json"), "{}").expect("projects config");
        fs::write(config_dir.join("workflow.json"), "{}").expect("workflow config");
        fs::write(config_dir.join("databases.json"), "{}").expect("databases config");
        temp
    }

    fn captured_version_result(label: &str, status: &str) -> runner::CapturedActionRunResult {
        runner::CapturedActionRunResult {
            display_label: label.into(),
            status_label: status.into(),
            success: true,
            events: Vec::new(),
            result: DwActionResult::App(AppActionResult::Version {
                version: "2026.07.04".into(),
            }),
        }
    }

    fn captured_version_result_with_event(
        label: &str,
        status: &str,
        pull_request_id: &str,
    ) -> runner::CapturedActionRunResult {
        let mut result = captured_version_result(label, status);
        result.events.push(DwActionEvent::Task(
            TaskActionEvent::ResolvingPullRequestWorkItems {
                pull_request_id: PullRequestId::from(pull_request_id),
            },
        ));
        result
    }

    fn captured_assigned_result(label: &str) -> runner::CapturedActionRunResult {
        runner::CapturedActionRunResult {
            display_label: label.into(),
            status_label: "ok".into(),
            success: true,
            events: Vec::new(),
            result: DwActionResult::Ado(AdoActionResult::Assigned(
                dw_ado_commands::commands::assigned::AssignedReport {
                    root: "/tmp/dw".into(),
                    project: "ha".into(),
                    top: 20,
                    include_final_states: false,
                    group_by_parent: false,
                    items: vec![dw_ado::WorkItemSnapshot {
                        id: "55264".into(),
                        kind: Some("Task".into()),
                        state: Some("Active".into()),
                        title: Some("Transmission automatique".into()),
                        url: None,
                    }],
                    groups: Vec::new(),
                    events: Vec::new(),
                },
            )),
        }
    }

    #[test]
    fn key_release_events_are_ignored() {
        assert!(should_handle_key_event(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE
        )));
        assert!(!should_handle_key_event(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
        assert!(!should_handle_key_event(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    }

    #[test]
    fn workspace_creation_execute_blocks_until_done() {
        let args = dw_task::start::StartArgs {
            work_item_ids: vec![dw_core::WorkItemId::from("42")],
            root: Some(dw_core::DevWorkflowRoot::from("/tmp/dw")),
            project: Some(dw_core::ProjectKey::from("ha")),
            repositories: Vec::new(),
            task: None,
            type_name: None,
            slug: None,
            skip_ado: false,
            with_active_children: false,
            create_child_tasks: false,
            mode: dw_core::ExecutionMode::Preview,
        };
        let preview = TuiAction {
            label: "Preview".into(),
            request: TuiActionRequest::TaskStart(args.clone()),
            description: "preview".into(),
            kind: ActionRisk::Safe,
        };
        let mut execute = preview.clone();
        execute.request = TuiActionRequest::TaskStart(dw_task::start::StartArgs {
            mode: dw_core::ExecutionMode::Execute,
            ..args
        });

        assert!(!action_blocks_until_done(&preview));
        assert!(action_blocks_until_done(&execute));
    }

    #[test]
    fn action_progress_modal_tracks_running_action() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let run_id = ActionRunId::new(7);

        app.open_action_progress_modal(run_id);

        assert_eq!(app.action_progress, Some(run_id));
        assert_eq!(app.modal_stack_labels(), vec!["action-progress"]);

        app.close_action_progress_modal(run_id);

        assert_eq!(app.action_progress, None);
        assert!(app.modal_stack_labels().is_empty());
    }

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
        let root = initialized_root();
        let app = App::new(Some(root.path().display().to_string()));

        assert!(app.snapshot_loading());
        assert_eq!(app.snapshot.root, root.path().display().to_string());
        assert!(app.snapshot.workspaces.is_empty());
        assert!(
            app.messages
                .iter()
                .any(|message| message.contains("Loading snapshot, work items and PRs"))
        );
        assert!(app.reload_assigned_after_snapshot);
        assert!(app.reload_pull_requests_after_snapshot);
    }

    #[test]
    fn loading_snapshot_does_not_surface_doctor_attention() {
        let root = initialized_root();
        let app = App::new(Some(root.path().display().to_string()));

        let items = app.cockpit_items();

        assert!(app.snapshot.config_doctor.checks.is_empty());
        assert!(
            items
                .iter()
                .all(|item| item.title != "Configuration needs attention")
        );
    }

    #[test]
    fn background_status_lines_explain_loading_and_idle_work() {
        let root = initialized_root();
        let app = App::new(Some(root.path().display().to_string()));

        let lines = app.background_status_lines();

        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("Snapshot: loading"))
        );
        assert!(lines.contains(&"My work items: not loaded".into()));
        assert!(lines.contains(&"PRs: not loaded".into()));
        assert!(lines.contains(&"Action: none".into()));
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
            request: TuiActionRequest::Doctor { fix: false },
            description: "Doctor".into(),
            kind: ActionRisk::Safe,
        };
        let third = TuiAction {
            label: "Quick start".into(),
            request: TuiActionRequest::Guide,
            description: "Show the startup path".into(),
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
            ["Next: Doctor", "Then: 1 other action(s)"]
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
            pull_request_id: Some(dw_core::PullRequestId::from("123")),
            title: Some("Demo".into()),
            is_draft: false,
            work_item_ids: vec!["42".into()],
            url: None,
            error: None,
        }];

        let lines = app.background_status_lines();

        assert!(lines.contains(&"Snapshot: ready".into()));
        assert!(lines.contains(&"My work items: 1 items".into()));
        assert!(lines.contains(&"PRs: 1 active".into()));
    }

    #[test]
    fn background_status_lines_surface_current_operation_event() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let latest = DwActionEvent::Started {
            action_id: "task.finish".into(),
        };
        app.history
            .start_running(ActionRunId::new(1), ActionRunLabel::new("Task finish"));
        app.history
            .append_running_event(ActionRunId::new(1), latest.clone());

        let lines = app.background_status_lines();
        let expected = dw_ui::action_event_line(&latest);

        assert!(
            lines
                .iter()
                .any(|line| line == &format!("Action: Task finish -> {expected}"))
        );
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
    fn workspace_view_jk_moves_workspace_selection_not_hidden_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![
            workspace("/tmp/ws-one", "one"),
            workspace("/tmp/ws-two", "two"),
        ];
        app.set_view(View::Workspaces);

        assert!(
            app.handle_view_navigation_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))
        );
        assert_eq!(app.selected_workspace, 1);
        assert_eq!(app.selected_action, 0);

        assert!(
            app.handle_view_navigation_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE))
        );
        assert_eq!(app.selected_workspace, 0);
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn composer_does_not_capture_tab_view_navigation() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.set_view(View::Composer);
        app.action_form.begin_editing(&app.snapshot);
        let selected_field = app.action_form.selected_field;

        assert!(!app.handle_view_navigation_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)));
        assert!(
            !app.handle_view_navigation_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
        );
        assert_eq!(app.action_form.selected_field, selected_field);
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
        assert!(labels.iter().any(|label| label.contains("Check")));
    }

    #[test]
    fn workspace_teardown_shortcut_action_is_rooted_and_confirmed() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let root = app.snapshot.root.clone();
        app.snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];

        let action =
            actions::selected_workspace_action(&app.snapshot, 0, WorkspaceAction::TeardownExecute)
                .expect("teardown action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        match action.request {
            TuiActionRequest::TaskTeardown(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws-one")
                );
                assert_eq!(
                    args.root.as_ref().map(dw_core::DevWorkflowRoot::as_str),
                    Some(root.as_str())
                );
                assert!(args.mode.executes());
                assert!(args.yes);
            }
            _ => panic!("expected teardown request"),
        }
    }

    #[test]
    fn workspace_finish_execute_action_is_rooted_and_confirmed() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let root = app.snapshot.root.clone();
        app.snapshot.workspaces = vec![workspace("/tmp/ws-one", "one")];

        let action =
            actions::selected_workspace_action(&app.snapshot, 0, WorkspaceAction::FinishExecute)
                .expect("finish action");

        assert!(matches!(action.kind, ActionRisk::Destructive));
        match action.request {
            TuiActionRequest::TaskFinish(args) => {
                assert_eq!(
                    args.workspace.as_ref().map(dw_core::WorkspacePath::as_str),
                    Some("/tmp/ws-one")
                );
                assert_eq!(
                    args.root.as_ref().map(dw_core::DevWorkflowRoot::as_str),
                    Some(root.as_str())
                );
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
                pull_request_id: Some(dw_core::PullRequestId::from("12")),
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
            "My work items loaded: 1 work item(s), 1 project error(s)."
        );
        assert_eq!(
            pull_request_load_summary(&prs),
            "PR context loaded: 1 active PR(s), 1 repository error(s)."
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
            "My work items are not loaded yet: wait for preload or reload."
        );

        app.start_assigned_load();
        assert_eq!(
            app.selected_ado_action_error(),
            "My work items are loading in the background; you can keep using the TUI."
        );
    }

    #[test]
    fn pull_request_action_error_explains_loading_state() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = false;

        assert_eq!(
            app.selected_pull_request_action_error(PullRequestAction::DiffPreview),
            "PRs are not loaded yet: wait for preload or reload."
        );

        app.start_pull_requests_load();
        assert_eq!(
            app.selected_pull_request_action_error(PullRequestAction::DiffPreview),
            "PRs are loading; you can keep navigating."
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
            .find(|item| item.title.contains("#42"))
            .expect("cockpit PR item");
        assert_eq!(item.section, "To do");
        assert!(matches!(
            item.primary_action.request,
            TuiActionRequest::TaskStartPr(_)
        ));
    }

    #[test]
    fn pull_request_footer_contains_shortcuts_not_selected_context() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::PullRequests;
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![pull_request("ha", "back", 55265)];

        let preview = crate::ui_text::shortcut_bar_line(&app);

        assert!(preview.contains("diff [d]"));
        assert!(!preview.contains("target"));
        assert!(!preview.contains("#55265"));
        assert!(!preview.contains("back ·"));
        assert!(!preview.contains("Preview PR workspace"));
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
                .any(|message| message.contains("missing URL"))
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
            pull_request_id: Some(dw_core::PullRequestId::from("12")),
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
                .any(|message| message.contains("missing PR URL"))
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
                .any(|field| field.label == "Project" && field.value == "ha")
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
                .any(|field| field.label == "Project" && field.value == "ha")
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
        assert_eq!(field_value(&form, "Work item IDs"), Some("42"));
        assert_eq!(field_value(&form, "Project"), Some("ha"));
        assert_eq!(
            field_value(&form, "Destination state"),
            Some("En réalisation")
        );
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
        assert_eq!(field_value(&form, "Project"), Some("ops"));
        assert_eq!(field_value(&form, "Repository"), Some("tools"));
        assert!(
            app.messages
                .iter()
                .any(|message| message.contains("Guided PR workspace form"))
        );
    }

    #[test]
    fn options_selection_moves_and_clamps() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.selected_menu_section = usize::MAX;

        app.open_options();
        assert_eq!(app.selected_menu_section, MENU_SECTIONS.len() - 1);

        app.move_option_up();
        assert_eq!(app.selected_menu_section, MENU_SECTIONS.len() - 2);

        app.selected_menu_section = 0;
        app.move_option_up();
        assert_eq!(app.selected_menu_section, 0);

        app.move_option_down();
        assert_eq!(app.selected_menu_section, 1);

        app.open_selected_menu_section();
        app.selected_option = 0;
        app.move_option_down();
        assert_eq!(app.selected_option, 1);
    }

    #[tokio::test]
    async fn config_show_quick_option_uses_core_detail_panel() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let root = app.snapshot.root.clone();

        assert!(
            app.run_core_quick_option(QuickOptionAction::ConfigShow)
                .await
                .expect("quick option")
        );

        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Effective configuration");
        let crate::model::DetailPanelContent::ConfigShow(report) = detail.content else {
            panic!("expected config show panel");
        };
        assert_eq!(report.root, root.as_str());
        assert!(app.history.entries.is_empty());
    }

    #[tokio::test]
    async fn config_detail_opened_from_menu_returns_to_menu_when_closed() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.open_options();

        assert!(app.options_open);
        assert!(
            app.run_core_quick_option(QuickOptionAction::ConfigShow)
                .await
                .expect("quick option")
        );
        assert!(app.options_open);
        assert!(app.detail.is_some());

        app.handle_detail_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("close detail");

        assert!(app.options_open);
        assert!(app.detail.is_none());
    }

    #[tokio::test]
    async fn config_doctor_quick_option_refreshes_snapshot_and_uses_core_detail_panel() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let root = app.snapshot.root.clone();
        app.snapshot.config_doctor = dw_config::ConfigDoctorReport {
            root: "/tmp/old".into(),
            passed: true,
            checks: vec![],
        };

        assert!(
            app.run_core_quick_option(QuickOptionAction::ConfigDoctor)
                .await
                .expect("quick option")
        );

        assert_eq!(app.snapshot.config_doctor.root, root.as_str());
        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Configuration doctor");
        let crate::model::DetailPanelContent::ConfigDoctor(report) = detail.content else {
            panic!("expected config doctor panel");
        };
        assert_eq!(report.root, root.as_str());
        assert_eq!(report, app.snapshot.config_doctor);
        assert!(app.history.entries.is_empty());
    }

    #[tokio::test]
    async fn agent_doctor_quick_option_uses_core_detail_panel() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        assert!(
            app.run_core_quick_option(QuickOptionAction::AgentDoctor)
                .await
                .expect("quick option")
        );

        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Agent doctor");
        let crate::model::DetailPanelContent::AgentDoctor(report) = detail.content else {
            panic!("expected agent doctor panel");
        };
        assert!(!report.checks.is_empty());
        assert!(app.history.entries.is_empty());
    }

    #[tokio::test]
    async fn guide_action_uses_detail_panel_not_history() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let action = actions::option_action(&app.snapshot.root, QuickOptionAction::Guide);

        assert!(
            app.run_inline_detail_action(&action)
                .await
                .expect("inline detail")
        );

        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "DevWorkflow guide");
        let crate::model::DetailPanelContent::Guide = detail.content else {
            panic!("expected guide panel");
        };
        assert!(app.history.entries.is_empty());
    }

    #[test]
    fn history_output_modal_opens_scrolls_and_closes() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.push(RunHistoryEntry {
            id: ActionRunId::new(1),
            request_label: ActionRunLabel::new("Doctor"),
            status: ActionRunStatus::Succeeded,
            record: ActionRunRecord::Running {
                events: vec![
                    RecordedActionEvent::fixed(
                        "2026-07-08 10:00:00Z",
                        dw_core::DwActionEvent::Started {
                            action_id: "one".into(),
                        },
                    ),
                    RecordedActionEvent::fixed(
                        "2026-07-08 10:00:01Z",
                        dw_core::DwActionEvent::Started {
                            action_id: "two".into(),
                        },
                    ),
                    RecordedActionEvent::fixed(
                        "2026-07-08 10:00:02Z",
                        dw_core::DwActionEvent::Started {
                            action_id: "three".into(),
                        },
                    ),
                ],
            },
        });
        app.history.push(RunHistoryEntry {
            id: ActionRunId::new(2),
            request_label: ActionRunLabel::new("Version"),
            status: ActionRunStatus::Succeeded,
            record: ActionRunRecord::Completed {
                events: Vec::new(),
                result: Box::new(DwActionResult::App(AppActionResult::Version {
                    version: "version".into(),
                })),
            },
        });

        app.open_history_output();
        assert!(app.history.output_open);
        assert_eq!(app.history.selected_entry, 1);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .expect("down");
        assert_eq!(app.history.output_scroll, 1);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE))
            .expect("previous");
        assert_eq!(app.history.selected_entry, 0);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))
            .expect("right");
        assert_eq!(app.history.selected_entry, 1);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))
            .expect("left");
        assert_eq!(app.history.selected_entry, 0);

        app.handle_history_output_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE))
            .expect("end");
        assert_eq!(
            app.history.output_scroll,
            crate::ui_text::history_journal_lines(&app)
                .len()
                .saturating_sub(1)
        );

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
    fn modal_stack_returns_to_menu_after_nested_history_closes() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.open_options();
        app.open_selected_menu_section();
        app.open_history_output();

        assert_eq!(
            app.modal_stack_labels(),
            vec!["menu", "menu-section", "history"]
        );
        assert!(app.options_open);
        assert!(app.history.output_open);

        app.handle_history_output_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("close history");

        assert_eq!(app.modal_stack_labels(), vec!["menu", "menu-section"]);
        assert!(app.options_open);
        assert!(!app.history.output_open);

        app.close_top_modal();

        assert_eq!(app.modal_stack_labels(), vec!["menu"]);
        assert!(app.options_open);

        app.close_top_modal();

        assert!(app.modal_stack.is_empty());
        assert!(!app.options_open);
    }

    #[test]
    fn history_shortcut_opens_empty_journal_modal() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        assert!(app.history.entries.is_empty());

        app.open_history_output();

        assert!(app.history.output_open);
        assert!(app.history.entries.is_empty());
    }

    #[test]
    fn help_opens_as_modal_without_changing_view() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Ado;

        app.open_help_modal();

        assert!(app.help_open);
        assert_eq!(app.view, View::Ado);
        app.handle_help_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE))
            .expect("close help");
        assert!(!app.help_open);
    }

    #[test]
    fn action_result_finishes_streaming_history_entry() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let run_id = ActionRunId::new(1);
        app.history
            .start_running(run_id, ActionRunLabel::new("Version"));
        app.history.append_running_event(
            run_id,
            dw_core::DwActionEvent::Task(dw_core::TaskActionEvent::ResolvingPullRequestWorkItems {
                pull_request_id: dw_core::PullRequestId::from("42"),
            }),
        );

        app.accept_action_result(
            run_id,
            ActionRunLabel::new("Version"),
            false,
            false,
            None,
            Ok(captured_version_result_with_event(
                "Version", "exit 0", "42",
            )),
        );

        assert_eq!(app.history.entries.len(), 1);
        let entry = app.history.selected_entry().expect("entry");
        assert_eq!(entry.status, ActionRunStatus::Succeeded);
        let preview_lines = crate::ui_text::history_entry_preview_lines(entry);
        assert!(
            preview_lines[0].ends_with(" | INF | task.pr.resolve.workitems | pull_request=#42")
        );
        assert_eq!(preview_lines[1], "Dev Workflow 2026.07.04");
    }

    #[test]
    fn report_action_result_opens_detail_panel_and_keeps_run_log() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let run_id = ActionRunId::new(1);
        app.history
            .start_running(run_id, ActionRunLabel::new("My work items · ha"));

        app.accept_action_result(
            run_id,
            ActionRunLabel::new("My work items · ha"),
            false,
            true,
            None,
            Ok(captured_assigned_result("My work items · ha")),
        );

        assert!(!app.history.output_open);
        let detail = app.detail.expect("detail panel");
        assert_eq!(detail.title(), "Result · My work items · ha");
        let crate::model::DetailPanelContent::ActionResult { result, .. } = detail.content else {
            panic!("expected action result panel");
        };
        let lines =
            dw_tui_adapter::render::action_result_lines(&result, &dw_ui::TerminalTheme::plain());
        assert!(lines.iter().any(|line| line.contains("#55264")));
        let entry = app.history.selected_entry().expect("entry");
        assert_eq!(entry.request_label.to_string(), "My work items · ha");
        assert!(
            crate::ui_text::history_entry_rendered_lines(entry)
                .iter()
                .any(|line| line.contains("#55264"))
        );
    }

    #[test]
    fn detail_panel_closes_even_when_modal_stack_is_desynced() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.detail = Some(DetailPanel::guide());
        app.modal_stack.clear();

        app.handle_detail_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("close detail");

        assert!(app.detail.is_none());
    }

    #[test]
    fn successful_option_action_updates_visible_snapshot_immediately() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            ActionRunId::new(1),
            ActionRunLabel::new("Color · always"),
            true,
            false,
            Some(ActionEffect::ColorMode(dw_core::ConfigColorMode::Always)),
            Ok(captured_version_result("Color · always", "exit 0")),
        );

        assert_eq!(app.snapshot.color_mode, dw_core::ConfigColorMode::Always);
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Cockpit option applied: color always")
        );
    }

    #[test]
    fn successful_agent_action_updates_visible_snapshot_immediately() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            ActionRunId::new(1),
            ActionRunLabel::new("Default agent · codex"),
            true,
            false,
            Some(ActionEffect::DefaultAgent(dw_core::Agent::Codex)),
            Ok(captured_version_result("Default agent · codex", "exit 0")),
        );

        assert_eq!(app.snapshot.default_agent(), dw_core::Agent::Codex);
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Cockpit option applied: agent codex")
        );
    }

    #[test]
    fn successful_agent_effect_normalizes_visible_agent() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            ActionRunId::new(1),
            ActionRunLabel::new("Default agent · CODEX-CLI"),
            true,
            false,
            Some(ActionEffect::DefaultAgent(dw_core::Agent::CodexCli)),
            Ok(captured_version_result(
                "Default agent · CODEX-CLI",
                "exit 0",
            )),
        );

        assert_eq!(app.snapshot.default_agent(), dw_core::Agent::CodexCli);
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Cockpit option applied: agent codex-cli")
        );
    }

    #[test]
    fn successful_set_root_action_updates_visible_root_immediately() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            ActionRunId::new(1),
            ActionRunLabel::new("Root · /tmp/new-root"),
            true,
            false,
            Some(ActionEffect::Root("/tmp/new-root".into())),
            Ok(captured_version_result("Root · /tmp/new-root", "exit 0")),
        );

        assert_eq!(app.root_override.as_deref(), Some("/tmp/new-root"));
        assert_eq!(app.snapshot.root, "/tmp/new-root");
        assert!(
            app.messages
                .iter()
                .any(|message| message == "Cockpit option applied: root /tmp/new-root")
        );
    }

    #[test]
    fn mutating_action_result_triggers_snapshot_reload_after_queue_drains() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        app.accept_action_result(
            ActionRunId::new(1),
            ActionRunLabel::new("Task sync · /tmp/ws"),
            true,
            false,
            None,
            Ok(captured_version_result("Task sync · /tmp/ws", "exit 0")),
        );

        assert!(app.messages.iter().any(
            |message| message == "Reloading snapshot, work items and PRs in the background..."
        ));
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
                .any(|message| message.starts_with("Snapshot reloaded:"))
        );
    }

    #[test]
    fn snapshot_reload_summary_includes_operational_counts() {
        let mut snapshot = TuiSnapshot::loading(Some("/tmp/missing-dw-root"));
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

    #[test]
    fn input_prompt_cancel_drops_response_without_submitting_selection() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut prompt = TuiInputPrompt::new(
            ActionRunId::new(7),
            InputRequest::SelectOne {
                id: "assigned-work-item".into(),
                label: "Work item".into(),
                help: None,
                choices: vec![dw_core::PromptChoice::new("42", "#42 Demo")],
            },
            sender,
        );

        prompt.cancel();

        assert!(receiver.recv().is_err());
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
            pull_request_id: Some(dw_core::PullRequestId::from(pull_request_id.to_string())),
            title: Some("Demo".into()),
            is_draft: false,
            work_item_ids: vec![dw_core::WorkItemId::from(pull_request_id.to_string())],
            url: None,
            error: None,
        }
    }

    fn workspace(path: &str, slug: &str) -> dw_workspace::TaskListItem {
        dw_workspace::TaskListItem {
            path: path.into(),
            project: "ha".into(),
            work_item_id: "42".into(),
            work_items: vec![dw_workspace::WorkspaceWorkItem {
                id: "42".into(),
                kind: Some("User Story".into()),
                title: Some("Demo".into()),
                state: Some("Active".into()),
            }],
            task_id: None,
            all_known_work_item_ids: vec!["42".into()],
            kind: "feature".into(),
            slug: slug.into(),
            branch_name: format!("feature/42-{slug}").into(),
            created_at: "2026-07-04T00:00:00Z".into(),
            work_item_type: Some("User Story".into()),
            work_item_title: Some("Demo".into()),
            work_item_state: Some("Active".into()),
            repositories: vec!["front".into()],
        }
    }
}
