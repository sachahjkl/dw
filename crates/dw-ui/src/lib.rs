mod ado_changelog;

pub use ado_changelog::render_ado_changelog_document;

pub fn diagnostic_log_event_line(event: &dw_core::DiagnosticLogEvent) -> String {
    format!(
        "{} [{}] {}",
        match event.level {
            dw_core::DiagnosticLogLevel::Warning => "WARN",
            dw_core::DiagnosticLogLevel::Info => "INFO",
            dw_core::DiagnosticLogLevel::Debug => "DEBUG",
        },
        event.target,
        event.detail
    )
}

pub fn task_action_event_line(event: &dw_core::TaskActionEvent) -> String {
    let action_id = event.action_id();
    match event {
        dw_core::TaskActionEvent::ExecutingStart {
            workspace,
            repository_count,
        } => format!("{action_id} workspace={workspace} repositories={repository_count}"),
        dw_core::TaskActionEvent::SyncLoadingWorkItems
        | dw_core::TaskActionEvent::SyncWritingManifest => action_id.to_string(),
        dw_core::TaskActionEvent::ExecutingRepoLatest { repository_count } => {
            format!("{action_id} repositories={repository_count}")
        }
        dw_core::TaskActionEvent::ExecutingAddRepo { repository } => {
            format!("{action_id} repository={repository}")
        }
        dw_core::TaskActionEvent::PlanningStart {
            project,
            work_item_ids,
        } => format!(
            "{action_id} project={project} workitems={}",
            join_display(work_item_ids)
        ),
        dw_core::TaskActionEvent::LoadingStartWorkItems {
            project,
            work_item_ids,
        } => format!(
            "{action_id} project={project} workitems={}",
            join_display(work_item_ids)
        ),
        dw_core::TaskActionEvent::BuildingStartPlan {
            project,
            repositories,
        } => format!(
            "{action_id} project={project} repositories={}",
            join_display(repositories)
        ),
        dw_core::TaskActionEvent::ResolvingPullRequestWorkItems { pull_request_id } => {
            format!("{action_id} pull_request=#{pull_request_id}")
        }
        dw_core::TaskActionEvent::ResolvedPullRequestWorkItems { work_item_ids } => {
            format!("{action_id} workitems={}", join_display(work_item_ids))
        }
        dw_core::TaskActionEvent::VerifyingFinish {
            pull_request_candidate_count,
        } => format!("{action_id} candidates={pull_request_candidate_count}"),
        dw_core::TaskActionEvent::FinishVerificationCompleted => action_id.to_string(),
        dw_core::TaskActionEvent::RunningGitOperation {
            operation,
            repository_count,
        } => format!(
            "{action_id} operation={} repositories={repository_count}",
            git_operation_key(*operation)
        ),
        dw_core::TaskActionEvent::RunningRepositoryGitOperation {
            repository,
            operation,
        } => format!(
            "{action_id} repository={repository} operation={}",
            git_operation_key(*operation)
        ),
        dw_core::TaskActionEvent::GitOperationCompleted { operation } => {
            format!("{action_id} operation={}", git_operation_key(*operation))
        }
        dw_core::TaskActionEvent::SkippingPullRequestCreation => action_id.to_string(),
        dw_core::TaskActionEvent::AuthenticatingAdoForPullRequests {
            pull_request_candidate_count,
        } => format!("{action_id} candidates={pull_request_candidate_count}"),
        dw_core::TaskActionEvent::CheckingActivePullRequest { repository }
        | dw_core::TaskActionEvent::CreatingPullRequest { repository } => {
            format!("{action_id} repository={repository}")
        }
        dw_core::TaskActionEvent::PullRequestWorkItemLinkSkipped {
            work_item_id,
            error,
        } => format!("{action_id} workitem=#{work_item_id} error={error}"),
        dw_core::TaskActionEvent::UpdatingFinishWorkItemStates { work_item_ids } => {
            format!("{action_id} workitems={}", join_display(work_item_ids))
        }
    }
}

pub fn ado_action_event_line(event: &dw_core::AdoActionEvent) -> String {
    let action_id = event.action_id();
    match event {
        dw_core::AdoActionEvent::Authenticating { project } => format!(
            "{action_id} project={}",
            project
                .as_ref()
                .map(|project| project.to_string())
                .unwrap_or_else(|| "resolved".into())
        ),
        dw_core::AdoActionEvent::DeviceLoginRequired {
            verification_uri,
            user_code,
            expires_in_seconds,
            poll_interval_seconds,
        } => format!(
            "{action_id} uri={verification_uri} code={user_code} expires={expires_in_seconds}s polling={poll_interval_seconds}s"
        ),
        dw_core::AdoActionEvent::LoadingAssignedWorkItems { project, top } => {
            format!("{action_id} project={project} top={top}")
        }
        dw_core::AdoActionEvent::GroupingAssignedWorkItems { project }
        | dw_core::AdoActionEvent::LoadingPullRequests { project } => {
            format!("{action_id} project={project}")
        }
        dw_core::AdoActionEvent::ResolvingPullRequestWorkItems { repositories } => {
            format!("{action_id} repositories={}", join_display(repositories))
        }
        dw_core::AdoActionEvent::ExtractingGitWorkItems { git_to } => {
            format!("{action_id} to={git_to}")
        }
        dw_core::AdoActionEvent::LoadingWorkItem { id }
        | dw_core::AdoActionEvent::LoadingWorkItemContext { id } => {
            format!("{action_id} workitem=#{id}")
        }
        dw_core::AdoActionEvent::LoadingWorkItems { ids }
        | dw_core::AdoActionEvent::LoadingChangelog { ids }
        | dw_core::AdoActionEvent::LoadingChangelogItems { ids } => {
            format!("{action_id} workitems={}", join_display(ids))
        }
        dw_core::AdoActionEvent::UpdatingWorkItemState { ids, state } => {
            format!("{action_id} workitems={} state={state}", join_display(ids))
        }
        dw_core::AdoActionEvent::UpdatedWorkItemState { id, state } => {
            format!("{action_id} workitem=#{id} state={state}")
        }
    }
}

pub fn action_event_line(event: &dw_core::DwActionEvent) -> Option<String> {
    match event {
        dw_core::DwActionEvent::Ado(event) => Some(ado_action_event_line(event)),
        dw_core::DwActionEvent::Task(event) => Some(task_action_event_line(event)),
        dw_core::DwActionEvent::Log(event) => Some(diagnostic_log_event_line(event)),
        _ => None,
    }
}

fn git_operation_key(operation: dw_core::GitOperation) -> &'static str {
    match operation {
        dw_core::GitOperation::CommitAndPush => "commit-and-push",
        dw_core::GitOperation::Push => "push",
    }
}

fn join_display<T: std::fmt::Display>(items: &[T]) -> String {
    if items.is_empty() {
        return "none".into();
    }
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

use std::io::IsTerminal;

pub fn banner(title: &str) -> String {
    format!("== {} ==", title)
}

pub fn is_stdin_interactive() -> bool {
    std::io::stdin().is_terminal()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalTheme {
    enabled: bool,
}

impl TerminalTheme {
    pub fn stdout(mode: ColorMode) -> Self {
        Self::new(
            mode,
            std::io::stdout().is_terminal(),
            std::env::var_os("NO_COLOR").is_some(),
        )
    }

    pub fn stdout_auto() -> Self {
        Self::stdout(ColorMode::Auto)
    }

    pub fn new(mode: ColorMode, is_terminal: bool, no_color: bool) -> Self {
        let enabled = match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => is_terminal && !no_color,
        };
        Self { enabled }
    }

    pub fn plain() -> Self {
        Self { enabled: false }
    }

    pub fn success(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Green.on_default().bold(), text)
    }

    pub fn warning(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Yellow.on_default().bold(), text)
    }

    pub fn error(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Red.on_default().bold(), text)
    }

    pub fn path(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Cyan.on_default(), text)
    }

    pub fn command(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Magenta.on_default(), text)
    }

    pub fn cyan(&self, text: &str) -> String {
        self.paint(anstyle::AnsiColor::Cyan.on_default(), text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(anstyle::Effects::DIMMED.into(), text)
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(anstyle::Effects::BOLD.into(), text)
    }

    pub fn style_line(&self, line: &str, is_error: bool) -> String {
        if line.is_empty() || is_json_like(line) {
            return line.into();
        }

        if is_error || line.starts_with_ignore_ascii_case("Error") {
            return self.bold(&self.error(line));
        }

        let trimmed = line.trim_start();
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            return self.bold(&self.cyan(line));
        }

        let styled = line
            .replace(": Done", &format!(": {}", self.bold(&self.success("Done"))))
            .replace("Done:", &format!("{}:", self.bold(&self.success("Done"))));

        if starts_with_any_ignore_ascii_case(
            &styled,
            &["Dry-run", "Retry", "PR not created", "Preview"],
        ) {
            return self.warning(&styled);
        }

        if let Some(styled_key_value) = self.style_known_key_value(&styled) {
            return styled_key_value;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let indent_length = line.len() - trimmed.len();
            let indent = &line[..indent_length];
            return format!("{indent}{}{}", self.dim(&trimmed[..2]), &trimmed[2..]);
        }

        if let Some(separator_index) = styled.find(':')
            && separator_index > 0
            && separator_index <= 40
        {
            let label = &styled[..separator_index];
            let suffix = &styled[separator_index..];
            if !label.contains("//") && !label.contains('\\') {
                return format!("{}{}", self.bold(&self.cyan(label)), suffix);
            }
        }

        if is_success_status_line(&styled) {
            return self.success(&styled);
        }

        if starts_with_any_ignore_ascii_case(
            &styled,
            &[
                "No ",
                "Nothing",
                "Sync skipped",
                "PR skipped",
                "ADO skipped",
                "Prune canceled",
                "Deletion canceled",
                "Finish canceled",
            ],
        ) {
            return self.warning(&styled);
        }

        if starts_with_any_ignore_ascii_case(
            &styled,
            &[
                "Next step",
                "Then, to",
                "Finally",
                "Available workspaces",
                "Project  WorkItem",
                "Update",
                "Update preparation",
                "Schemas and agent contexts regenerated",
                "DevWorkflow configuration",
                "Configuration diagnostics",
                "Configuration updated",
                "Task workspaces",
                "Task sync",
                "Workspace rename",
                "Workspace cleanup",
                "Workspace work items",
                "Current workspace",
                "Repository update",
                "Repository commits",
                "Add repository",
                "Workspace deletion",
                "Workspace finish",
                "Handoff ",
                "Commit to create",
                "Pull requests to create",
                "Task preflight",
                "Preflight details",
                "Handoff validation",
                "Handoff details",
                "Assigned ADO",
                "ADO work item",
                "ADO context",
                "ADO connection",
                "DB query",
                "SQL guard",
                "ADO child task",
                "Secret",
            ],
        ) {
            return self.bold(&self.cyan(&styled));
        }

        styled
    }

    fn style_known_key_value(&self, line: &str) -> Option<String> {
        let separator_index = line.find(':')?;
        if separator_index == 0 || separator_index > 40 {
            return None;
        }

        let label = &line[..separator_index];
        let label_name = label.trim();
        if !matches!(label_name, "Status" | "Result" | "Decision" | "Next") {
            return None;
        }

        let suffix = &line[separator_index + 1..];
        let value_start = suffix.len() - suffix.trim_start().len();
        let value_padding = &suffix[..value_start];
        let value = &suffix[value_start..];
        let styled_label = self.bold(&self.cyan(label));
        let styled_value = self.style_known_value(label_name, value);

        Some(format!("{styled_label}:{value_padding}{styled_value}"))
    }

    fn style_known_value(&self, label_name: &str, value: &str) -> String {
        match label_name {
            "Next" => self.warning(value),
            "Decision" => {
                if value.contains('✓') {
                    self.success(value)
                } else if value.contains('!') || value.contains('✕') {
                    self.error(value)
                } else {
                    self.bold(value)
                }
            }
            "Status" | "Result" => self.style_status_value(value),
            _ => value.into(),
        }
    }

    fn style_status_value(&self, value: &str) -> String {
        let normalized = value.to_lowercase();
        if contains_any(
            &normalized,
            &[
                "not connected",
                "needs fixing",
                "blocked",
                "not found",
                "incomplete",
                "error",
                "failed",
            ],
        ) {
            self.error(value)
        } else if contains_any(
            &normalized,
            &["changed", "truncated", "skipped", "to do", "pending"],
        ) {
            self.warning(value)
        } else if contains_any(
            &normalized,
            &[
                "connected",
                "finished",
                "valid",
                "authorized",
                "saved",
                "present",
                "deleted",
                "ok",
                "done",
            ],
        ) {
            self.success(value)
        } else {
            value.into()
        }
    }

    fn paint(&self, style: anstyle::Style, text: &str) -> String {
        if self.enabled {
            format!("{}{}{}", style.render(), text, style.render_reset())
        } else {
            text.into()
        }
    }
}

fn is_json_like(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[') || trimmed.starts_with('"')
}

fn is_success_status_line(line: &str) -> bool {
    starts_with_any_ignore_ascii_case(
        line,
        &[
            "Workspace created",
            "Worktree created",
            "Workspace renamed",
            "Workspace synchronized",
            "Workspace deleted",
            "Repository added",
            "Work items added",
            "Work items removed",
            "Binary replaced",
            "Commits/push finished",
            "PR created",
            "Root refreshed",
            "Workspace updated",
        ],
    ) || (line.starts_with_ignore_ascii_case("Repo ") && line.contains(':'))
}

fn starts_with_any_ignore_ascii_case(value: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| value.starts_with_ignore_ascii_case(prefix))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

trait StartsWithIgnoreAsciiCase {
    fn starts_with_ignore_ascii_case(&self, prefix: &str) -> bool;
}

impl StartsWithIgnoreAsciiCase for str {
    fn starts_with_ignore_ascii_case(&self, prefix: &str) -> bool {
        self.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, TerminalTheme};

    #[test]
    fn never_keeps_plain_text() {
        let theme = TerminalTheme::new(ColorMode::Never, true, false);
        assert_eq!(theme.success("OK"), "OK");
    }

    #[test]
    fn auto_disables_color_when_no_color_is_set() {
        let theme = TerminalTheme::new(ColorMode::Auto, true, true);
        assert_eq!(theme.warning("WARN"), "WARN");
    }

    #[test]
    fn always_emits_ansi() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        assert!(theme.error("ERR").contains("\u{1b}"));
    }

    #[test]
    fn style_line_colors_status_lines() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        let styled = theme.style_line("Workspace created: S:/dw", false);

        assert!(styled.contains("\u{1b}"));
        assert!(styled.contains("Workspace created"));
    }

    #[test]
    fn style_line_colors_known_status_values() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        let ok = theme.style_line("Status    : connected", false);
        let blocked = theme.style_line("Status    : blocked", false);

        assert!(ok.contains("\u{1b}[1m\u{1b}[32mconnected"));
        assert!(blocked.contains("\u{1b}[1m\u{1b}[31mblocked"));
    }

    #[test]
    fn style_line_colors_action_commands() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);
        let styled = theme.style_line("Action    : finish workspace", false);

        assert!(styled.contains("\u{1b}[1m\u{1b}[36mAction"));
    }

    #[test]
    fn style_line_colors_noop_and_cancelled_messages_as_warnings() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);

        assert!(
            theme
                .style_line("No task workspace found.", false)
                .contains("\u{1b}[33m")
        );
        assert!(
            theme
                .style_line("Nothing to finish.", false)
                .contains("\u{1b}[33m")
        );
        assert!(
            theme
                .style_line("Deletion canceled.", false)
                .contains("\u{1b}[33m")
        );
    }

    #[test]
    fn style_line_keeps_known_key_values_plain_without_color() {
        let theme = TerminalTheme::plain();

        assert_eq!(
            theme.style_line("Action    : finish workspace", false),
            "Action    : finish workspace"
        );
        assert_eq!(
            theme.style_line("Status    : blocked", false),
            "Status    : blocked"
        );
    }

    #[test]
    fn style_line_preserves_json_lines() {
        let theme = TerminalTheme::new(ColorMode::Always, false, false);

        assert_eq!(
            theme.style_line(r#"{"schema":1}"#, false),
            r#"{"schema":1}"#
        );
    }

    #[test]
    fn style_line_keeps_plain_when_color_disabled() {
        let theme = TerminalTheme::plain();

        assert_eq!(
            theme.style_line("Workspace created: S:/dw", false),
            "Workspace created: S:/dw"
        );
    }

    #[test]
    fn stdout_auto_is_constructible() {
        let theme = TerminalTheme::stdout_auto();

        assert_eq!(
            theme.style_line(r#"{"schema":1}"#, false),
            r#"{"schema":1}"#
        );
    }
}
