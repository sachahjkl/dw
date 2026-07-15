use dw_app::DwActionResult;
use dw_core::{DwActionEvent, ExternalLaunchPlan};
use std::collections::BTreeSet;
use std::fmt;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionRunId(u64);

impl ActionRunId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRunLabel(String);

impl ActionRunLabel {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for ActionRunLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRunErrorMessage(String);

impl ActionRunErrorMessage {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for ActionRunErrorMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRunStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JournalLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Other,
}

impl JournalLogLevel {
    pub const ALL: [Self; 5] = [
        Self::Error,
        Self::Warn,
        Self::Info,
        Self::Debug,
        Self::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Other => "other",
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            Self::Error => "ERR",
            Self::Warn => "WRN",
            Self::Info => "INF",
            Self::Debug => "DBG",
            Self::Other => "---",
        }
    }
}

impl ActionRunStatus {
    pub fn is_running(self) -> bool {
        self == Self::Running
    }
}

impl fmt::Display for ActionRunStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => formatter.write_str("running"),
            Self::Succeeded => formatter.write_str("ok"),
            Self::Failed => formatter.write_str("error"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHistoryEntry {
    pub id: ActionRunId,
    pub request_label: ActionRunLabel,
    pub status: ActionRunStatus,
    pub record: ActionRunRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalTimestamp(String);

impl JournalTimestamp {
    pub fn now_utc() -> Self {
        let now = OffsetDateTime::now_utc();
        Self(format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        ))
    }

    #[cfg(test)]
    pub fn fixed(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedActionEvent {
    pub occurred_at: JournalTimestamp,
    pub event: DwActionEvent,
}

impl RecordedActionEvent {
    pub fn now(event: DwActionEvent) -> Self {
        Self {
            occurred_at: JournalTimestamp::now_utc(),
            event,
        }
    }

    #[cfg(test)]
    pub fn fixed(occurred_at: impl Into<String>, event: DwActionEvent) -> Self {
        Self {
            occurred_at: JournalTimestamp::fixed(occurred_at),
            event,
        }
    }
}

impl RunHistoryEntry {
    pub fn latest_event(&self) -> Option<&DwActionEvent> {
        self.record.latest_event()
    }
}

#[derive(Debug, Clone)]
pub enum ActionRunRecord {
    Running {
        events: Vec<RecordedActionEvent>,
    },
    Completed {
        events: Vec<RecordedActionEvent>,
        result: Box<DwActionResult>,
    },
    ExternalLaunch {
        plan: Box<ExternalLaunchPlan>,
    },
    Failed {
        events: Vec<RecordedActionEvent>,
        error: ActionRunErrorMessage,
    },
}

impl PartialEq for ActionRunRecord {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl Eq for ActionRunRecord {}

impl ActionRunRecord {
    pub fn running() -> Self {
        Self::Running { events: Vec::new() }
    }

    pub fn failed(events: Vec<DwActionEvent>, error: ActionRunErrorMessage) -> Self {
        Self::Failed {
            events: record_events_now(events),
            error,
        }
    }

    pub fn latest_event(&self) -> Option<&DwActionEvent> {
        match self {
            Self::Running { events }
            | Self::Completed { events, .. }
            | Self::Failed { events, .. } => events.last().map(|event| &event.event),
            Self::ExternalLaunch { .. } => None,
        }
    }

    #[cfg(test)]
    pub fn events(&self) -> &[RecordedActionEvent] {
        match self {
            Self::Running { events }
            | Self::Completed { events, .. }
            | Self::Failed { events, .. } => events,
            Self::ExternalLaunch { .. } => &[],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HistoryState {
    pub entries: Vec<RunHistoryEntry>,
    pub output_open: bool,
    pub output_scroll: usize,
    pub selected_entry: usize,
    pub output_fullscreen: bool,
    output_log_levels: BTreeSet<JournalLogLevel>,
}

impl HistoryState {
    fn ensure_default_log_levels(&mut self) {
        if self.output_log_levels.is_empty() {
            self.output_log_levels = JournalLogLevel::ALL.into_iter().collect();
        }
    }

    pub fn push(&mut self, entry: RunHistoryEntry) {
        self.entries.push(entry);
        if self.entries.len() > 20 {
            self.entries.remove(0);
        }
        self.selected_entry = self.entries.len().saturating_sub(1);
    }

    pub fn start_running(&mut self, id: ActionRunId, request_label: ActionRunLabel) {
        self.push(RunHistoryEntry {
            id,
            request_label,
            status: ActionRunStatus::Running,
            record: ActionRunRecord::running(),
        });
    }

    pub fn append_running_event(&mut self, id: ActionRunId, event: DwActionEvent) {
        let Some(entry) = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.id == id && entry.status.is_running())
        else {
            return;
        };
        if let ActionRunRecord::Running { events } = &mut entry.record {
            events.push(RecordedActionEvent::now(event));
            cap_events(events);
        }
    }

    pub fn record_events_for(
        &self,
        id: ActionRunId,
        events: Vec<DwActionEvent>,
    ) -> Vec<RecordedActionEvent> {
        let Some(entry) = self
            .entries
            .iter()
            .rev()
            .find(|entry| entry.id == id && entry.status.is_running())
        else {
            return record_events_now(events);
        };
        let ActionRunRecord::Running { events: recorded } = &entry.record else {
            return record_events_now(events);
        };
        if recorded.len() == events.len()
            && recorded
                .iter()
                .zip(events.iter())
                .all(|(recorded, event)| recorded.event == *event)
        {
            return recorded.clone();
        }
        record_events_now(events)
    }

    pub fn finish_running(
        &mut self,
        id: ActionRunId,
        status: ActionRunStatus,
        record: ActionRunRecord,
    ) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.id == id && entry.status.is_running())
        else {
            return false;
        };
        entry.status = status;
        entry.record = record;
        true
    }

    pub fn open_output(&mut self) -> bool {
        self.ensure_default_log_levels();
        self.output_open = true;
        self.output_scroll = 0;
        self.selected_entry = self.entries.len().saturating_sub(1);
        true
    }

    pub fn close_output(&mut self) {
        self.output_open = false;
        self.output_scroll = 0;
    }

    pub fn toggle_output_fullscreen(&mut self) {
        self.output_fullscreen = !self.output_fullscreen;
        self.output_scroll = 0;
    }

    pub fn toggle_log_level(&mut self, level: JournalLogLevel) {
        self.ensure_default_log_levels();
        if !self.output_log_levels.remove(&level) {
            self.output_log_levels.insert(level);
        }
        if self.output_log_levels.is_empty() {
            self.output_log_levels.insert(level);
        }
        self.output_scroll = 0;
    }

    pub fn enable_all_log_levels(&mut self) {
        self.output_log_levels = JournalLogLevel::ALL.into_iter().collect();
        self.output_scroll = 0;
    }

    pub fn log_level_enabled(&self, level: JournalLogLevel) -> bool {
        self.output_log_levels.is_empty() || self.output_log_levels.contains(&level)
    }

    pub fn log_level_labels(&self) -> Vec<String> {
        JournalLogLevel::ALL
            .into_iter()
            .map(|level| {
                let marker = if self.log_level_enabled(level) {
                    "x"
                } else {
                    " "
                };
                format!("[{marker}] {}", level.label())
            })
            .collect()
    }

    pub fn scroll_output_up(&mut self) {
        self.output_scroll = self.output_scroll.saturating_sub(1);
    }

    pub fn scroll_output_home(&mut self) {
        self.output_scroll = 0;
    }

    pub fn select_previous_entry(&mut self) {
        self.selected_entry = self.selected_entry.saturating_sub(1);
        self.output_scroll = 0;
    }

    pub fn select_next_entry(&mut self) {
        if !self.entries.is_empty() {
            self.selected_entry = (self.selected_entry + 1).min(self.entries.len() - 1);
        }
        self.output_scroll = 0;
    }

    pub fn selected_entry(&self) -> Option<&RunHistoryEntry> {
        self.entries.get(self.selected_entry)
    }

    pub fn current_running_entry(&self) -> Option<&RunHistoryEntry> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.status.is_running())
    }
}

fn record_events_now(events: Vec<DwActionEvent>) -> Vec<RecordedActionEvent> {
    events.into_iter().map(RecordedActionEvent::now).collect()
}

fn cap_events(events: &mut Vec<RecordedActionEvent>) {
    const MAX_EVENTS: usize = 160;
    if events.len() <= MAX_EVENTS {
        return;
    }
    let omitted = events.len() - MAX_EVENTS;
    events.drain(0..omitted);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_state_caps_entries_and_controls_output_modal() {
        let mut history = HistoryState::default();
        assert!(history.open_output());
        assert!(history.output_open);
        assert_eq!(history.selected_entry, 0);
        history.close_output();

        for index in 0..22 {
            history.push(RunHistoryEntry {
                id: ActionRunId::new(index),
                request_label: ActionRunLabel::new(format!("Doctor {index}")),
                status: ActionRunStatus::Succeeded,
                record: ActionRunRecord::Running {
                    events: vec![
                        RecordedActionEvent::fixed(
                            "2026-07-08 10:00:00Z",
                            DwActionEvent::Started {
                                action_id: "one".into(),
                            },
                        ),
                        RecordedActionEvent::fixed(
                            "2026-07-08 10:00:01Z",
                            DwActionEvent::Started {
                                action_id: "two".into(),
                            },
                        ),
                        RecordedActionEvent::fixed(
                            "2026-07-08 10:00:02Z",
                            DwActionEvent::Started {
                                action_id: "three".into(),
                            },
                        ),
                    ],
                },
            });
        }

        assert_eq!(history.entries.len(), 20);
        assert_eq!(history.selected_entry, 19);
        assert!(history.open_output());

        history.select_previous_entry();
        assert_eq!(history.selected_entry, 18);
        assert_eq!(history.output_scroll, 0);

        history.select_next_entry();
        assert_eq!(history.selected_entry, 19);

        history.close_output();
        assert!(!history.output_open);
        assert_eq!(history.output_scroll, 0);
    }

    #[test]
    fn running_history_entry_streams_and_finishes_by_run_id() {
        let mut history = HistoryState::default();
        let id = ActionRunId::new(1);

        history.start_running(id, ActionRunLabel::new("Task finish"));
        history.append_running_event(
            id,
            DwActionEvent::Started {
                action_id: "prepare".into(),
            },
        );
        history.append_running_event(
            id,
            DwActionEvent::Started {
                action_id: "push-front".into(),
            },
        );

        let entry = history.selected_entry().expect("entry");
        assert_eq!(entry.status, ActionRunStatus::Running);
        assert_eq!(entry.record.events().len(), 2);

        assert!(history.finish_running(
            id,
            ActionRunStatus::Succeeded,
            ActionRunRecord::Completed {
                events: entry.record.events().to_vec(),
                result: Box::new(DwActionResult::App(dw_app::AppActionResult::Version {
                    version: "2026.07.06.3".into(),
                })),
            }
        ));
        let entry = history.selected_entry().expect("entry");
        assert_eq!(entry.status, ActionRunStatus::Succeeded);
        assert!(matches!(entry.record, ActionRunRecord::Completed { .. }));
    }
}
