use crate::actions::QuickOptionState;
use crate::app::App;
use crate::form::FormState;
#[cfg(test)]
use crate::history::{ActionRunErrorMessage, ActionRunId, ActionRunLabel};
use crate::history::{ActionRunRecord, ActionRunStatus, RunHistoryEntry};
#[cfg(test)]
use crate::model::View;
use crate::model::{ActionRisk, TuiAction};
use dw_core::{Agent, ConfigColorMode};

pub(crate) fn help_lines() -> Vec<&'static str> {
    vec![
        "Navigation",
        "next view [Tab]",
        "previous view [Shift-Tab]",
        "main views [1-6]",
        "selection down [j]",
        "selection up [k]",
        "reload data [r]",
        "",
        "Actions",
        "run selected operation [Enter]",
        "open operation composer [n]",
        "open menu [m]",
        "help [?]",
        "",
        "Domain views show their own action buttons at the bottom of the active panel.",
        "ADO work items and PRs preload in the background when the TUI starts.",
        "",
        "Quit",
        "quit [q]",
        "quit [Esc]",
    ]
}

#[cfg(test)]
pub(crate) fn shortcut_bar_line(app: &App) -> String {
    let running = app
        .running_action_label()
        .map(|label| format!(" | running [{label}]"))
        .unwrap_or_default();
    format!(
        "{} | menu [m] | help [?] | quit [q]{}",
        view_hint(app),
        running
    )
}

pub(crate) fn state_modal_lines(app: &App) -> Vec<String> {
    let mut lines = vec!["Loads".into()];
    lines.extend(app.background_status_lines());
    if let Some(status) = app.active_action_status_text() {
        lines.push(String::new());
        lines.push("Current operation".into());
        lines.push(status);
    }
    let queued = app.action_queue_status_lines();
    if !queued.is_empty() {
        lines.push(String::new());
        lines.push("Action queue".into());
        lines.extend(queued);
    }
    lines.push(String::new());
    lines.push("Messages".into());
    if app.messages.is_empty() {
        lines.push("No messages.".into());
    } else {
        lines.extend(app.messages.iter().rev().take(12).rev().cloned());
    }
    lines
}

pub(crate) fn guide_detail_lines() -> Vec<String> {
    vec![
        "DevWorkflow quick guide".into(),
        String::new(),
        "1. Check the environment".into(),
        "   Open Config, then run configuration and agent diagnostics.".into(),
        "   Fix blocking checks before creating workspaces.".into(),
        String::new(),
        "2. Read the cockpit".into(),
        "   Dashboard prioritizes PRs without workspace, active workspaces, assigned work items and alerts.".into(),
        "   Enter runs the selected row's primary operation.".into(),
        String::new(),
        "3. Process ADO and PRs".into(),
        "   ADO: select a project and work item, then prepare, add context or open the card.".into(),
        "   PRs: load active PRs, create a workspace, finish or open the PR.".into(),
        String::new(),
        "4. Work a workspace".into(),
        "   Workspaces: open the agent, check, sync, prepare handoff or finish.".into(),
        "   Destructive actions require explicit TUI confirmation.".into(),
        String::new(),
        "5. Explore data".into(),
        "   DB: explore schema, describe a table or run a guided read-only query.".into(),
        "   Long results open in the Journal modal.".into(),
        String::new(),
        "6. Build an advanced operation".into(),
        "   Composer: choose a flow, fill fields, apply suggestions.".into(),
        "   Preview shows the TUI intent and risk level, not a command to copy.".into(),
        String::new(),
        "Inside the TUI".into(),
        "   Modals display read results.".into(),
        "   Background operations stay in Journal.".into(),
    ]
}

pub(crate) fn history_marker(entry: &RunHistoryEntry) -> &'static str {
    match entry.status {
        ActionRunStatus::Running => "...",
        ActionRunStatus::Succeeded => "OK",
        ActionRunStatus::Failed => "KO",
    }
}

#[cfg(test)]
pub(crate) fn view_hint(app: &App) -> &'static str {
    match app.view {
        View::Dashboard => "cockpit down [j]    cockpit up [k]    decide [Enter]    reload [r]",
        View::Workspaces => {
            "workspace down [J]    workspace up [K]    open [o]    check [p]    sync [s]    latest [l]    handoff [v]    commit [c]    finish preview [f]    finish execute [F]    remove preview [t]    remove execute [x]"
        }
        View::Ado if app.assigned_loading() => {
            "ADO is loading in the background; you can keep navigating."
        }
        View::Ado => {
            "project previous [K]    project next [J]    item down [j]    item up [k]    prepare workspace [n]    create workspace [x]    move state [e]    state form [E]    context [c]    card [w]    open ADO [u]"
        }
        View::PullRequests if app.pull_requests_loading() => {
            "PRs are loading in the background; you can keep navigating."
        }
        View::PullRequests => {
            "prepare workspace [n]    create workspace [x]    PR form [N]    finish preview [f]    finish execute [F]    changes [c]    diff [d]    open agent [o]    open PR [u]"
        }
        View::Db => {
            "explore schema [Enter]    explore schema [s]    describe table [d]    guided query [e]"
        }
        View::Composer => {
            "edit or run [Enter]    select down [j]    select up [k]    suggestion [Ctrl+Space]    flows [Esc]    next view [Tab]"
        }
    }
}

pub(crate) fn confirmation_lines(action: &TuiAction) -> Vec<String> {
    let mut lines = Vec::new();
    match action.kind {
        ActionRisk::Safe => {
            lines.push("This reads or inspects data; no expected modification.".into());
        }
        ActionRisk::OpensExternal => {
            lines.push("This opens an external or interactive process.".into());
            lines.push("Return here when that process exits.".into());
        }
        ActionRisk::DryRun => {
            lines.push(
                "Preview only: review the returned plan before choosing an execute action.".into(),
            );
        }
        ActionRisk::Destructive => {
            if action.bypasses_cli_confirmation() {
                lines.push("Destructive confirmation is carried by the TUI request.".into());
            } else {
                lines.push("This may modify or delete data, workspaces, or remote state.".into());
            }
            lines.push("Review the Operation and Effect before confirming.".into());
        }
    }
    lines
}

pub(crate) fn options_summary_lines(app: &App) -> Vec<String> {
    vec![
        format!("Root: {}", app.snapshot.root),
        format!(
            "Config: {} projects · {} repositories · {} DB · doctor {}",
            app.snapshot.project_count(),
            app.snapshot.repository_count(),
            app.snapshot.database_count(),
            if app.snapshot.config_doctor.passed {
                "OK"
            } else {
                "KO"
            }
        ),
        format!(
            "Agent: {}    Color: {}",
            app.snapshot.default_agent(),
            app.snapshot.color_mode
        ),
    ]
}

pub(crate) fn history_journal_lines(app: &App) -> Vec<String> {
    let Some(entry) = app.history.selected_entry() else {
        return vec![
            "No operation in Journal.".into(),
            "Long-running or background actions will appear here with their logs.".into(),
        ];
    };
    let marker = history_marker(entry);
    let mut lines = vec![
        format!(
            "Run         : {}/{}",
            app.history.selected_entry + 1,
            app.history.entries.len()
        ),
        format!("{marker} {} ({})", entry.request_label, entry.status),
    ];
    if entry.status.is_running() {
        lines.push(
            active_operation_line(entry)
                .map(|line| format!("Now         : {line}"))
                .unwrap_or_else(|| "Now         : waiting for first event".into()),
        );
        lines.push(
            "Output is being captured; closing this modal does not interrupt the action.".into(),
        );
    } else {
        lines.push("Output captured.".into());
    }
    lines.push(String::new());
    let rendered = history_entry_rendered_lines(entry);
    if rendered.is_empty() {
        if entry.status.is_running() {
            lines.push("Waiting for the first output line...".into());
        } else {
            lines.push("No output captured for this run.".into());
        }
    } else {
        lines.extend(rendered);
    }
    lines
}

fn active_operation_line(entry: &RunHistoryEntry) -> Option<String> {
    entry.latest_event().map(dw_ui::action_event_line)
}

pub(crate) fn history_entry_rendered_lines(entry: &RunHistoryEntry) -> Vec<String> {
    let theme = dw_ui::TerminalTheme::plain();
    let lines = match &entry.record {
        ActionRunRecord::Running { events } => events
            .iter()
            .map(dw_ui::action_event_line)
            .collect::<Vec<_>>(),
        ActionRunRecord::Completed { events, result } => {
            let mut lines = events
                .iter()
                .map(dw_ui::action_event_line)
                .collect::<Vec<_>>();
            let result_lines = dw_tui_adapter::render::action_result_lines(result.as_ref(), &theme);
            if !lines.is_empty() && !result_lines.is_empty() {
                lines.push(String::new());
            }
            lines.extend(result_lines);
            lines
        }
        ActionRunRecord::ExternalLaunch { plan } => {
            dw_tui_adapter::render::task_open_launch_lines(plan)
        }
        ActionRunRecord::Failed { events, error } => {
            let mut lines = events
                .iter()
                .map(dw_ui::action_event_line)
                .collect::<Vec<_>>();
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(error.to_string());
            lines
        }
    };
    capped_history_lines(lines)
}

#[cfg(test)]
pub(crate) fn history_entry_preview_lines(entry: &RunHistoryEntry) -> Vec<String> {
    history_entry_rendered_lines(entry)
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn capped_history_lines(mut lines: Vec<String>) -> Vec<String> {
    const MAX_OUTPUT_LINES: usize = 160;
    if lines.len() <= MAX_OUTPUT_LINES {
        return lines;
    }
    let omitted = lines.len() - MAX_OUTPUT_LINES;
    let mut kept = Vec::with_capacity(MAX_OUTPUT_LINES + 1);
    kept.push(format!("... {omitted} previous lines hidden ..."));
    kept.extend(lines.drain(omitted..));
    kept
}

pub(crate) fn option_active(
    state: QuickOptionState,
    current_agent: Agent,
    current_color: ConfigColorMode,
) -> bool {
    match state {
        QuickOptionState::Agent(agent) => current_agent == agent,
        QuickOptionState::Color(color) => current_color == color,
        QuickOptionState::None => false,
    }
}

pub(crate) fn form_preview_lines(app: &App) -> Vec<String> {
    let Some(form) = &app.form else {
        return vec!["No form open.".into()];
    };
    form_preview_lines_for(form, app)
}

pub(crate) fn action_builder_preview_lines(app: &App) -> Vec<String> {
    form_preview_lines_for(&app.action_form, app)
}

fn form_preview_lines_for(form: &FormState, app: &App) -> Vec<String> {
    let mut lines = Vec::new();
    let missing = form.missing_required_fields();
    let invalid = form.invalid_field_messages();
    match form.build_action(&app.snapshot.root) {
        Some(action) => {
            lines.push(action.display_label());
            lines.push(format!("Risk: {}", action.kind.risk_label()));
            if matches!(action.kind, ActionRisk::Destructive) && action.bypasses_cli_confirmation()
            {
                lines.push(
                    "Destructive confirmation is carried by the TUI request; TUI confirmation required."
                        .into(),
                );
            }
        }
        None => {
            lines.push("Incomplete action".into());
        }
    }
    if !missing.is_empty() {
        lines.push(format!("Required: {}", missing.join(", ")));
    }
    lines.extend(invalid.into_iter().map(|message| format!("Fix: {message}")));
    if let Some(field) = form.fields.get(form.selected_field) {
        let value = if field.value.trim().is_empty() {
            "<empty>"
        } else {
            field.value.as_str()
        };
        lines.push(format!("Field: {} = {}", field.label, value));
        if !field.help.trim().is_empty() {
            lines.push(format!("Help: {}", field.help));
        }
    }
    if let Some(value) = form.selected_suggestion(&app.snapshot) {
        lines.push(format!("Suggestion: {value}"));
    }
    lines.push(
        "run [Enter]    field with arrows/j/k    suggestion [Ctrl+Space]    toggle [Space]    cancel [Esc]    next view [Tab]"
            .into(),
    );
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{FormState, FormTemplate};
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

    #[test]
    fn dashboard_hint_points_to_accelerators() {
        let app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        assert!(view_hint(&app).contains("cockpit"));
        assert!(view_hint(&app).contains("decide"));
    }

    #[test]
    fn dashboard_hint_stays_actionable_while_snapshot_loads() {
        let root = initialized_root();
        let app = App::new(Some(root.path().display().to_string()));

        assert!(app.snapshot_loading());
        assert!(view_hint(&app).contains("cockpit"));
        assert!(view_hint(&app).contains("reload"));
    }

    #[test]
    fn actions_hint_targets_builder_not_cli_catalog() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Composer;

        assert!(view_hint(&app).contains("edit or run"));
        assert!(shortcut_bar_line(&app).contains("edit or run [Enter]"));
        assert!(!shortcut_bar_line(&app).contains("target"));
    }

    #[test]
    fn shortcut_bar_contains_only_shortcuts_not_selected_context() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        let dashboard = shortcut_bar_line(&app);
        assert!(!dashboard.contains("target"));
        assert!(!dashboard.contains("Guide"));
        assert!(!dashboard.contains("menu/config"));
        assert_eq!(dashboard.matches("menu [m]").count(), 1);

        app.view = View::PullRequests;
        app.snapshot.pull_requests = vec![crate::model::TuiPullRequest {
            workspace: None,
            project: "ha".into(),
            repository: "back".into(),
            ado_repository: "back".into(),
            branch: "feature/55265-demo".into(),
            target_branch: "develop".into(),
            pull_request_id: Some(dw_core::PullRequestId::from("55265")),
            title: Some("Corriger footer".into()),
            is_draft: false,
            work_item_ids: vec!["55265".into()],
            url: None,
            error: None,
        }];

        let prs = shortcut_bar_line(&app);
        assert!(prs.contains("diff [d]"));
        assert!(!prs.contains("target"));
        assert!(!prs.contains("#55265"));
        assert!(!prs.contains("Corriger footer"));
        assert!(!prs.contains("Preview"));
        assert!(!prs.contains("workspace PR"));
    }

    #[test]
    fn ado_hint_lists_native_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Ado;

        let hint = view_hint(&app);

        assert!(hint.contains("prepare workspace"));
        assert!(hint.contains("move state"));
        assert!(hint.contains("move state [e]"));
        assert!(hint.contains("state form [E]"));
        assert!(!hint.contains("n: "));
        assert!(!hint.contains("e/E: "));
        assert!(hint.contains("context"));
        assert!(hint.contains("open ADO"));
    }

    #[test]
    fn workspace_hint_lists_native_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::Workspaces;

        let hint = view_hint(&app);

        assert!(hint.contains("check"));
        assert!(hint.contains("sync"));
        assert!(hint.contains("latest"));
        assert!(hint.contains("handoff"));
        assert!(hint.contains("finish"));
        assert!(hint.contains("remove"));
    }

    #[test]
    fn pull_request_hint_lists_review_actions() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.view = View::PullRequests;

        let hint = view_hint(&app);

        assert!(hint.contains("prepare workspace"));
        assert!(hint.contains("PR form"));
        assert!(hint.contains("finish"));
        assert!(hint.contains("changes"));
        assert!(hint.contains("diff"));
        assert!(hint.contains("open agent"));
        assert!(hint.contains("open PR"));
    }

    #[test]
    fn config_is_reached_through_menu_shortcut() {
        let app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        let hint = shortcut_bar_line(&app);

        assert!(hint.contains("menu [m]"));
        assert!(!View::ALL.iter().any(|view| view.label() == "Config"));
    }

    #[test]
    fn help_mentions_start_pr_form_prefill() {
        let help = help_lines().join("\n");

        assert!(help.contains("operation composer [n]"));
        assert!(help.contains("preload in the background"));
    }

    #[test]
    fn help_mentions_direct_view_shortcuts_and_help_key() {
        let help = help_lines().join("\n");

        assert!(help.contains("main views [1-6]"));
        assert!(help.contains("help [?]"));
    }

    #[test]
    fn options_summary_lines_show_loaded_config_counts() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.snapshot.projects = serde_json::from_str(
            r#"{
  "projects": {
    "ha": {
      "displayName": "HA",
      "repositories": {
        "front": { "url": "", "defaultBranch": "develop" },
        "back": { "url": "", "defaultBranch": "main" }
      }
    }
  }
}"#,
        )
        .expect("projects config");
        app.snapshot.databases.globals.insert(
            "shared".into(),
            serde_json::json!({"provider": "sqlserver"}),
        );
        app.snapshot.color_mode = dw_core::ConfigColorMode::Always;
        app.snapshot.config_doctor.passed = true;

        let lines = options_summary_lines(&app);

        assert!(lines[0].contains(app.snapshot.root.as_str()));
        assert!(lines[1].contains("1 projects"));
        assert!(lines[1].contains("2 repositories"));
        assert!(lines[1].contains("1 DB"));
        assert!(lines[1].contains("doctor OK"));
        assert!(lines[2].contains("Color: always"));
    }

    #[test]
    fn confirmation_lines_do_not_duplicate_action_buttons() {
        let action = TuiAction {
            label: "Teardown execute".into(),
            request: crate::model::TuiActionRequest::TaskTeardown(dw_task::repo::TeardownArgs {
                workspace: Some(dw_core::WorkspacePath::from("/tmp/ws")),
                root: None,
                project: None,
                work_item_ids: Vec::new(),
                r#continue: false,
                mode: dw_core::ExecutionMode::Execute,
                yes: true,
            }),
            description: "Remove workspace".into(),
            kind: ActionRisk::Destructive,
        };

        let lines = confirmation_lines(&action);
        let joined = lines.join("\n");

        assert!(lines.iter().any(|line| line.contains("TUI request")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Review the Operation"))
        );
        assert!(!joined.contains("Enter/"));
        assert!(!joined.contains("Esc/"));
        assert!(!joined.contains("Teardown execute"));
    }

    #[test]
    fn form_preview_lines_show_risk_and_injected_yes() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::AdoSetState)
            .expect("ado set-state template");
        form.begin_editing(&app.snapshot);
        for field in &mut form.fields {
            match field.label.as_str() {
                "Work item IDs" => field.value = "42".into(),
                "Project" => field.value = "ha".into(),
                "Destination state" => field.value = "En réalisation".into(),
                _ => {}
            }
        }
        app.form = Some(form);

        let lines = form_preview_lines(&app);

        assert!(lines[0].contains("Composer · Move work item state"));
        assert!(lines.iter().any(|line| line.contains("Risk:")));
        assert!(lines.iter().any(|line| line.contains("TUI request")));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Field: Work item IDs"))
        );
        assert!(lines.iter().any(|line| line.contains("Help: ADO IDs")));
    }

    #[test]
    fn form_preview_lines_explain_incomplete_action() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbQuery)
            .expect("db query template");
        form.begin_editing(&app.snapshot);
        form.fields
            .iter_mut()
            .find(|field| field.label == "SQL")
            .expect("sql field")
            .value
            .clear();
        app.form = Some(form);

        let lines = form_preview_lines(&app);

        assert_eq!(lines[0], "Incomplete action");
        assert!(lines.iter().any(|line| line == "Required: SQL"));
        assert!(lines.iter().any(|line| line == "Field: Project = <empty>"));
        assert!(
            lines
                .iter()
                .any(|line| line == "Help: Optional configured project")
        );
        assert!(lines.last().is_some_and(|line| line.contains("Enter")));
    }

    #[test]
    fn form_preview_lines_warn_about_invalid_numeric_fields() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let mut form = FormState::selecting();
        form.template_index = FormTemplate::ALL
            .iter()
            .position(|template| *template == FormTemplate::DbQuery)
            .expect("db query template");
        form.begin_editing(&app.snapshot);
        form.fields
            .iter_mut()
            .find(|field| field.label == "Max rows")
            .expect("max rows field")
            .value = "many".into();
        app.form = Some(form);

        let lines = form_preview_lines(&app);

        assert!(
            lines
                .iter()
                .any(|line| line == "Fix: Max rows must be a whole number.")
        );
    }

    #[test]
    fn history_marker_distinguishes_running_success_and_failure() {
        let running = RunHistoryEntry {
            id: ActionRunId::new(1),
            request_label: ActionRunLabel::new("Task finish"),
            status: ActionRunStatus::Running,
            record: ActionRunRecord::running(),
        };
        let success = RunHistoryEntry {
            status: ActionRunStatus::Succeeded,
            ..running.clone()
        };
        let failure = RunHistoryEntry {
            status: ActionRunStatus::Failed,
            ..running.clone()
        };

        assert_eq!(history_marker(&running), "...");
        assert_eq!(history_marker(&success), "OK");
        assert_eq!(history_marker(&failure), "KO");
    }

    #[test]
    fn history_journal_lines_explain_running_capture() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history
            .start_running(ActionRunId::new(1), ActionRunLabel::new("Task finish"));

        let lines = history_journal_lines(&app);

        assert!(lines[1].contains("running"));
        assert!(lines[2].contains("waiting for first event"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Output is being captured"))
        );
        assert!(lines.iter().any(|line| line.contains("first output line")));
    }

    #[test]
    fn history_journal_lines_show_latest_running_event() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let latest = dw_core::DwActionEvent::Started {
            action_id: "task.finish".into(),
        };
        app.history
            .start_running(ActionRunId::new(1), ActionRunLabel::new("Task finish"));
        app.history.append_running_event(
            ActionRunId::new(1),
            dw_core::DwActionEvent::Started {
                action_id: "task.start".into(),
            },
        );
        app.history
            .append_running_event(ActionRunId::new(1), latest.clone());

        let lines = history_journal_lines(&app);
        let expected = dw_ui::action_event_line(&latest);

        assert_eq!(lines[2], format!("Now         : {expected}"));
    }

    #[test]
    fn state_modal_lines_show_current_operation() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        let latest = dw_core::DwActionEvent::Started {
            action_id: "task.finish".into(),
        };
        app.history
            .start_running(ActionRunId::new(1), ActionRunLabel::new("Task finish"));
        app.history
            .append_running_event(ActionRunId::new(1), latest.clone());

        let lines = state_modal_lines(&app);
        let expected = dw_ui::action_event_line(&latest);

        assert!(lines.iter().any(|line| line == "Current operation"));
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("Task finish -> {expected}"))
        );
    }

    #[test]
    fn history_journal_lines_explain_empty_journal() {
        let app = App::new_ready(Some("/tmp/missing-dw-root".into()));

        let lines = history_journal_lines(&app);

        assert!(lines[0].contains("No operation"));
        assert!(lines[1].contains("background"));
        assert!(!lines.iter().any(|line| line.contains("Esc/h")));
    }

    #[test]
    fn history_journal_lines_render_failed_record_events_and_error() {
        let mut app = App::new_ready(Some("/tmp/missing-dw-root".into()));
        app.history.push(RunHistoryEntry {
            id: ActionRunId::new(1),
            request_label: ActionRunLabel::new("Doctor"),
            status: ActionRunStatus::Failed,
            record: ActionRunRecord::failed(
                vec![dw_core::DwActionEvent::Started {
                    action_id: "prepare".into(),
                }],
                ActionRunErrorMessage::new("boom"),
            ),
        });

        let lines = history_journal_lines(&app);

        assert_eq!(lines[2], "Output captured.");
        assert!(lines.iter().any(|line| line.contains("prepare")));
        assert!(lines.iter().any(|line| line == "boom"));
    }

    #[test]
    fn option_active_matches_agent_and_color() {
        assert!(option_active(
            QuickOptionState::Agent(Agent::Codex),
            Agent::Codex,
            ConfigColorMode::Auto
        ));
        assert!(option_active(
            QuickOptionState::Color(ConfigColorMode::Always),
            Agent::Codex,
            ConfigColorMode::Always
        ));
        assert!(!option_active(
            QuickOptionState::Agent(Agent::Opencode),
            Agent::Codex,
            ConfigColorMode::Auto
        ));
        assert!(!option_active(
            QuickOptionState::None,
            Agent::Codex,
            ConfigColorMode::Auto
        ));
    }
}
