use dw_app::DwActionResult;
use dw_core::DwActionEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHistoryEntry {
    pub request_label: String,
    pub status: String,
    pub success: bool,
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
    Failed {
        error: String,
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

    pub fn failed(error: String) -> Self {
        Self::Failed { error }
    }

    pub fn events(&self) -> &[DwActionEvent] {
        match self {
            Self::Running { events } | Self::Completed { events, .. } => events,
            Self::Failed { .. } => &[],
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

    pub fn start_running(&mut self, request_label: String) {
        self.push(RunHistoryEntry {
            request_label,
            status: "running".into(),
            success: true,
            record: ActionRunRecord::running(),
        });
    }

    pub fn append_running_event(&mut self, request_label: &str, event: DwActionEvent) {
        let Some(entry) = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_label == request_label && entry.status == "running")
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
        request_label: &str,
        status: String,
        success: bool,
        record: ActionRunRecord,
    ) -> bool {
        let Some(entry) = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_label == request_label && entry.status == "running")
        else {
            return false;
        };
        entry.status = status;
        entry.success = success;
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

    pub fn scroll_output_down(&mut self) {
        self.output_scroll = (self.output_scroll + 1).min(self.output_max_scroll());
    }

    pub fn scroll_output_home(&mut self) {
        self.output_scroll = 0;
    }

    pub fn scroll_output_end(&mut self) {
        self.output_scroll = self.output_max_scroll();
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

    pub fn output_max_scroll(&self) -> usize {
        self.selected_entry()
            .map(|entry| entry.record.events().len().saturating_sub(1))
            .unwrap_or_default()
    }
}

#[cfg(test)]
pub fn preview_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .cloned()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn capped_lines(mut lines: Vec<String>) -> Vec<String> {
    cap_rendered_lines(&mut lines);
    lines
}

fn cap_rendered_lines(lines: &mut Vec<String>) {
    const MAX_OUTPUT_LINES: usize = 160;
    if lines.len() <= MAX_OUTPUT_LINES {
        return;
    }
    let omitted = lines.len() - MAX_OUTPUT_LINES;
    let mut kept = Vec::with_capacity(MAX_OUTPUT_LINES + 1);
    kept.push(format!("... {omitted} previous lines hidden ..."));
    kept.extend(lines.drain(omitted..));
    *lines = kept;
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
    fn preview_lines_keeps_last_non_empty_lines() {
        let preview = preview_lines(&[
            "one".into(),
            String::new(),
            "two".into(),
            "three".into(),
            "four".into(),
        ]);

        assert_eq!(preview, ["two", "three", "four"]);
    }

    #[test]
    fn capped_lines_caps_long_action_output() {
        let output = (0..170)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>();

        let lines = capped_lines(output);

        assert_eq!(lines.len(), 161);
        assert!(lines[0].contains("previous lines hidden"));
        assert_eq!(lines.last().map(String::as_str), Some("line 169"));
    }

    #[test]
    fn history_state_caps_entries_and_controls_output_modal() {
        let mut history = HistoryState::default();
        assert!(history.open_output());
        assert!(history.output_open);
        assert_eq!(history.selected_entry, 0);
        history.close_output();

        for index in 0..22 {
            history.push(RunHistoryEntry {
                request_label: format!("Doctor {index}"),
                status: "exit 0".into(),
                success: true,
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

        history.scroll_output_down();
        assert_eq!(history.output_scroll, 1);

        history.scroll_output_end();
        assert_eq!(history.output_scroll, 2);

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
    fn running_history_entry_streams_and_finishes() {
        let mut history = HistoryState::default();

        history.start_running("Task finish".into());
        history.append_running_event(
            "Task finish",
            DwActionEvent::Started {
                action_id: "prepare".into(),
            },
        );
        history.append_running_event(
            "Task finish",
            DwActionEvent::Started {
                action_id: "push-front".into(),
            },
        );

        let entry = history.selected_entry().expect("entry");
        assert_eq!(entry.status, "running");
        assert_eq!(entry.record.events().len(), 2);

        assert!(history.finish_running(
            "Task finish",
            "exit 0".into(),
            true,
            ActionRunRecord::Completed {
                events: entry.record.events().to_vec(),
                result: Box::new(DwActionResult::App(dw_app::AppActionResult::Version {
                    version: "2026.07.06.3".into(),
                })),
            }
        ));
        let entry = history.selected_entry().expect("entry");
        assert_eq!(entry.status, "exit 0");
        assert!(entry.success);
        assert!(matches!(entry.record, ActionRunRecord::Completed { .. }));
    }
}
