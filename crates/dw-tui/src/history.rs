use dw_app::DwActionResult;
use dw_core::{DwActionEvent, ExternalLaunchPlan};
use std::fmt;

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

#[derive(Debug, Clone)]
pub enum ActionRunRecord {
    Running {
        events: Vec<DwActionEvent>,
    },
    Completed {
        events: Vec<DwActionEvent>,
        result: Box<DwActionResult>,
    },
    ExternalLaunch {
        plan: Box<ExternalLaunchPlan>,
    },
    Failed {
        events: Vec<DwActionEvent>,
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
        Self::Failed { events, error }
    }

    pub fn events(&self) -> &[DwActionEvent] {
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
}

impl HistoryState {
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
            events.push(event);
            cap_events(events);
        }
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
        self.output_open = true;
        self.output_scroll = 0;
        self.selected_entry = self.entries.len().saturating_sub(1);
        true
    }

    pub fn close_output(&mut self) {
        self.output_open = false;
        self.output_scroll = 0;
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

    pub fn running_events(&self, id: ActionRunId) -> Vec<DwActionEvent> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.id == id && entry.status.is_running())
            .map(|entry| entry.record.events().to_vec())
            .unwrap_or_default()
    }
}

fn cap_events(events: &mut Vec<DwActionEvent>) {
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
                        DwActionEvent::Started {
                            action_id: "one".into(),
                        },
                        DwActionEvent::Started {
                            action_id: "two".into(),
                        },
                        DwActionEvent::Started {
                            action_id: "three".into(),
                        },
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
