#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunHistoryEntry {
    pub request_label: String,
    pub status: String,
    pub success: bool,
    pub output_preview: Vec<String>,
    pub output_lines: Vec<String>,
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
            output_preview: vec!["Action running...".into()],
            output_lines: Vec::new(),
        });
    }

    pub fn append_running_line(&mut self, request_label: &str, line: String) {
        let Some(entry) = self
            .entries
            .iter_mut()
            .rev()
            .find(|entry| entry.request_label == request_label && entry.status == "running")
        else {
            return;
        };
        entry.output_lines.push(line);
        cap_output_lines(&mut entry.output_lines);
        entry.output_preview = last_preview_lines(&entry.output_lines);
    }

    pub fn finish_running(
        &mut self,
        request_label: &str,
        status: String,
        success: bool,
        output: &str,
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
        entry.output_preview = output_preview(output);
        entry.output_lines = output_lines(output);
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
            .map(|entry| entry.output_lines.len().saturating_sub(1))
            .unwrap_or_default()
    }
}

pub fn output_preview(output: &str) -> Vec<String> {
    output_lines(output)
        .into_iter()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn output_lines(output: &str) -> Vec<String> {
    let mut lines = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    cap_output_lines(&mut lines);
    lines
}

fn cap_output_lines(lines: &mut Vec<String>) {
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

fn last_preview_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_preview_keeps_last_non_empty_lines() {
        let preview = output_preview("one\n\ntwo\nthree\nfour\n");

        assert_eq!(preview, ["two", "three", "four"]);
    }

    #[test]
    fn output_lines_caps_long_action_output() {
        let output = (0..170)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        let lines = output_lines(&output);

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
                output_preview: Vec::new(),
                output_lines: vec!["one".into(), "two".into(), "three".into()],
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
        history.append_running_line("Task finish", "Preparing...".into());
        history.append_running_line("Task finish", "Push front...".into());

        let entry = history.selected_entry().expect("entry");
        assert_eq!(entry.status, "running");
        assert_eq!(entry.output_preview, ["Preparing...", "Push front..."]);

        assert!(history.finish_running(
            "Task finish",
            "exit 0".into(),
            true,
            "Preparing...\nPush front...\nOK"
        ));
        let entry = history.selected_entry().expect("entry");
        assert_eq!(entry.status, "exit 0");
        assert!(entry.success);
        assert_eq!(
            entry.output_preview,
            ["Preparing...", "Push front...", "OK"]
        );
    }
}
