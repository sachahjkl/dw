use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, Wrap},
};

use crate::actions::{PullRequestAction, QUICK_OPTIONS, quick_option_shortcut_hint};
use crate::app::App;
use crate::background::BackgroundKind;
use crate::form::{FieldKind, FormMode, FormState, FormTemplate};
use crate::model::{
    ActionRisk, CockpitSeverity, DetailPanelContent, TuiAction, View, WorkspaceAction,
};
use crate::ui_text::{
    action_builder_preview_lines, confirmation_lines, form_preview_lines, help_lines,
    history_output_lines, option_active, options_summary_lines, shortcut_bar_line,
    state_modal_lines,
};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, root[0], app);
    render_tabs(frame, root[1], app);
    render_body(frame, root[2], app);
    render_footer(frame, root[3], app);

    if let Some(action) = &app.confirmation {
        render_confirmation(frame, area, action);
    }

    if app.form.is_some() {
        render_form(frame, area, app);
    }

    if app.options_open {
        render_options(frame, area, app);
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
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut title = vec![Span::styled(
        &app.snapshot.root,
        Style::default().fg(Color::Gray),
    )];
    if app.assigned_loading() {
        title.push(Span::raw("  "));
        title.push(loading_span(
            "ADO",
            app.loading_elapsed_label(BackgroundKind::Assigned),
        ));
    }
    if app.snapshot_loading() {
        title.push(Span::raw("  "));
        title.push(loading_span(
            "Snapshot",
            app.loading_elapsed_label(BackgroundKind::Snapshot),
        ));
    }
    if app.pull_requests_loading() {
        title.push(Span::raw("  "));
        title.push(loading_span(
            "PRs",
            app.loading_elapsed_label(BackgroundKind::PullRequests),
        ));
    }
    if app.action_loading() {
        title.push(Span::raw("  "));
        title.push(loading_span(
            "Opération",
            app.loading_elapsed_label(BackgroundKind::Action),
        ));
    }
    let queued = app.pending_action_count();
    if queued > 0 {
        title.push(Span::raw("  "));
        title.push(Span::styled(
            format!("File {queued}"),
            Style::default().fg(Color::LightMagenta),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(title)).style(Style::default().bg(Color::Black)),
        area,
    );
}

fn loading_span(label: &'static str, elapsed: Option<String>) -> Span<'static> {
    Span::styled(
        format!("{label} {}...", elapsed.unwrap_or_else(|| "<1s".into())),
        Style::default().fg(Color::Yellow),
    )
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let labels = View::ALL
        .iter()
        .enumerate()
        .map(|(index, view)| {
            let style = if *view == app.view {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Span::styled(format!(" {} {} ", index + 1, view.label()), style)
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(Line::from(labels)), area);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) {
    match app.view {
        View::Dashboard => render_dashboard(frame, area, app),
        View::Workspaces => render_workspaces(frame, area, app),
        View::Ado => render_ado(frame, area, app),
        View::PullRequests => render_pull_requests(frame, area, app),
        View::Db => render_db(frame, area, app),
        View::Config => render_config(frame, area, app),
        View::Composer => render_action_builder_view(frame, area, app),
        View::Help => render_help(frame, area),
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
        Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Min(28),
                Constraint::Length(14),
                Constraint::Length(22),
                Constraint::Min(20),
            ],
        )
        .header(
            Row::new(["Section", "Sujet", "Statut", "Opération", "Contexte"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title("Cockpit · Entrée lance l'opération primaire")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn cockpit_color(severity: CockpitSeverity) -> Color {
    match severity {
        CockpitSeverity::Normal => Color::White,
        CockpitSeverity::Attention => Color::Yellow,
        CockpitSeverity::Blocked => Color::LightRed,
    }
}

fn render_metrics(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default().title("Synthèse").borders(Borders::ALL);
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
            Constraint::Min(0),
        ])
        .split(inner);

    let workspace_total = app.snapshot.workspaces.len().max(1) as u16;
    let prune_ratio = ((app.snapshot.prune_candidates as u16) * 100 / workspace_total).min(100);
    let lines = [
        format!("Projets       {}", app.snapshot.project_count()),
        format!("Workspaces    {}", app.snapshot.workspaces.len()),
        format!("Work items    {}", app.snapshot.assigned_count()),
        format!(
            "PR actives    {}",
            app.snapshot
                .pull_requests
                .iter()
                .filter(|item| item.pull_request_id.is_some())
                .count()
        ),
        format!("Nettoyage     {}", app.snapshot.prune_candidates),
        format!("DB            {}", app.snapshot.database_count()),
        format!("Agent         {}", app.snapshot.default_agent()),
    ];
    for (line, row) in lines.iter().zip(rows.iter()) {
        frame.render_widget(Paragraph::new(line.as_str()), *row);
    }
    frame.render_widget(
        Gauge::default()
            .label("workspaces prêts à nettoyer")
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(prune_ratio),
        rows[7],
    );
    for (line, row) in app
        .background_status_lines()
        .iter()
        .zip(rows.iter().skip(8))
    {
        frame.render_widget(
            Paragraph::new(line.as_str()).style(Style::default().fg(Color::Gray)),
            *row,
        );
    }
}

fn render_pull_requests(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(7)])
        .split(area);

    if !app.snapshot.pull_requests_loaded {
        let message = if app.pull_requests_loading() {
            app.loading_elapsed_label(BackgroundKind::PullRequests)
                .map(|elapsed| format!("Chargement des PRs en arrière-plan depuis {elapsed}.\nr: relancer le chargement    Tab: changer d'onglet"))
                .unwrap_or_else(|| {
                    "Chargement des PRs en arrière-plan.\nr: relancer le chargement    Tab: changer d'onglet".into()
                })
        } else {
            "Entrer dans l'onglet PRs charge les PR actives.\nr: recharger les données.".into()
        };
        frame.render_widget(
            Paragraph::new(message)
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

    if app.snapshot.pull_requests.is_empty() {
        frame.render_widget(
            Paragraph::new("Aucun workspace/repository local à relier à une PR.")
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
                item.project.clone(),
                item.repository.clone(),
                item.pull_request_id
                    .map(|id| format!("#{id}"))
                    .unwrap_or_else(|| "-".into()),
                if item.error.is_some() {
                    "erreur".into()
                } else if item.pull_request_id.is_some() {
                    if item.is_draft {
                        "draft".into()
                    } else {
                        "ouverte".into()
                    }
                } else {
                    "absente".into()
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
                    "local".into()
                } else {
                    "-".into()
                },
                item.branch.clone(),
                item.title.clone().unwrap_or_default(),
            ])
            .style(style)
        });
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(18),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(14),
                Constraint::Length(7),
                Constraint::Min(24),
                Constraint::Min(24),
            ],
        )
        .header(
            Row::new([
                "Projet",
                "Repository",
                "PR",
                "État",
                "Work items",
                "Local",
                "Branche",
                "Titre",
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

fn render_pull_request_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = pull_request_action_lines(app).join("\n");
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Opérations PR")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn pull_request_action_lines(app: &App) -> Vec<String> {
    let Some(item) = app.snapshot.pull_requests.get(app.selected_pull_request) else {
        return vec![
            "Aucune PR sélectionnée.".into(),
            "Entrer dans l'onglet PRs ou appuyer sur r charge les PRs actives.".into(),
            "Entrée/n: préparer workspace    x: créer workspace    f/F: finaliser".into(),
            "c: changements PR    d: diff local    j/k: sélectionner    r: rafraîchir".into(),
        ];
    };

    let pr = item
        .pull_request_id
        .map(|id| format!("#{id}"))
        .unwrap_or_else(|| "pas de PR active".into());
    let title = item.title.as_deref().unwrap_or("-");
    let suffix = item
        .error
        .as_ref()
        .map(|error| format!(" · {error}"))
        .unwrap_or_default();
    let workspace = item
        .workspace
        .as_ref()
        .map(|path| format!("Workspace local: {path}"))
        .unwrap_or_else(|| "Workspace local: aucun - x crée un workspace depuis la PR".into());
    let primary = if item.workspace.is_some() {
        app.selected_pull_request_action_preview_for(PullRequestAction::FinishExecute)
            .or_else(|| {
                app.selected_pull_request_action_preview_for(PullRequestAction::FinishPreview)
            })
            .map(|command| format!("Finaliser: {command}"))
            .unwrap_or_else(|| "Finaliser: indisponible pour cette ligne".into())
    } else {
        app.selected_pull_request_action_preview_for(PullRequestAction::StartExecute)
            .or_else(|| {
                app.selected_pull_request_action_preview_for(PullRequestAction::StartPreview)
            })
            .map(|command| format!("Créer workspace: {command}"))
            .unwrap_or_else(|| "Créer workspace: PR inexploitable".into())
    };
    let secondary = if item.workspace.is_some() {
        app.selected_pull_request_action_preview_for(PullRequestAction::DiffPreview)
            .map(|command| format!("Diff local: {command}"))
            .unwrap_or_else(|| "Diff local: indisponible".into())
    } else {
        app.selected_pull_request_action_preview_for(PullRequestAction::Changelog)
            .map(|command| format!("Changements: {command}"))
            .unwrap_or_else(|| "Changements: indisponible".into())
    };

    vec![
        format!(
            "{} / {} · {} · {}{}",
            item.project, item.repository, pr, title, suffix
        ),
        workspace,
        primary,
        secondary,
        "Entrée/n: préparer    x: créer workspace    f/F: finaliser    c: changements    d: diff"
            .into(),
    ]
}

fn render_workspace_summary(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = app
        .snapshot
        .workspaces
        .iter()
        .take(12)
        .map(|workspace| {
            ListItem::new(Line::from(vec![
                Span::styled(&workspace.project, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(
                    &workspace.display_work_items,
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(&workspace.slug, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect::<Vec<_>>();
    let items = if items.is_empty() {
        vec![ListItem::new("Aucun workspace task détecté.")]
    } else {
        items
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Workspaces récents")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_workspaces(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(7),
            Constraint::Min(7),
        ])
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
                workspace.project.clone(),
                workspace.display_work_items.clone(),
                workspace.kind.clone(),
                workspace.slug.clone(),
                workspace.repositories.join(", "),
            ])
            .style(style)
        });
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(22),
                Constraint::Length(10),
                Constraint::Length(24),
                Constraint::Min(16),
            ],
        )
        .header(
            Row::new(["Projet", "Work items", "Type", "Slug", "Repositories"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title(
                    "Workspaces  Entrée/o: ouvrir  p vérifier  s sync  l latest  v handoff  c commit  f/F finir  t/x supprimer",
                )
                .borders(Borders::ALL),
        ),
        chunks[0],
    );
    render_workspace_actions(frame, chunks[1], app);
    render_actions(frame, chunks[2], app);
}

fn render_workspace_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = workspace_action_lines(app).join("\n");
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Opérations workspace")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn workspace_action_lines(app: &App) -> Vec<String> {
    let Some(workspace) = app.snapshot.workspaces.get(app.selected_workspace) else {
        return vec![
            "Aucun workspace sélectionné.".into(),
            "n: action guidée    r: recharger    o: options".into(),
            "Les actions apparaissent dès qu'un workspace task existe.".into(),
        ];
    };

    let selected = format!(
        "{} · {} · {} · {}",
        workspace.project,
        workspace.display_work_items,
        workspace.slug,
        workspace.repositories.join(", ")
    );
    let preflight = app
        .selected_workspace_action_preview_for(WorkspaceAction::Preflight)
        .map(|command| format!("Vérifier: {command}"))
        .unwrap_or_else(|| "Vérifier: indisponible".into());
    let finish = app
        .selected_workspace_action_preview_for(WorkspaceAction::FinishExecute)
        .map(|command| format!("Finaliser: {command}"))
        .unwrap_or_else(|| "Finaliser: indisponible".into());
    let teardown = app
        .selected_workspace_action_preview_for(WorkspaceAction::TeardownExecute)
        .map(|command| format!("Supprimer: {command}"))
        .unwrap_or_else(|| "Supprimer: indisponible".into());

    vec![
        selected,
        preflight,
        finish,
        teardown,
        "Entrée/o: ouvrir    p: vérifier    s: sync    l: latest    v: handoff    c: commit    f/F: finir    t/x: supprimer"
            .into(),
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
            String::from("Aucune base configurée"),
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
            Row::new(["Scope", "Database", "Opération"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .title("Bases configurées  Entrée/s explorer  d décrire  e requête")
                .borders(Borders::ALL),
        ),
        chunks[0],
    );
    render_actions(frame, chunks[1], app);
}

fn render_config(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);
    let mut lines = vec![
        format!("Root       : {}", app.snapshot.root),
        format!("Couleur    : {}", app.snapshot.color_mode),
        format!("Agent      : {}", app.snapshot.default_agent()),
        format!("Projets    : {}", app.snapshot.project_count()),
        format!("Databases  : {}", app.snapshot.database_count()),
        format!("Workspaces : {}", app.snapshot.workspaces.len()),
        format!(
            "Doctor     : {}",
            if app.snapshot.config_doctor.passed {
                "valide"
            } else {
                "à corriger"
            }
        ),
        String::new(),
        "Vérifications".into(),
    ];
    lines.extend(config_doctor_lines(app).into_iter().take(7));
    lines.extend([
        String::new(),
        "Accélérateurs : s voir config, d diagnostic config, f rafraîchir, g guide, a diagnostic agents".into(),
        "Navigation : o options rapides, r recharger, Entrée lancer l'opération sélectionnée"
            .into(),
    ]);
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(
                Block::default()
                    .title("Configuration effective")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        chunks[0],
    );
    render_actions(frame, chunks[1], app);
}

fn config_doctor_lines(app: &App) -> Vec<String> {
    app.snapshot
        .config_doctor
        .checks
        .iter()
        .flat_map(|check| {
            let status = if check.passed { "OK" } else { "KO" };
            let mut lines = vec![format!("{status} {}", check.path)];
            if let Some(message) = check.message.as_deref() {
                lines.push(format!("   Détail : {message}"));
            }
            lines
        })
        .collect()
}

fn render_detail_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(detail) = &app.detail else {
        return;
    };
    let popup = centered_rect(82, 72, area);
    frame.render_widget(Clear, popup);
    let lines = detail_panel_lines(&detail.content);
    let title = detail.title();
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(Block::default().title(title.as_str()).borders(Borders::ALL))
            .scroll((detail.scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn detail_panel_lines(content: &DetailPanelContent) -> Vec<String> {
    match content {
        DetailPanelContent::Guide(lines) => lines.clone(),
        DetailPanelContent::ConfigShow(report) => config_show_detail_lines(report),
        DetailPanelContent::ConfigDoctor(report) => config_doctor_detail_lines(report),
        DetailPanelContent::AgentDoctor(report) => agent_doctor_detail_lines(report),
        DetailPanelContent::OperationResult { lines, .. } => lines.clone(),
    }
}

fn config_show_detail_lines(report: &dw_config::ConfigShow) -> Vec<String> {
    vec![
        format!("Root      : {}", report.root),
        format!("Couleur   : {}", report.color),
        format!("Réglages  : {}", report.settings_path),
        String::new(),
        "Fichiers".into(),
        config_file_detail_line("projects", &report.projects_path, report.projects_exists),
        config_file_detail_line("workflow", &report.workflow_path, report.workflow_exists),
        config_file_detail_line("databases", &report.databases_path, report.databases_exists),
        String::new(),
        "Esc: fermer    j/k: scroller    Home/End: début/fin".into(),
    ]
}

fn config_doctor_detail_lines(report: &dw_config::ConfigDoctorReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Statut    : {}",
            if report.passed {
                "valide"
            } else {
                "à corriger"
            }
        ),
        format!("Root      : {}", report.root),
        String::new(),
        "Vérifications".into(),
    ];
    for check in &report.checks {
        lines.push(config_check_detail_line(check));
        if let Some(message) = check.message.as_deref() {
            lines.push(format!("  Détail  : {message}"));
        }
    }
    lines.push(String::new());
    lines.push(if report.passed {
        "Résultat  : Configuration valide.".into()
    } else {
        "Résultat  : Configuration incomplète. Corriger les points signalés puis relancer le diagnostic."
            .into()
    });
    lines.push("Esc: fermer    j/k: scroller    Home/End: début/fin".into());
    lines
}

fn agent_doctor_detail_lines(report: &dw_agent::command::AgentDoctorReport) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Statut    : {}",
            if report.passed() {
                "agents disponibles"
            } else {
                "à corriger"
            }
        ),
        format!(
            "Disponibles: {}/{}",
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
            check.agent_name,
            check.command
        ));
        if !check.available {
            lines.push(format!(
                "  Action  : installer `{}` ou vérifier le PATH",
                check.command
            ));
        }
    }
    lines.push(String::new());
    lines.push("Esc: fermer    j/k: scroller    Home/End: début/fin".into());
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
        let message = if app.assigned_loading() {
            app.loading_elapsed_label(BackgroundKind::Assigned)
                .map(|elapsed| format!("Chargement des work items assignés depuis {elapsed}."))
                .unwrap_or_else(|| "Chargement des work items assignés en arrière-plan.".into())
        } else {
            "Entrer dans l'onglet ADO charge les work items assignés.".into()
        };
        frame.render_widget(
            Paragraph::new(message)
                .block(Block::default().title("Projets ADO").borders(Borders::ALL))
                .wrap(Wrap { trim: true }),
            area,
        );
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
        Line::from("Aucun projet configuré.")
    } else {
        Line::from(spans)
    };
    frame.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .title("Projets ADO  [ / ] ou J / K")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn render_ado_items(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if !app.snapshot.assigned_loaded {
        let message = if app.assigned_loading() {
            app.loading_elapsed_label(BackgroundKind::Assigned)
                .map(|elapsed| format!("Chargement en cours depuis {elapsed}.\nVous pouvez changer d'onglet; r relance le chargement."))
                .unwrap_or_else(|| {
                    "Chargement en cours.\nVous pouvez changer d'onglet; r relance le chargement.".into()
                })
        } else {
            "Chargement non lancé.\nr: recharger les données.".into()
        };
        frame.render_widget(
            Paragraph::new(message)
                .block(Block::default().title("Assigned").borders(Borders::ALL))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let Some(project) = app.snapshot.assigned.get(app.selected_ado_project) else {
        frame.render_widget(
            Paragraph::new("Configurer des projets Azure DevOps pour alimenter ce tableau.")
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
            Paragraph::new("Aucun work item assigné hors états finaux.")
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
                item.state.clone(),
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
                Constraint::Min(30),
            ],
        )
        .header(
            Row::new(["ID", "Type", "État", "Titre"]).style(
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
    let lines = ado_action_lines(app).join("\n");
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Opérations ADO")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn ado_action_lines(app: &App) -> Vec<String> {
    let selected = app
        .snapshot
        .assigned
        .get(app.selected_ado_project)
        .and_then(|project| {
            project.items.get(app.selected_ado_item).map(|item| {
                format!(
                    "{} #{} · {}",
                    project.key,
                    item.id,
                    if item.title.is_empty() {
                        "-"
                    } else {
                        &item.title
                    }
                )
            })
        })
        .unwrap_or_else(|| "Aucun work item sélectionné.".into());
    let state_preview = app
        .selected_ado_set_state_action_preview()
        .map(|command| format!("État workflow: {command}"))
        .unwrap_or_else(|| "État workflow: aucun état configuré pour ce type".into());
    vec![
        selected,
        state_preview,
        "Entrée/n: préparer workspace    x: créer workspace    e/E: état workflow    c: contexte    w: fiche"
            .into(),
        "j/k: work item    J/K ou [/]: projet    r: rafraîchir".into(),
    ]
}

fn render_actions(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);
    let filter = if app.filter_active {
        format!("Recherche: {}_", app.filter)
    } else if app.filter.is_empty() {
        "Recherche: /".into()
    } else {
        format!("Recherche: {}", app.filter)
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
        vec![ListItem::new("Aucune action disponible pour ce filtre.")]
    } else {
        items
    };
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title("Opérations disponibles")
                .borders(Borders::ALL),
        ),
        chunks[1],
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let help = help_lines().join("\n");
    frame.render_widget(
        Paragraph::new(help)
            .block(Block::default().title("Aide").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new(styled_shortcut_line(&shortcut_bar_line(app)))
            .block(Block::default().title("Raccourcis").borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn styled_shortcut_line(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    for (segment_index, segment) in text.split(" | ").enumerate() {
        if segment_index > 0 {
            spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        if let Some((shortcut, label)) = segment.split_once(':') {
            let shortcut_style = Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            spans.push(Span::styled(shortcut.to_owned(), shortcut_style));
            spans.push(Span::styled(
                ":".to_owned(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::raw(label.to_owned()));
        } else {
            spans.push(Span::raw(segment.to_owned()));
        }
    }
    Line::from(spans)
}

fn database_rows(app: &App) -> Vec<[String; 3]> {
    app.snapshot
        .database_entries
        .iter()
        .map(|database| {
            let scope = database.project.clone().unwrap_or_else(|| "global".into());
            let action = if let Some(project) = database.project.as_deref() {
                format!("Schema ({project}/{})", database.key)
            } else {
                format!("Schema ({})", database.key)
            };
            [scope, database.key.clone(), action]
        })
        .collect()
}

fn render_confirmation(frame: &mut Frame<'_>, area: Rect, action: &TuiAction) {
    let popup = centered_rect(70, 28, area);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(confirmation_lines(action).join("\n"))
            .block(
                Block::default()
                    .title(action.kind.confirmation_title())
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true }),
        popup,
    );
}

fn render_options(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(82, 70, area);
    frame.render_widget(Clear, popup);
    let current_agent = app.snapshot.default_agent();
    let current_color = app.snapshot.color_mode.as_str();
    let block = Block::default().title("Options").borders(Borders::ALL);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(options_summary_lines(app).join("\n")).wrap(Wrap { trim: true }),
        chunks[0],
    );

    let rows = QUICK_OPTIONS.iter().enumerate().map(|(index, item)| {
        let selected = index == app.selected_option;
        let active = option_active(item.state, &current_agent, current_color);
        let style = if selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else if active {
            Style::default().fg(Color::LightGreen)
        } else {
            Style::default()
        };
        Row::new([
            item.key.to_string(),
            item.section.into(),
            item.label.into(),
            if active {
                "actif".into()
            } else {
                String::new()
            },
            item.hint.into(),
        ])
        .style(style)
    });
    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(5),
                Constraint::Length(24),
                Constraint::Length(18),
                Constraint::Length(8),
                Constraint::Min(28),
            ],
        )
        .header(
            Row::new(["Key", "Groupe", "Option", "État", "Opération"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ),
        chunks[1],
    );

    let selected = QUICK_OPTIONS
        .get(app.selected_option)
        .map(|option| {
            crate::actions::option_action(&app.snapshot.root, option.action).display_label()
        })
        .unwrap_or_else(|| "Aucune option sélectionnée.".into());
    let shortcuts = quick_option_shortcut_hint();
    frame.render_widget(
        Paragraph::new(format!(
            "{selected}\nEntrée: lancer    j/k: sélectionner    raccourcis: {shortcuts}    Esc/o: fermer"
        ))
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .wrap(Wrap { trim: true }),
        chunks[2],
    );
}

fn render_history_output(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup = centered_rect(82, 72, area);
    frame.render_widget(Clear, popup);
    let lines = history_output_lines(app);
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(Block::default().title("Lancements").borders(Borders::ALL))
            .scroll((app.history.output_scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        popup,
    );
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
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .block(
                Block::default()
                    .title("État et messages")
                    .borders(Borders::ALL),
            )
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn render_form(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(form) = &app.form else {
        return;
    };
    let popup = centered_rect(78, 72, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .title("Constructeur d’action")
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
                "Choisir un template (Entrée)"
            } else {
                "Constructeur avancé · choisir un template"
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
                    "oui".into()
                } else {
                    "non".into()
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
            Row::new(["Champ", "Valeur", "Aide"]).style(
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
    fn ado_action_lines_preview_workflow_state_action() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned_loaded = true;
        app.snapshot.assigned = vec![ado_project("ha", "User Story")];

        let lines = ado_action_lines(&app);

        assert!(lines[0].contains("ha #42"));
        assert!(lines[1].contains("Passer à l’état"));
        assert!(lines[2].contains("e/E: état workflow"));
    }

    #[test]
    fn ado_action_lines_explain_missing_workflow_state_mapping() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.assigned_loaded = true;
        app.snapshot.assigned = vec![ado_project("ha", "Epic")];

        let lines = ado_action_lines(&app);

        assert_eq!(lines[1], "État workflow: aucun état configuré pour ce type");
    }

    #[test]
    fn workspace_action_lines_preview_primary_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.workspaces = vec![workspace("/tmp/ws-front", "demo")];

        let lines = workspace_action_lines(&app);

        assert!(lines[0].contains("ha · #42 Demo · demo · front"));
        assert!(lines[1].contains("Vérifier"));
        assert!(lines[2].contains("Finaliser workspace"));
        assert!(lines[3].contains("Supprimer workspace"));
    }

    #[test]
    fn pull_request_action_lines_preview_local_finish_and_diff() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![pull_request(Some("/tmp/ws-front"))];

        let lines = pull_request_action_lines(&app);

        assert!(lines[0].contains("ha / front · #42"));
        assert_eq!(lines[1], "Workspace local: /tmp/ws-front");
        assert!(lines[2].contains("Finaliser PR"));
        assert!(lines[3].contains("Diff"));
    }

    #[test]
    fn pull_request_action_lines_preview_remote_workspace_creation() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.pull_requests_loaded = true;
        app.snapshot.pull_requests = vec![pull_request(None)];

        let lines = pull_request_action_lines(&app);

        assert_eq!(
            lines[1],
            "Workspace local: aucun - x crée un workspace depuis la PR"
        );
        assert!(lines[2].contains("Créer workspace PR"));
        assert!(lines[3].contains("Résumer changements"));
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
    fn config_doctor_lines_show_status_and_details() {
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
                    message: Some("Fichier introuvable".into()),
                },
            ],
        };

        let lines = config_doctor_lines(&app);

        assert!(lines[0].starts_with("OK "));
        assert!(lines[1].starts_with("KO "));
        assert!(lines[2].contains("Fichier introuvable"));
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
            pull_request_id: Some(42),
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
