use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::actions::{AdoItemAction, PullRequestAction};
use crate::app::{App, MENU_SECTIONS, MenuSection, ModalKind, TuiInputPrompt};
use crate::background::BackgroundKind;
use crate::form::{FieldKind, FormMode, FormState, FormTemplate};
use crate::history::{ActionRunRecord, JournalLogLevel};
use crate::model::{
    ActionRisk, CockpitSeverity, DetailPanelContent, TuiAction, View, WorkspaceAction,
};
use crate::ui_text::{
    JournalLine, action_builder_preview_lines, confirmation_lines, form_preview_lines,
    guide_detail_lines, help_lines, history_journal_line_items, option_active,
    options_summary_lines, state_modal_lines,
};

const MAX_ACTION_BAR_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionIntent {
    Primary,
    Review,
    External,
    Dangerous,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionButton {
    label: String,
    key: &'static str,
    intent: ActionIntent,
    enabled: bool,
}

impl ActionButton {
    fn new(label: impl Into<String>, key: &'static str, intent: ActionIntent) -> Self {
        Self {
            label: label.into(),
            key,
            intent,
            enabled: true,
        }
    }

    fn disabled(label: impl Into<String>, key: &'static str) -> Self {
        Self {
            label: label.into(),
            key,
            intent: ActionIntent::Disabled,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusBadge {
    label: String,
    status: &'static str,
    color: Color,
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let footer_height = footer_height(app, area.width);
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(12),
            Constraint::Length(footer_height),
        ])
        .split(area);

    render_top_bar(frame, root[0], app);
    render_top_separator(frame, root[1]);
    render_body(frame, root[2], app);
    render_footer(frame, root[3], app);

    if app.snapshot.needs_init {
        render_init_required(frame, area, app);
        return;
    }

    if let Some(prompt) = &app.input_prompt {
        render_input_prompt(frame, area, prompt);
        return;
    }

    if let Some(action) = &app.confirmation {
        render_confirmation(frame, area, action);
    }

    if app.form.is_some() {
        render_form(frame, area, app);
    }

    if app.modal_stack.is_empty() {
        if app.options_open {
            render_options(frame, area, app);
        }
        if app.help_open {
            render_help_modal(frame, area);
        }
        if app.detail.is_some() {
            render_detail_panel(frame, area, app);
        }
        if app.history.output_open {
            render_history_output(frame, area, app);
        }
        if app.state_open {
            render_state_modal(frame, area, app);
        }
    } else {
        for modal in &app.modal_stack {
            match modal {
                ModalKind::Menu => render_options(frame, area, app),
                ModalKind::MenuSection => render_menu_section(frame, area, app),
                ModalKind::Help => render_help_modal(frame, area),
                ModalKind::Detail => render_detail_panel(frame, area, app),
                ModalKind::History => render_history_output(frame, area, app),
                ModalKind::State => render_state_modal(frame, area, app),
                ModalKind::ActionProgress => render_action_progress_modal(frame, area, app),
            }
        }
    }
}

fn render_top_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut spans = tab_spans(app);
    let status = top_status_text(app);
    let right = if status.is_empty() {
        app.snapshot.root.clone()
    } else {
        format!("{status}  {}", app.snapshot.root)
    };
    let content_width = spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum::<usize>();
    let right_width = right.chars().count();
    let padding = (area.width as usize)
        .saturating_sub(content_width + right_width)
        .max(1);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(right, Style::default().fg(Color::Gray)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black)),
        area,
    );
}

fn tab_spans(app: &App) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, view) in View::ALL.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        let style = if *view == app.view {
            Style::default().fg(Color::White)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        };
        spans.push(Span::styled(
            format!(" {} [{}] ", view.label(), index + 1),
            style,
        ));
    }
    spans
}

fn top_status_text(app: &App) -> String {
    let mut labels = Vec::new();
    if app.assigned_loading() {
        labels.push(loading_label(
            "ADO",
            app.loading_elapsed_label(BackgroundKind::Assigned),
        ));
    }
    if app.snapshot_loading() {
        labels.push(loading_label(
            "Snapshot",
            app.loading_elapsed_label(BackgroundKind::Snapshot),
        ));
    }
    if app.pull_requests_loading() {
        labels.push(loading_label(
            "PRs",
            app.loading_elapsed_label(BackgroundKind::PullRequests),
        ));
    }
    if app.action_loading() {
        labels.push(action_loading_label(
            app.latest_action_event_line(),
            app.loading_elapsed_label(BackgroundKind::Action),
        ));
    }
    let queued = app.pending_action_count();
    if queued > 0 {
        labels.push(format!("Queue {queued}"));
    }
    labels.join("  ")
}

fn loading_label(label: &'static str, elapsed: Option<String>) -> String {
    format!("{label} {}...", elapsed.unwrap_or_else(|| "<1s".into()))
}

fn action_loading_label(operation: Option<String>, elapsed: Option<String>) -> String {
    let elapsed = elapsed.unwrap_or_else(|| "<1s".into());
    match operation {
        Some(operation) => format!("Operation {elapsed}: {operation}"),
        None => format!("Operation {elapsed}..."),
    }
}

fn render_top_separator(frame: &mut Frame<'_>, area: Rect) {
    let width = area.width as usize;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(width),
            Style::default().fg(Color::DarkGray),
        )))
        .style(Style::default().bg(Color::Black)),
        area,
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.view {
        View::Dashboard => render_dashboard(frame, area, app),
        View::Workspaces => render_workspaces(frame, area, app),
        View::Ado => render_ado(frame, area, app),
        View::PullRequests => render_pull_requests(frame, area, app),
        View::Db => render_db(frame, area, app),
        View::Composer => render_action_builder_view(frame, area, app),
    }
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15), Constraint::Min(8)])
        .split(columns[0]);

    render_metrics(frame, left[0], app);
    render_workspace_summary(frame, left[1], app);
    render_cockpit(frame, columns[1], app);
}

fn render_cockpit(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = app.cockpit_items();
    let visible_height = list_visible_height(area);
    let offset = scroll_offset(app.selected_cockpit, visible_height);
    let rows = items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible_height)
        .map(|(index, item)| {
            let style = if index == app.selected_cockpit {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(cockpit_color(item.severity))
            };
            Row::new([
                item.section.into(),
                item.title.clone(),
                item.status.clone(),
                item.primary_action.display_label(),
                item.subtitle.clone(),
            ])
            .style(style)
        });
    frame.render_widget(
        Table::new(rows, cockpit_table_constraints())
            .header(
                Row::new(["Section", "Subject", "Status", "Operation", "Context"]).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(
                Block::default()
                    .title("Cockpit · Enter runs the primary operation")
                    .borders(Borders::ALL),
            ),
        area,
    );
}

fn cockpit_table_constraints() -> [Constraint; 5] {
    [
        Constraint::Length(12),
        Constraint::Min(28),
        Constraint::Length(14),
        Constraint::Length(22),
        Constraint::Length(28),
    ]
}

fn cockpit_color(severity: CockpitSeverity) -> Color {
    match severity {
        CockpitSeverity::Normal => Color::White,
        CockpitSeverity::Attention => Color::Yellow,
        CockpitSeverity::Blocked => Color::LightRed,
    }
}

fn render_metrics(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default().title("Readiness").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let workspace_total = app.snapshot.workspaces.len().max(1) as u16;
    let prune_ratio = ((app.snapshot.prune_candidates as u16) * 100 / workspace_total).min(100);
    frame.render_widget(
        Paragraph::new(status_badge_line(&readiness_badges(app))),
        rows[0],
    );
    let lines = [
        metric_line(
            "Projects",
            app.snapshot.project_count().to_string(),
            Color::LightBlue,
        ),
        metric_line(
            "Workspaces",
            app.snapshot.workspaces.len().to_string(),
            Color::LightGreen,
        ),
        metric_line(
            "Work items",
            app.snapshot.assigned_count().to_string(),
            Color::Yellow,
        ),
        metric_line(
            "Active PRs",
            app.snapshot
                .pull_requests
                .iter()
                .filter(|item| item.pull_request_id.is_some())
                .count()
                .to_string(),
            Color::Cyan,
        ),
        metric_line(
            "Cleanup",
            app.snapshot.prune_candidates.to_string(),
            Color::LightRed,
        ),
        metric_line(
            "DB",
            app.snapshot.database_count().to_string(),
            Color::Magenta,
        ),
        metric_line(
            "Agent",
            app.snapshot.default_agent().to_string(),
            Color::White,
        ),
    ];
    for (line, row) in lines.iter().zip(rows.iter().skip(1)) {
        frame.render_widget(Paragraph::new(line.clone()), *row);
    }
    frame.render_widget(
        Paragraph::new("workspaces ready to clean").style(Style::default().fg(Color::Yellow)),
        rows[7],
    );
    frame.render_widget(
        Gauge::default()
            .label("")
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(prune_ratio),
        rows[8],
    );
    for (line, row) in app
        .background_status_lines()
        .iter()
        .zip(rows.iter().skip(9))
    {
        frame.render_widget(
            Paragraph::new(line.as_str()).style(Style::default().fg(Color::Gray)),
            *row,
        );
    }
}

fn readiness_badges(app: &App) -> Vec<StatusBadge> {
    vec![
        readiness_badge(
            "Config",
            app.snapshot_loading(),
            true,
            app.snapshot.config_doctor.passed,
        ),
        readiness_badge(
            "ADO",
            app.assigned_loading(),
            app.snapshot.assigned_loaded,
            true,
        ),
        readiness_badge(
            "PRs",
            app.pull_requests_loading(),
            app.snapshot.pull_requests_loaded,
            true,
        ),
        readiness_badge("Action", app.action_loading(), !app.action_loading(), true),
    ]
}

fn readiness_badge(label: &'static str, loading: bool, loaded: bool, healthy: bool) -> StatusBadge {
    let (status, color) = if loading {
        ("loading", Color::Yellow)
    } else if !loaded {
        ("waiting", Color::DarkGray)
    } else if healthy {
        ("ready", Color::LightGreen)
    } else {
        ("check", Color::LightRed)
    };
    StatusBadge {
        label: label.into(),
        status,
        color,
    }
}

fn metric_line(label: &'static str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), Style::default().fg(Color::DarkGray)),
        Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_pull_requests(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(7)])
        .split(area);

    if !app.snapshot.pull_requests_loaded {
        if app.pull_requests_loading() {
            render_loading_panel(
                frame,
                chunks[0],
                "Pull requests",
                "Loading active PRs",
                app.loading_elapsed_label(BackgroundKind::PullRequests),
            );
        } else {
            render_empty_state(
                frame,
                chunks[0],
                "Pull requests",
                "PRs are waiting for the background preload.",
            );
        }
        render_pull_request_actions(frame, chunks[1], app);
        return;
    }

    if app.snapshot.pull_requests.is_empty() {
        frame.render_widget(
            Paragraph::new("No local workspace/repository can be linked to a PR.")
                .block(
                    Block::default()
                        .title("Pull requests")
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: true }),
            chunks[0],
        );
        render_pull_request_actions(frame, chunks[1], app);
        return;
    }

    let visible_height = table_visible_height(chunks[0]);
    let offset = scroll_offset(app.selected_pull_request, visible_height);
    let rows = app
        .snapshot
        .pull_requests
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible_height)
        .map(|(index, item)| {
            let style = if index == app.selected_pull_request {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else if item.error.is_some() {
                Style::default().fg(Color::LightRed)
            } else if item.pull_request_id.is_some() {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default().fg(Color::Gray)
            };
            Row::new([
                item.project.to_string(),
                item.repository.to_string(),
                item.pull_request_id
                    .as_ref()
                    .map(|id| format!("#{id}"))
                    .unwrap_or_else(|| "-".into()),
                if item.error.is_some() {
                    "error".into()
                } else if item.pull_request_id.is_some() {
                    if item.is_draft {
                        "draft".into()
                    } else {
                        "open".into()
                    }
                } else {
                    "missing".into()
                },
                if item.work_item_ids.is_empty() {
                    "-".into()
                } else {
                    item.work_item_ids
                        .iter()
                        .map(|id| format!("#{id}"))
                        .collect::<Vec<_>>()
                        .join(",")
                },
                if item.workspace.is_some() {
                    "yes".into()
                } else {
                    "no".into()
                },
                item.branch.clone(),
                item.title.clone().unwrap_or_default(),
            ])
            .style(style)
        });
    frame.render_widget(
        Table::new(rows, pull_request_table_constraints())
            .header(
                Row::new([
                    "Project",
                    "Repository",
                    "PR",
                    "State",
                    "Work items",
                    "Workspace",
                    "Branch",
                    "Title",
                ])
                .style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(
                Block::default()
                    .title("Pull requests")
                    .borders(Borders::ALL),
            ),
        chunks[0],
    );
    render_pull_request_actions(frame, chunks[1], app);
}

fn pull_request_table_constraints() -> [Constraint; 8] {
    [
        Constraint::Length(10),
        Constraint::Length(18),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(14),
        Constraint::Length(9),
        Constraint::Length(28),
        Constraint::Min(30),
    ]
}

fn render_pull_request_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default().title("PR selection").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(selected_pull_request_target_lines(app)),
        inner,
    );
}

fn selected_pull_request_target_lines(app: &App) -> Vec<Line<'static>> {
    let Some(item) = app.snapshot.pull_requests.get(app.selected_pull_request) else {
        return vec![target_line("Target", "No PR selected")];
    };
    let pr = item
        .pull_request_id
        .as_ref()
        .map(|id| format!("#{id}"))
        .unwrap_or_else(|| "missing PR".into());
    let title = item.title.clone().unwrap_or_else(|| "-".into());
    vec![
        target_line(
            "Target",
            format!("{} / {} · {}", item.project, item.repository, pr),
        ),
        target_line("Title", title),
    ]
}

fn pull_request_action_buttons(app: &App) -> Vec<ActionButton> {
    if app
        .snapshot
        .pull_requests
        .get(app.selected_pull_request)
        .is_none()
    {
        return vec![
            ActionButton::disabled("Prepare", "n"),
            ActionButton::disabled("Create", "x"),
            ActionButton::disabled("Changes", "c"),
            ActionButton::disabled("Diff", "d"),
        ];
    }

    vec![
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::StartPreview),
            "Prepare",
            "n",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::StartExecute),
            "Create",
            "x",
            ActionIntent::Primary,
        ),
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::OpenAgent),
            "Open agent",
            "o",
            ActionIntent::External,
        ),
        ActionButton::new("PR form", "N", ActionIntent::Review),
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::FinishPreview),
            "Finish preview",
            "f",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::FinishExecute),
            "Finish",
            "F",
            ActionIntent::Dangerous,
        ),
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::Changelog),
            "Changes",
            "c",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_pull_request_action_preview_for(PullRequestAction::DiffPreview),
            "Diff",
            "d",
            ActionIntent::Review,
        ),
        ActionButton::new("Open PR", "u", ActionIntent::External),
    ]
}

fn action_if_available(
    preview: Option<String>,
    label: &'static str,
    key: &'static str,
    intent: ActionIntent,
) -> ActionButton {
    if preview.is_some() {
        ActionButton::new(label, key, intent)
    } else {
        ActionButton::disabled(label, key)
    }
}

fn action_if_enabled(
    enabled: bool,
    label: &'static str,
    key: &'static str,
    intent: ActionIntent,
) -> ActionButton {
    if enabled {
        ActionButton::new(label, key, intent)
    } else {
        ActionButton::disabled(label, key)
    }
}

fn render_loading_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    label: &'static str,
    elapsed: Option<String>,
) {
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner);
    let elapsed = elapsed.unwrap_or_else(|| "<1s".into());
    frame.render_widget(
        Gauge::default()
            .label(format!("◆ {label} · {elapsed}"))
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .percent(72),
        rows[1],
    );
}

fn render_empty_state(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    message: &'static str,
) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("◇ ", Style::default().fg(Color::DarkGray)),
            Span::styled(message, Style::default().fg(Color::Gray)),
        ]))
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_workspace_summary(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = app
        .snapshot
        .workspaces
        .iter()
        .take(12)
        .map(|workspace| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    workspace.project.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(
                    workspace_work_items_label(workspace),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(workspace.slug.to_string(), Style::default().fg(Color::Gray)),
            ]))
        })
        .collect::<Vec<_>>();
    let items = if items.is_empty() {
        vec![ListItem::new("No task workspace detected.")]
    } else {
        items
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Recent workspaces")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_workspaces(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(8)])
        .split(area);
    let workspace_visible_height = table_visible_height(chunks[0]);
    let workspace_offset = scroll_offset(app.selected_workspace, workspace_visible_height);
    let rows = app
        .snapshot
        .workspaces
        .iter()
        .enumerate()
        .skip(workspace_offset)
        .take(workspace_visible_height)
        .map(|(index, workspace)| {
            let style = if index == app.selected_workspace {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Row::new([
                workspace.project.to_string(),
                workspace_work_items_label(workspace),
                workspace.kind.to_string(),
                workspace.slug.to_string(),
                workspace
                    .repositories
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            ])
            .style(style)
        });
    frame.render_widget(
        Table::new(rows, workspace_table_constraints())
            .header(
                Row::new(["Project", "Work items", "Type", "Slug", "Repositories"]).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(Block::default().title("Workspaces").borders(Borders::ALL)),
        chunks[0],
    );
    render_workspace_actions(frame, chunks[1], app);
}

fn workspace_table_constraints() -> [Constraint; 5] {
    [
        Constraint::Length(12),
        Constraint::Min(32),
        Constraint::Length(10),
        Constraint::Length(24),
        Constraint::Length(20),
    ]
}

fn workspace_work_items_label(workspace: &dw_workspace::TaskListItem) -> String {
    workspace
        .work_items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_workspace_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .title("Workspace selection")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(selected_workspace_target_lines(app)), inner);
}

fn selected_workspace_target_lines(app: &App) -> Vec<Line<'static>> {
    let Some(workspace) = app.snapshot.workspaces.get(app.selected_workspace) else {
        return vec![target_line("Target", "No workspace selected")];
    };

    vec![
        target_line(
            "Target",
            format!(
                "{} · {} · {}",
                workspace.project,
                workspace_work_items_label(workspace),
                workspace.slug
            ),
        ),
        target_line(
            "Path",
            if workspace.path.as_str().is_empty() {
                "-".into()
            } else {
                workspace.path.to_string()
            },
        ),
        target_line(
            "Branch",
            if workspace.branch_name.as_str().is_empty() {
                "-".into()
            } else {
                workspace.branch_name.to_string()
            },
        ),
        target_line(
            "Repos",
            if workspace.repositories.is_empty() {
                "-".into()
            } else {
                workspace
                    .repositories
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        ),
    ]
}

fn workspace_action_buttons(app: &App) -> Vec<ActionButton> {
    if app
        .snapshot
        .workspaces
        .get(app.selected_workspace)
        .is_none()
    {
        return vec![
            ActionButton::disabled("Open", "o"),
            ActionButton::disabled("Check", "p"),
            ActionButton::disabled("Finish", "F"),
            ActionButton::disabled("Remove", "x"),
        ];
    }

    vec![
        ActionButton::new("Open", "o", ActionIntent::External),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::Preflight),
            "Check",
            "p",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::Sync),
            "Sync",
            "s",
            ActionIntent::Primary,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::RepoLatest),
            "Latest",
            "l",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::HandoffValidate),
            "Handoff",
            "v",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::CommitPreview),
            "Commit",
            "c",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::FinishPreview),
            "Finish preview",
            "f",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::FinishExecute),
            "Finish",
            "F",
            ActionIntent::Dangerous,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::TeardownPreview),
            "Remove preview",
            "t",
            ActionIntent::Review,
        ),
        action_if_available(
            app.selected_workspace_action_preview_for(WorkspaceAction::TeardownExecute),
            "Remove",
            "x",
            ActionIntent::Dangerous,
        ),
    ]
}

fn render_db(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    let visible_height = table_visible_height(chunks[0]);
    let offset = scroll_offset(app.selected_database, visible_height);
    let rows = database_rows(app)
        .into_iter()
        .enumerate()
        .skip(offset)
        .take(visible_height)
        .map(|(index, row)| {
            let style = if index == app.selected_database {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Row::new(row).style(style)
        })
        .collect::<Vec<_>>();
    let rows = if rows.is_empty() {
        vec![Row::new([
            String::from("-"),
            String::from("-"),
            String::from("No database configured"),
        ])]
    } else {
        rows
    };
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(14),
                Constraint::Length(22),
                Constraint::Min(28),
            ],
        )
        .header(
            Row::new(["Scope", "Database", "Operation"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title("Configured databases  Enter/s explore  d describe  e query")
                .borders(Borders::ALL),
        ),
        chunks[0],
    );
    render_actions(frame, chunks[1], app);
}

fn render_detail_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(detail) = &app.detail else {
        return;
    };
    let popup = centered_rect(82, 72, area);
    frame.render_widget(Clear, popup);
    let lines = detail_panel_lines(&detail.content);
    let title = detail.title();
    let block = Block::default().title(title.as_str()).borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .scroll((detail.scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(
            "close [Esc]    close [Enter]    scroll down [j]    scroll up [k]    top [Home]    bottom [End]",
        )),
        chunks[1],
    );
}

fn detail_panel_lines(content: &DetailPanelContent) -> Vec<String> {
    match content {
        DetailPanelContent::Guide => guide_detail_lines(),
        DetailPanelContent::ConfigShow(report) => config_show_detail_lines(report),
        DetailPanelContent::ConfigDoctor(report) => config_doctor_detail_lines(report),
        DetailPanelContent::AgentDoctor(report) => agent_doctor_detail_lines(report),
        DetailPanelContent::ActionResult { events, result, .. } => {
            let theme = dw_ui::TerminalTheme::plain();
            let mut lines = events
                .iter()
                .map(dw_ui::action_event_line)
                .collect::<Vec<_>>();
            let result_lines = dw_tui_adapter::render::action_result_lines(result, &theme);
            if !lines.is_empty() && !result_lines.is_empty() {
                lines.push(String::new());
            }
            lines.extend(result_lines);
            if lines.is_empty() {
                lines.push("No detail returned.".into());
            }
            lines
        }
    }
}

fn config_show_detail_lines(report: &dw_config::ConfigShow) -> Vec<String> {
    vec![
        format!("Root      : {}", report.root),
        format!("Color     : {}", report.color),
        format!("Settings  : {}", report.settings_path),
        String::new(),
        "Files".into(),
        config_file_detail_line("projects", &report.projects_path, report.projects_exists),
        config_file_detail_line("workflow", &report.workflow_path, report.workflow_exists),
        config_file_detail_line("databases", &report.databases_path, report.databases_exists),
    ]
}

fn config_doctor_detail_lines(report: &dw_config::ConfigDoctorReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Status    : {}",
            if report.passed {
                "valid"
            } else {
                "needs fixes"
            }
        ),
        format!("Root      : {}", report.root),
        String::new(),
        "Checks".into(),
    ];
    for check in &report.checks {
        lines.push(config_check_detail_line(check));
        if let Some(message) = check.message.as_deref() {
            lines.push(format!("  Detail  : {message}"));
        }
    }
    lines.push(String::new());
    lines.push(if report.passed {
        "Result    : Configuration is valid.".into()
    } else {
        "Result    : Configuration is incomplete. Fix reported points, then run doctor again."
            .into()
    });
    lines
}

fn agent_doctor_detail_lines(report: &dw_agent::command::AgentDoctorReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Status    : {}",
            if report.passed() {
                "agents available"
            } else {
                "needs fixes"
            }
        ),
        format!(
            "Available : {}/{}",
            report.available_count(),
            report.total_count()
        ),
        String::new(),
        "Agents".into(),
    ];
    for check in &report.checks {
        lines.push(format!(
            "{} {:10} via {}",
            if check.available { "OK" } else { "KO" },
            check.agent,
            check.command
        ));
        if !check.available {
            lines.push(format!(
                "  Action  : install `{}` or check PATH",
                check.command
            ));
        }
    }
    lines
}

fn config_file_detail_line(label: &str, path: &str, exists: bool) -> String {
    format!("{} {:9}: {}", if exists { "OK" } else { "KO" }, label, path)
}

fn config_check_detail_line(check: &dw_config::ConfigDoctorCheck) -> String {
    format!("{} {}", if check.passed { "OK" } else { "KO" }, check.path)
}

fn render_ado(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(6),
        ])
        .split(area);

    render_ado_project_tabs(frame, chunks[0], app);
    render_ado_items(frame, chunks[1], app);
    render_ado_actions(frame, chunks[2], app);
}

fn render_ado_project_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if !app.snapshot.assigned_loaded {
        if app.assigned_loading() {
            render_loading_panel(
                frame,
                area,
                "ADO projects",
                "Loading assigned work items",
                app.loading_elapsed_label(BackgroundKind::Assigned),
            );
        } else {
            render_empty_state(
                frame,
                area,
                "ADO projects",
                "Work items are waiting for the background preload.",
            );
        }
        return;
    }

    let spans = app
        .snapshot
        .assigned
        .iter()
        .enumerate()
        .map(|(index, project)| {
            let style = if index == app.selected_ado_project {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if project.error.is_some() {
                Style::default().fg(Color::LightRed)
            } else {
                Style::default().fg(Color::Gray)
            };
            Span::styled(
                format!(" {} ({}) ", project.key, project.items.len()),
                style,
            )
        })
        .collect::<Vec<_>>();
    let line = if spans.is_empty() {
        Line::from("No configured project.")
    } else {
        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(line).block(Block::default().title("ADO projects").borders(Borders::ALL)),
        area,
    );
}

fn render_ado_items(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if !app.snapshot.assigned_loaded {
        if app.assigned_loading() {
            render_loading_panel(
                frame,
                area,
                "Assigned",
                "Loading work item cards",
                app.loading_elapsed_label(BackgroundKind::Assigned),
            );
        } else {
            render_empty_state(
                frame,
                area,
                "Assigned",
                "Work item data is not available yet.",
            );
        }
        return;
    }

    let Some(project) = app.snapshot.assigned.get(app.selected_ado_project) else {
        frame.render_widget(
            Paragraph::new("Configure Azure DevOps projects to populate this table.")
                .block(Block::default().title("Assigned").borders(Borders::ALL))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    };

    if let Some(error) = &project.error {
        frame.render_widget(
            Paragraph::new(format!("{}\n\n{}", project.label, error))
                .block(Block::default().title("Assigned").borders(Borders::ALL))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    if project.items.is_empty() {
        frame.render_widget(
            Paragraph::new("No assigned work item outside final states.")
                .block(
                    Block::default()
                        .title(project.label.as_str())
                        .borders(Borders::ALL),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let visible_height = table_visible_height(area);
    let offset = scroll_offset(app.selected_ado_item, visible_height);
    let rows = project
        .items
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible_height)
        .map(|(index, item)| {
            let style = if index == app.selected_ado_item {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Row::new([
                format!("#{}", item.id),
                item.kind.clone(),
                item.state.to_string(),
                if app
                    .snapshot
                    .workspace_for_work_item(&project.key, &item.id)
                    .is_some()
                {
                    "yes".into()
                } else {
                    "no".into()
                },
                item.title.clone(),
            ])
            .style(style)
        });
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(16),
                Constraint::Length(16),
                Constraint::Length(9),
                Constraint::Min(30),
            ],
        )
        .header(
            Row::new(["ID", "Type", "State", "Workspace", "Title"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title(project.label.as_str())
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_ado_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .title("ADO selection")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(selected_ado_target_lines(app)), inner);
}

fn selected_ado_target_lines(app: &App) -> Vec<Line<'static>> {
    let Some((project, item)) = app
        .snapshot
        .assigned
        .get(app.selected_ado_project)
        .and_then(|project| {
            project
                .items
                .get(app.selected_ado_item)
                .map(|item| (project, item))
        })
    else {
        return vec![target_line("Target", "No work item selected")];
    };
    vec![
        target_line(
            "Target",
            format!("{} #{} · {}", project.key, item.id, item.kind),
        ),
        target_line(
            "Title",
            if item.title.is_empty() {
                "-".into()
            } else {
                item.title.clone()
            },
        ),
    ]
}

fn ado_action_buttons(app: &App) -> Vec<ActionButton> {
    let mut buttons = vec![
        action_if_enabled(
            app.snapshot.assigned.len() > 1,
            "Project prev",
            "K",
            ActionIntent::Review,
        ),
        action_if_enabled(
            app.snapshot.assigned.len() > 1,
            "Project next",
            "J",
            ActionIntent::Review,
        ),
        ActionButton::new("Item up", "k", ActionIntent::Review),
        ActionButton::new("Item down", "j", ActionIntent::Review),
    ];

    if app
        .snapshot
        .assigned
        .get(app.selected_ado_project)
        .and_then(|project| project.items.get(app.selected_ado_item))
        .is_none()
    {
        buttons.extend([
            ActionButton::disabled("Prepare", "n"),
            ActionButton::disabled("Create", "x"),
            ActionButton::disabled("Move state", "e"),
            ActionButton::disabled("Context", "c"),
        ]);
        return buttons;
    }

    buttons.extend([
        action_if_available(
            selected_ado_action_preview(app, AdoItemAction::StartPreview),
            "Prepare",
            "n",
            ActionIntent::Review,
        ),
        action_if_available(
            selected_ado_action_preview(app, AdoItemAction::StartExecute),
            "Create",
            "x",
            ActionIntent::Primary,
        ),
        action_if_available(
            selected_ado_action_preview(app, AdoItemAction::OpenAgent),
            "Open agent",
            "o",
            ActionIntent::External,
        ),
        action_if_available(
            app.selected_ado_set_state_action_preview(),
            "Move state",
            "e",
            ActionIntent::Dangerous,
        ),
        ActionButton::new("State form", "E", ActionIntent::Review),
        action_if_available(
            selected_ado_action_preview(app, AdoItemAction::Context),
            "Context",
            "c",
            ActionIntent::Review,
        ),
        action_if_available(
            selected_ado_action_preview(app, AdoItemAction::WorkItem),
            "Card",
            "w",
            ActionIntent::Review,
        ),
        ActionButton::new("Open ADO", "u", ActionIntent::External),
    ]);
    buttons
}

fn selected_ado_action_preview(app: &App, action: AdoItemAction) -> Option<String> {
    crate::actions::selected_ado_action(
        &app.snapshot,
        app.selected_ado_project,
        app.selected_ado_item,
        action,
    )
    .map(|action| action.display_label())
}

fn render_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);
    let filter = if app.filter_active {
        format!("Search: {}_", app.filter)
    } else if app.filter.is_empty() {
        "Search: /".into()
    } else {
        format!("Search: {}", app.filter)
    };
    frame.render_widget(
        Paragraph::new(filter).block(Block::default().borders(Borders::ALL)),
        chunks[0],
    );

    let actions = app.visible_actions();
    let action_visible_height = list_visible_height(chunks[1]);
    let action_offset = scroll_offset(app.selected_action, action_visible_height);
    let items = actions
        .iter()
        .enumerate()
        .skip(action_offset)
        .take(action_visible_height)
        .map(|(visible_index, (_, action))| {
            let style = if visible_index == app.selected_action {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(kind_color(action.kind))
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<18}", action.display_label()),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(&action.description, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect::<Vec<_>>();
    let items = if items.is_empty() {
        vec![ListItem::new("No action available for this filter.")]
    } else {
        items
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Available operations")
                .borders(Borders::ALL),
        ),
        chunks[1],
    );
}

fn render_help_modal(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(72, 58, area);
    frame.render_widget(Clear, popup);
    let block = Block::default().title("Help").borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);
    let help = help_lines().join("\n");
    frame.render_widget(Paragraph::new(help).wrap(Wrap { trim: true }), chunks[0]);
    frame.render_widget(
        Paragraph::new(styled_shortcut_line("close [Esc]    close [?]    menu [m]")),
        chunks[1],
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let actions = footer_buttons(app);
    frame.render_widget(
        Paragraph::new(action_bar_lines(&actions, area.width))
            .style(Style::default().bg(Color::Black))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn footer_height(app: &App, width: u16) -> u16 {
    action_bar_lines(&footer_buttons(app), width)
        .len()
        .clamp(1, MAX_ACTION_BAR_LINES) as u16
}

fn footer_buttons(app: &App) -> Vec<ActionButton> {
    if app.snapshot.needs_init {
        return vec![
            ActionButton::new("Init root", "Enter", ActionIntent::Primary),
            ActionButton::new("Init root", "i", ActionIntent::Primary),
            ActionButton::new("Quit", "q", ActionIntent::Review),
        ];
    }
    let mut actions = match app.view {
        View::Dashboard => vec![
            ActionButton::new("Select", "j/k", ActionIntent::Review),
            ActionButton::new("Decide", "Enter", ActionIntent::Primary),
            ActionButton::new("Reload", "r", ActionIntent::Review),
        ],
        View::Workspaces => workspace_action_buttons(app),
        View::Ado if app.assigned_loading() => {
            vec![ActionButton::new("Reload", "r", ActionIntent::Review)]
        }
        View::Ado => ado_action_buttons(app),
        View::PullRequests if app.pull_requests_loading() => {
            vec![ActionButton::new("Reload", "r", ActionIntent::Review)]
        }
        View::PullRequests => pull_request_action_buttons(app),
        View::Db => vec![
            ActionButton::new("Explore schema", "Enter", ActionIntent::Primary),
            ActionButton::new("Explore schema", "s", ActionIntent::Review),
            ActionButton::new("Describe", "d", ActionIntent::Review),
            ActionButton::new("Query", "e", ActionIntent::Review),
        ],
        View::Composer => vec![
            ActionButton::new("Run", "Enter", ActionIntent::Primary),
            ActionButton::new("Select", "j/k", ActionIntent::Review),
            ActionButton::new("Suggest", "Ctrl+Space", ActionIntent::Review),
            ActionButton::new("Flows", "Esc", ActionIntent::Review),
        ],
    };
    actions.extend([
        ActionButton::new("Menu", "m", ActionIntent::Review),
        ActionButton::new("Help", "?", ActionIntent::Review),
        ActionButton::new("Quit", "q", ActionIntent::Review),
    ]);
    actions
}

fn render_init_required(frame: &mut Frame<'_>, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new("").style(
            Style::default()
                .fg(Color::DarkGray)
                .bg(Color::Black)
                .add_modifier(Modifier::DIM),
        ),
        area,
    );
    let popup = centered_rect(62, 34, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title("Initialize DevWorkflow")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "This root is not initialized.",
                Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            target_line("Root", app.snapshot.root.clone()),
            Line::from(""),
            Line::from("The TUI is locked until the root config, schemas, cache and project directories exist."),
            Line::from("The init action uses the same core init path as the CLI."),
        ])
        .wrap(Wrap { trim: true }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(action_bar_line(&[
            ActionButton::new("Initialize", "Enter", ActionIntent::Primary),
            ActionButton::new("Initialize", "i", ActionIntent::Primary),
            ActionButton::new("Quit", "q", ActionIntent::Review),
        ])),
        chunks[1],
    );
}

fn styled_shortcut_line(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let mut first = true;
    for segment in shortcut_segments(text) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if !first {
            spans.push(Span::raw(" "));
        }
        first = false;
        if let Some((label, shortcut)) = shortcut_label_key(segment) {
            spans.extend(shortcut_chip(shortcut, label));
        } else {
            spans.push(Span::styled(
                format!(" {segment} "),
                Style::default().fg(Color::Gray).bg(Color::Black),
            ));
        }
    }
    Line::from(spans)
}

fn action_bar_line(actions: &[ActionButton]) -> Line<'static> {
    action_bar_lines(actions, u16::MAX)
        .into_iter()
        .next()
        .unwrap_or_else(|| Line::from(""))
}

fn action_bar_lines(actions: &[ActionButton], width: u16) -> Vec<Line<'static>> {
    let max_width = width as usize;
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut line_width = 0;
    for (index, action) in actions.iter().enumerate() {
        let action_width = action_button_width(action);
        let separator_width = usize::from(!spans.is_empty());
        if max_width > 0
            && line_width > 0
            && line_width + separator_width + action_width > max_width
        {
            lines.push(Line::from(spans));
            spans = Vec::new();
            line_width = 0;
        }
        if index > 0 && !spans.is_empty() {
            spans.push(Span::raw(" "));
            line_width += 1;
        }
        spans.extend(action_button_spans(action));
        line_width += action_width;
    }
    if !spans.is_empty() || lines.is_empty() {
        lines.push(Line::from(spans));
    }
    lines.truncate(MAX_ACTION_BAR_LINES);
    lines
}

fn action_button_spans(action: &ActionButton) -> Vec<Span<'static>> {
    let style = action_intent_style(action.intent, action.enabled);
    vec![Span::styled(
        format!(" {} [{}] ", action.label, action.key),
        style,
    )]
}

fn action_button_width(action: &ActionButton) -> usize {
    action.label.chars().count() + action.key.chars().count() + 5
}

fn action_intent_style(intent: ActionIntent, enabled: bool) -> Style {
    if !enabled {
        return Style::default().fg(Color::DarkGray).bg(Color::Black);
    }
    match intent {
        ActionIntent::Primary => Style::default()
            .fg(Color::Black)
            .bg(Color::LightGreen)
            .add_modifier(Modifier::BOLD),
        ActionIntent::Review => Style::default().fg(Color::Black).bg(Color::LightBlue),
        ActionIntent::External => Style::default().fg(Color::Black).bg(Color::Cyan),
        ActionIntent::Dangerous => Style::default()
            .fg(Color::White)
            .bg(Color::LightRed)
            .add_modifier(Modifier::BOLD),
        ActionIntent::Disabled => Style::default().fg(Color::DarkGray).bg(Color::Black),
    }
}

fn risk_badge_line(risk: ActionRisk) -> Line<'static> {
    let (label, style) = match risk {
        ActionRisk::Safe => (
            " safe ",
            Style::default().fg(Color::Black).bg(Color::LightGreen),
        ),
        ActionRisk::DryRun => (
            " preview ",
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        ActionRisk::OpensExternal => (
            " external ",
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        ActionRisk::Destructive => (
            " updates data ",
            Style::default()
                .fg(Color::White)
                .bg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        ),
    };
    Line::from(vec![Span::styled(label, style)])
}

fn confirmation_confirm_button(risk: ActionRisk) -> ActionButton {
    match risk {
        ActionRisk::Safe => ActionButton::new("Run", "Enter", ActionIntent::Primary),
        ActionRisk::DryRun => ActionButton::new("Run preview", "Enter", ActionIntent::Review),
        ActionRisk::OpensExternal => ActionButton::new("Open", "Enter", ActionIntent::External),
        ActionRisk::Destructive => ActionButton::new("Confirm", "Enter", ActionIntent::Dangerous),
    }
}

fn target_line(label: &'static str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), Style::default().fg(Color::DarkGray)),
        Span::styled(value.into(), Style::default().fg(Color::White)),
    ])
}

fn status_badge_line(badges: &[StatusBadge]) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, badge) in badges.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!(" {}: {} ", badge.label, badge.status),
            Style::default().fg(Color::Black).bg(badge.color),
        ));
    }
    Line::from(spans)
}

fn shortcut_label_key(segment: &str) -> Option<(&str, &str)> {
    let (label, shortcut) = segment.rsplit_once('[')?;
    let shortcut = shortcut.strip_suffix(']')?.trim();
    let label = label.trim();
    if label.is_empty() || shortcut.is_empty() {
        return None;
    }
    Some((label, shortcut))
}

fn shortcut_segments(text: &str) -> Vec<&str> {
    text.split(" | ")
        .flat_map(|segment| segment.split("    "))
        .collect()
}

fn shortcut_chip(shortcut: &str, label: &str) -> Vec<Span<'static>> {
    let chip = Style::default().fg(Color::White).bg(Color::Blue);
    vec![Span::styled(format!(" {} [{shortcut}] ", label), chip)]
}

#[cfg(test)]
fn shortcut_chip_style() -> Style {
    Style::default().fg(Color::White).bg(Color::Blue)
}

fn database_rows(app: &App) -> Vec<[String; 3]> {
    app.snapshot
        .database_entries
        .iter()
        .map(|database| {
            let scope = database
                .project
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "global".into());
            let action = if let Some(project) = database.project.as_ref() {
                format!("Schema ({project}/{})", database.key)
            } else {
                format!("Schema ({})", database.key)
            };
            [scope, database.key.to_string(), action]
        })
        .collect()
}

fn render_confirmation(frame: &mut Frame<'_>, area: Rect, action: &TuiAction) {
    let popup = centered_rect(70, 28, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title(action.kind.confirmation_title())
        .borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(Paragraph::new(risk_badge_line(action.kind)), chunks[0]);
    frame.render_widget(
        Paragraph::new(vec![
            target_line("Operation", action.display_label()),
            target_line("Effect", action.description.clone()),
        ])
        .wrap(Wrap { trim: true }),
        chunks[1],
    );
    let mut lines = confirmation_lines(action);
    lines.retain(|line| !line.trim().is_empty() && line != &action.display_label());
    frame.render_widget(
        Paragraph::new(lines.join("\n")).wrap(Wrap { trim: true }),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(action_bar_line(&[
            confirmation_confirm_button(action.kind),
            ActionButton::new("Cancel", "Esc", ActionIntent::Review),
        ])),
        chunks[3],
    );
}

fn render_input_prompt(frame: &mut Frame<'_>, area: Rect, prompt: &TuiInputPrompt) {
    let popup = centered_rect(72, 36, area);
    frame.render_widget(Clear, popup);
    let block = Block::default().title("Action input").borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(input_prompt_header_lines(prompt)).wrap(Wrap { trim: true }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(input_prompt_body_lines(prompt).join("\n")).wrap(Wrap { trim: false }),
        chunks[1],
    );
    frame.render_widget(Paragraph::new(input_prompt_footer(prompt)), chunks[2]);
}

fn input_prompt_header_lines(prompt: &TuiInputPrompt) -> Vec<Line<'static>> {
    let (label, help) = match &prompt.request {
        dw_core::InputRequest::Confirm { label, help, .. }
        | dw_core::InputRequest::SelectOne { label, help, .. }
        | dw_core::InputRequest::SelectMany { label, help, .. }
        | dw_core::InputRequest::Text { label, help, .. }
        | dw_core::InputRequest::Secret { label, help, .. } => (label, help),
    };
    vec![
        target_line("Run", format!("{:?}", prompt.run_id)),
        target_line("Prompt", label.clone()),
        target_line(
            "Help",
            help.clone()
                .unwrap_or_else(|| "Respond to continue the action.".into()),
        ),
    ]
}

fn input_prompt_body_lines(prompt: &TuiInputPrompt) -> Vec<String> {
    match &prompt.request {
        dw_core::InputRequest::Confirm { default, .. } => {
            vec![format!("Default: {}", if *default { "yes" } else { "no" })]
        }
        dw_core::InputRequest::SelectOne { choices, .. } => choices
            .iter()
            .enumerate()
            .map(|(index, choice)| {
                format!(
                    "{} {}",
                    if index == prompt.selected { ">" } else { " " },
                    choice.label
                )
            })
            .collect(),
        dw_core::InputRequest::SelectMany { choices, .. } => choices
            .iter()
            .enumerate()
            .map(|(index, choice)| {
                format!(
                    "{} [{}] {}",
                    if index == prompt.selected { ">" } else { " " },
                    if prompt.selected_many.contains(&index) {
                        "x"
                    } else {
                        " "
                    },
                    choice.label
                )
            })
            .collect(),
        dw_core::InputRequest::Text { .. } => vec![format!("Value: {}", prompt.value)],
        dw_core::InputRequest::Secret { .. } => vec![format!(
            "Value: {}",
            "*".repeat(prompt.value.chars().count())
        )],
    }
}

fn input_prompt_footer(prompt: &TuiInputPrompt) -> Line<'static> {
    match &prompt.request {
        dw_core::InputRequest::Confirm { .. } => action_bar_line(&[
            ActionButton::new("Yes", "y/Enter", ActionIntent::Primary),
            ActionButton::new("No", "n/Esc", ActionIntent::Review),
        ]),
        dw_core::InputRequest::SelectOne { .. } => {
            Line::from("select [j/k]    submit [Enter]    cancel [Esc]")
        }
        dw_core::InputRequest::SelectMany { .. } => {
            Line::from("select [j/k]    toggle [Space]    submit [Enter]    cancel [Esc]")
        }
        dw_core::InputRequest::Text { .. } | dw_core::InputRequest::Secret { .. } => {
            Line::from("type value    backspace    submit [Enter]    cancel [Esc]")
        }
    }
}

fn render_options(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(58, 46, area);
    frame.render_widget(Clear, popup);
    let block = Block::default().title("Menu").borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(options_summary_lines(app).join("\n")).wrap(Wrap { trim: true }),
        chunks[0],
    );

    let items = MENU_SECTIONS
        .iter()
        .enumerate()
        .map(|section| {
            let (index, section) = section;
            let selected = index == app.selected_menu_section;
            let style = if selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<16}", section.label()),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(section.description(), Style::default().fg(Color::Gray)),
            ]))
            .style(style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        List::new(items).block(Block::default().title("Sections").borders(Borders::ALL)),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(
            "open [Enter]    select down [j]    select up [k]    close [Esc]    close [m]",
        )),
        chunks[2],
    );
}

fn render_menu_section(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(72, 58, area);
    frame.render_widget(Clear, popup);
    let section = app.selected_menu_section();
    let block = Block::default()
        .title(format!("Menu / {}", section.label()))
        .borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(inner);
    let current_agent = app.snapshot.default_agent();
    let current_color = app.snapshot.color_mode;
    let rows = match section {
        MenuSection::Information => vec![
            menu_row(
                app.selected_option == 0,
                false,
                "h",
                "Journal",
                "",
                "View operations and logs",
            ),
            menu_row(
                app.selected_option == 1,
                false,
                "i",
                "State/debug",
                "",
                "View loads, messages and queue",
            ),
            menu_row(
                app.selected_option == 2,
                false,
                "?",
                "Help",
                "",
                "Open help",
            ),
        ],
        _ => app
            .quick_options_for_menu_section(section)
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let active = option_active(item.state, current_agent, current_color);
                menu_row(
                    index == app.selected_option,
                    active,
                    item.key.to_string(),
                    item.label,
                    if active { "active" } else { "" },
                    item.hint,
                )
            })
            .collect(),
    };
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(5),
                Constraint::Length(18),
                Constraint::Length(8),
                Constraint::Min(26),
            ],
        )
        .header(
            Row::new(["Key", "Option", "State", "Operation"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title("Options").borders(Borders::ALL)),
        chunks[0],
    );
    let preview = match section {
        MenuSection::Information => match app.selected_option {
            0 => "Open operation journal and logs.".into(),
            1 => "Open current state, loads and messages.".into(),
            2 => "Open help.".into(),
            _ => "No selected option.".into(),
        },
        _ => app
            .quick_options_for_menu_section(section)
            .get(app.selected_option)
            .map(|option| {
                crate::actions::option_action(&app.snapshot.root, option.action).display_label()
            })
            .unwrap_or_else(|| "No selected option.".into()),
    };
    frame.render_widget(
        Paragraph::new(preview)
            .block(Block::default().title("Preview").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(
            "run or open [Enter]    select down [j]    select up [k]    back [Esc]    close [m]",
        )),
        chunks[2],
    );
}

fn menu_row(
    selected: bool,
    active: bool,
    key: impl Into<String>,
    option: impl Into<String>,
    state: impl Into<String>,
    operation: impl Into<String>,
) -> Row<'static> {
    let style = if selected {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else if active {
        Style::default().fg(Color::LightGreen)
    } else {
        Style::default()
    };
    Row::new([key.into(), option.into(), state.into(), operation.into()]).style(style)
}

fn render_history_output(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = if app.history.output_fullscreen {
        area
    } else {
        centered_rect(82, 72, area)
    };
    frame.render_widget(Clear, popup);
    let lines = history_journal_line_items(app);
    let title = if app.history.output_fullscreen {
        "Journal fullscreen"
    } else {
        "Journal"
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(styled_journal_lines(lines))
            .scroll((app.history.output_scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(
            "close [Esc/h]    fullscreen [f]    levels [e/w/i/d/o]    all [a]    scroll [j/k]    run [←/→]    top/bottom [Home/End]",
        )),
        chunks[1],
    );
}

fn styled_journal_lines(lines: Vec<JournalLine>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let style = match line.level() {
                JournalLogLevel::Error => Style::default().fg(Color::LightRed),
                JournalLogLevel::Warn => Style::default().fg(Color::Yellow),
                JournalLogLevel::Info => Style::default().fg(Color::Cyan),
                JournalLogLevel::Debug => Style::default().fg(Color::Magenta),
                JournalLogLevel::Other => Style::default().fg(Color::Gray),
            };
            let text = line.render_text();
            if text.starts_with("Run") || text.starts_with("Levels") || text.starts_with("View") {
                Line::from(Span::styled(text, Style::default().fg(Color::White)))
            } else {
                Line::from(Span::styled(text, style))
            }
        })
        .collect()
}

fn render_action_progress_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(78, 48, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title("Creating workspace")
        .borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(action_progress_lines(app).join("\n")).wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(
            "please wait until the action finishes    force quit [Ctrl+C]",
        )),
        chunks[1],
    );
}

fn action_progress_lines(app: &App) -> Vec<String> {
    let Some(run_id) = app.action_progress else {
        return vec!["Waiting for action...".into()];
    };
    let Some(entry) = app
        .history
        .entries
        .iter()
        .rev()
        .find(|entry| entry.id == run_id)
    else {
        return vec!["Waiting for action...".into()];
    };
    let mut lines = vec![
        format!("Operation : {}", entry.request_label),
        format!("Status    : {}", entry.status),
        String::new(),
        "Live progress".into(),
    ];
    let events: &[crate::history::RecordedActionEvent] = match &entry.record {
        ActionRunRecord::Running { events }
        | ActionRunRecord::Completed { events, .. }
        | ActionRunRecord::Failed { events, .. } => events.as_slice(),
        ActionRunRecord::ExternalLaunch { .. } => &[],
    };
    if events.is_empty() {
        lines.push("- waiting for first event".into());
    } else {
        lines.extend(
            events
                .iter()
                .rev()
                .take(12)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|event| format!("- {}", dw_ui::action_event_line(&event.event))),
        );
    }
    lines
}

fn render_state_modal(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(82, 64, area);
    frame.render_widget(Clear, popup);
    let lines = state_modal_lines(app);
    let scroll = if app.state_scroll == usize::MAX {
        lines.len().saturating_sub(1)
    } else {
        app.state_scroll
    };
    let block = Block::default()
        .title("State and messages")
        .borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(
            "close [Esc]    close [i]    scroll down [j]    scroll up [k]    top [Home]    bottom [End]",
        )),
        chunks[1],
    );
}

fn render_form(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(form) = &app.form else {
        return;
    };
    let popup = centered_rect(78, 72, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title("Action composer")
        .borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    render_form_state(frame, inner, form, app, true);
}

fn render_action_builder_view(frame: &mut Frame<'_>, area: Rect, app: &App) {
    render_form_state(frame, area, &app.action_form, app, false);
}

fn render_form_state(frame: &mut Frame<'_>, area: Rect, form: &FormState, app: &App, modal: bool) {
    match form.mode {
        FormMode::Selecting => {
            let items = FormTemplate::ALL
                .iter()
                .enumerate()
                .map(|(index, template)| {
                    let style = if index == form.template_index {
                        Style::default().fg(Color::Black).bg(Color::Cyan)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:<18}", template.label()),
                            style.add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" "),
                        Span::styled(template.description(), Style::default().fg(Color::Gray)),
                    ]))
                })
                .collect::<Vec<_>>();
            let title = if modal {
                "Choose a template (Enter)"
            } else {
                "Advanced composer · choose a template"
            };
            frame.render_widget(
                List::new(items).block(Block::default().title(title).borders(Borders::ALL)),
                area,
            );
        }
        FormMode::Editing => render_form_fields(frame, area, form, app, modal),
    }
}

fn render_form_fields(frame: &mut Frame<'_>, area: Rect, form: &FormState, app: &App, modal: bool) {
    if form.template == FormTemplate::AdoSetState {
        render_ado_state_form_fields(frame, area, form, app, modal);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(5)])
        .split(area);

    let rows = form.fields.iter().enumerate().map(|(index, field)| {
        let selected = index == form.selected_field;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default()
        };
        let value = match field.kind {
            FieldKind::Text => field.value.clone(),
            FieldKind::Toggle => {
                if field.enabled() {
                    "yes".into()
                } else {
                    "no".into()
                }
            }
        };
        Row::new([field.label.clone(), value, field.help.clone()]).style(style)
    });
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(18),
                Constraint::Length(34),
                Constraint::Min(24),
            ],
        )
        .header(
            Row::new(["Field", "Value", "Help"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title(form.template.label())
                .borders(Borders::ALL),
        ),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(if modal {
            form_preview_lines(app).join("\n")
        } else {
            action_builder_preview_lines(app).join("\n")
        })
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .wrap(Wrap { trim: true }),
        chunks[1],
    );
}

fn render_ado_state_form_fields(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &FormState,
    app: &App,
    _modal: bool,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    let work_items = form_field_value(form, "Work item IDs");
    let project = form_field_value(form, "Project");
    let destination = form_field_value(form, "Destination state");
    let note = form_field_value(form, "ADO note");
    let current_state = app
        .snapshot
        .assigned
        .get(app.selected_ado_project)
        .and_then(|project| project.items.get(app.selected_ado_item))
        .map(|item| item.state.clone())
        .unwrap_or_else(|| "current".into());
    let title = app
        .snapshot
        .assigned
        .get(app.selected_ado_project)
        .and_then(|project| project.items.get(app.selected_ado_item))
        .map(|item| item.title.clone())
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "Selected Azure DevOps work item".into());
    let summary = vec![
        risk_badge_line(ActionRisk::Destructive),
        target_line("Work item", format!("{project} #{work_items}")),
        target_line("Title", title),
        Line::from(vec![
            Span::styled(current_state.to_string(), Style::default().fg(Color::Gray)),
            Span::styled("  ->  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                destination.clone(),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        target_line("ADO note", note.clone()),
        Line::from(""),
        Line::from(Span::styled(
            "This writes the state transition to Azure DevOps.",
            Style::default().fg(Color::LightRed),
        )),
    ];
    frame.render_widget(
        Paragraph::new(summary)
            .block(
                Block::default()
                    .title("Move ADO work item state")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let rows = form.fields.iter().enumerate().map(|(index, field)| {
        let selected = index == form.selected_field;
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default()
        };
        Row::new([
            field.label.clone(),
            field.value.clone(),
            match field.label.as_str() {
                "Work item IDs" => "Selected work item(s)".into(),
                "Project" => "ADO project".into(),
                "Destination state" => "Next state to apply".into(),
                "ADO note" => "History entry written to ADO".into(),
                _ => field.help.clone(),
            },
        ])
        .style(style)
    });
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(20),
                Constraint::Length(34),
                Constraint::Min(24),
            ],
        )
        .header(
            Row::new(["Step", "Value", "Meaning"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title("Transition").borders(Borders::ALL)),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(action_bar_line(&[
            ActionButton::new("Apply", "Enter", ActionIntent::Dangerous),
            ActionButton::new("Cancel", "Esc", ActionIntent::Review),
            ActionButton::new("Next field", "Tab", ActionIntent::Review),
            ActionButton::new("Suggestion", "Ctrl+Space", ActionIntent::Review),
        ])),
        chunks[2],
    );
}

fn form_field_value(form: &FormState, label: &str) -> String {
    form.fields
        .iter()
        .find(|field| field.label == label)
        .map(|field| field.value.clone())
        .unwrap_or_default()
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn list_visible_height(area: Rect) -> usize {
    area.height.saturating_sub(2).max(1) as usize
}

fn table_visible_height(area: Rect) -> usize {
    area.height.saturating_sub(3).max(1) as usize
}

fn scroll_offset(selected: usize, visible_height: usize) -> usize {
    if visible_height == 0 {
        return selected;
    }
    selected.saturating_add(1).saturating_sub(visible_height)
}

fn kind_color(kind: ActionRisk) -> Color {
    match kind {
        ActionRisk::Safe => Color::White,
        ActionRisk::OpensExternal => Color::LightBlue,
        ActionRisk::DryRun => Color::LightYellow,
        ActionRisk::Destructive => Color::LightRed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_bar_tabs_match_gitui_label_key_shape() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Workspaces;

        let spans = tab_spans(&app);
        let text = spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let selected = spans
            .iter()
            .find(|span| span.content.contains("Workspaces [2]"))
            .expect("selected tab");
        let inactive = spans
            .iter()
            .find(|span| span.content.contains("Dashboard [1]"))
            .expect("inactive tab");

        assert!(text.contains("Dashboard [1]"));
        assert!(text.contains("Workspaces [2]"));
        assert!(text.contains(" | "));
        assert_eq!(selected.style.fg, Some(Color::White));
        assert!(!selected.style.add_modifier.contains(Modifier::DIM));
        assert!(inactive.style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn action_loading_label_names_latest_operation() {
        let event = dw_core::DwActionEvent::Started {
            action_id: "task.finish".into(),
        };
        let rendered = dw_ui::action_event_line(&event);

        let label = action_loading_label(Some(rendered.clone()), Some("4s".into()));

        assert!(label.contains("Operation 4s"));
        assert!(label.contains(&rendered));
    }

    #[test]
    fn title_like_table_columns_take_remaining_width() {
        assert_eq!(cockpit_table_constraints()[1], Constraint::Min(28));
        assert_eq!(pull_request_table_constraints()[7], Constraint::Min(30));
        assert_eq!(workspace_table_constraints()[1], Constraint::Min(32));
    }

    #[test]
    fn shortcut_line_renders_gitui_like_action_key_chips() {
        let line = styled_shortcut_line("cockpit down [j]    decision [Enter] | quit [q]");
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(text.contains(" cockpit down [j]"));
        assert!(text.contains(" decision [Enter]"));
        assert!(text.contains(" quit [q]"));
        assert_eq!(line.spans[0].style, shortcut_chip_style());
        assert!(!line.spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(
            !line.spans[0]
                .style
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn action_bar_wraps_shortcut_actions_by_chip() {
        let actions = [
            ActionButton::new("Explore schema", "Enter", ActionIntent::Primary),
            ActionButton::new("Describe", "d", ActionIntent::Review),
            ActionButton::new("Query", "e", ActionIntent::Review),
        ];

        let lines = action_bar_lines(&actions, 20);

        assert_eq!(lines.len(), 3);
        let rendered = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert_eq!(rendered[0], " Explore schema [Enter] ");
        assert_eq!(rendered[1], " Describe [d] ");
        assert_eq!(rendered[2], " Query [e] ");
    }

    #[test]
    fn action_bar_caps_wrapped_shortcuts_to_three_lines() {
        let actions = [
            ActionButton::new("One", "1", ActionIntent::Review),
            ActionButton::new("Two", "2", ActionIntent::Review),
            ActionButton::new("Three", "3", ActionIntent::Review),
            ActionButton::new("Four", "4", ActionIntent::Review),
            ActionButton::new("Five", "5", ActionIntent::Review),
        ];

        let lines = action_bar_lines(&actions, 1);

        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn confirmation_button_matches_action_risk() {
        let preview = confirmation_confirm_button(ActionRisk::DryRun);
        let external = confirmation_confirm_button(ActionRisk::OpensExternal);
        let destructive = confirmation_confirm_button(ActionRisk::Destructive);

        assert_eq!(preview.label, "Run preview");
        assert_eq!(preview.intent, ActionIntent::Review);
        assert_eq!(external.label, "Open");
        assert_eq!(external.intent, ActionIntent::External);
        assert_eq!(destructive.label, "Confirm");
        assert_eq!(destructive.intent, ActionIntent::Dangerous);
    }

    #[test]
    fn ado_action_buttons_preview_workflow_state_action() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned_loaded = true;
        app.snapshot.assigned = vec![ado_project("ha", "User Story")];

        let target = selected_ado_target_lines(&app);
        let buttons = ado_action_buttons(&app);

        let target_text = target
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(target_text.contains("ha #42"));
        assert!(buttons.iter().any(|button| button.label == "Prepare"));
        assert!(
            buttons
                .iter()
                .any(|button| button.label == "Move state" && button.enabled)
        );
        assert!(
            !buttons
                .iter()
                .any(|button| button.label.contains("workflow state"))
        );
    }

    #[test]
    fn ado_action_buttons_disable_missing_workflow_state_mapping() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned_loaded = true;
        app.snapshot.assigned = vec![ado_project("ha", "Epic")];

        let buttons = ado_action_buttons(&app);
        let move_state = buttons
            .iter()
            .find(|button| button.label == "Move state")
            .expect("move state button");

        assert!(!move_state.enabled);
    }

    #[test]
    fn workspace_action_buttons_preview_primary_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![workspace("/tmp/ws-front", "demo")];

        let target = selected_workspace_target_lines(&app);
        let buttons = workspace_action_buttons(&app);
        let target_text = target
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(target_text.contains("ha · #42 Demo [Active] · demo"));
        assert!(buttons.iter().any(|button| button.label == "Check"));
        assert!(buttons.iter().any(|button| button.label == "Finish"));
        assert!(buttons.iter().any(|button| button.label == "Remove"));
    }

    #[test]
    fn pull_request_action_buttons_preview_local_finish_and_diff() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![pull_request(Some("/tmp/ws-front"))];

        let target = selected_pull_request_target_lines(&app);
        let buttons = pull_request_action_buttons(&app);
        let target_text = target
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(target_text.contains("ha / front · #42"));
        assert!(
            buttons
                .iter()
                .any(|button| button.label == "Finish" && button.enabled)
        );
        assert!(
            buttons
                .iter()
                .any(|button| button.label == "Diff" && button.enabled)
        );
    }

    #[test]
    fn pull_request_action_buttons_preview_remote_workspace_creation() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![pull_request(None)];

        let buttons = pull_request_action_buttons(&app);

        assert!(
            buttons
                .iter()
                .any(|button| button.label == "Create" && button.enabled)
        );
        assert!(
            buttons
                .iter()
                .any(|button| button.label == "Changes" && button.enabled)
        );
    }

    #[test]
    fn database_rows_include_global_and_project_databases() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.databases.globals.insert(
            "shared".into(),
            serde_json::json!({"provider": "sqlserver"}),
        );
        app.snapshot.databases.projects.insert(
            "ha".into(),
            serde_json::json!({"databases": {"ha-dev": {"provider": "sqlserver"}}}),
        );
        app.snapshot.database_entries =
            crate::model::database_entries_for_tui(&app.snapshot.databases);

        let rows = database_rows(&app);

        assert!(
            rows.iter()
                .any(|row| row[0] == "global" && row[1] == "shared")
        );
        assert!(rows.iter().any(|row| row[0] == "ha" && row[1] == "ha-dev"));
    }

    #[test]
    fn config_doctor_detail_lines_show_status_and_details() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.config_doctor = dw_config::ConfigDoctorReport {
            root: "/tmp/missing-dw-root".into(),
            passed: false,
            checks: vec![
                dw_config::ConfigDoctorCheck {
                    path: "/tmp/missing-dw-root/config/projects.jsonc".into(),
                    passed: true,
                    message: None,
                },
                dw_config::ConfigDoctorCheck {
                    path: "/tmp/missing-dw-root/config/workflow.jsonc".into(),
                    passed: false,
                    message: Some("File not found".into()),
                },
            ],
        };

        let lines = config_doctor_detail_lines(&app.snapshot.config_doctor);

        assert!(lines.iter().any(|line| line.contains("Status")));
        assert!(lines.iter().any(|line| line.contains("Checks")));
        assert!(lines.iter().any(|line| line.contains("File not found")));
    }

    fn ado_project(key: &str, kind: &str) -> crate::model::AdoAssignedProject {
        crate::model::AdoAssignedProject {
            key: key.into(),
            label: "Hommage Agence".into(),
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

    fn pull_request(workspace: Option<&str>) -> crate::model::TuiPullRequest {
        crate::model::TuiPullRequest {
            workspace: workspace.map(str::to_string),
            project: "ha".into(),
            repository: "front".into(),
            ado_repository: "HA Front".into(),
            branch: "feature/42-demo".into(),
            target_branch: "develop".into(),
            pull_request_id: Some(dw_core::PullRequestId::from("42")),
            title: Some("Demo".into()),
            is_draft: false,
            work_item_ids: vec!["42".into()],
            url: Some("https://example.invalid/pr/42".into()),
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
