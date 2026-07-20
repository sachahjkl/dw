package tui

import (
	"time"

	"github.com/sachahjkl/dw/internal/action"
)

const (
	maxHistoryRuns   = 20
	maxHistoryEvents = 160
)

type LogLevel uint8

const (
	ErrorLevel LogLevel = iota
	WarningLevel
	InfoLevel
	DebugLevel
	OtherLevel
)

var allLogLevels = [...]LogLevel{ErrorLevel, WarningLevel, InfoLevel, DebugLevel, OtherLevel}

type RunStatus uint8

const (
	RunRunning RunStatus = iota
	RunSucceeded
	RunFailed
)

type RecordedEvent struct {
	At    time.Time
	Raw   action.EventEnvelope
	Level LogLevel
	Scope string
	Text  string
}

type RunRecord struct {
	ID       uint64
	Label    string
	Status   RunStatus
	Events   []RecordedEvent
	Lines    []string
	Error    string
	External *ExternalProcess
}

type History struct {
	Runs       []RunRecord
	Selected   int
	Scroll     int
	Fullscreen bool
	Levels     [5]bool
}

func newHistory() History {
	return History{Levels: [5]bool{true, true, true, true, true}}
}

func (h *History) start(id uint64, label string) {
	h.Runs = append(h.Runs, RunRecord{ID: id, Label: label, Status: RunRunning})
	if len(h.Runs) > maxHistoryRuns {
		copy(h.Runs, h.Runs[len(h.Runs)-maxHistoryRuns:])
		h.Runs = h.Runs[:maxHistoryRuns]
	}
	h.Selected = len(h.Runs) - 1
	h.Scroll = 0
}

func (h *History) appendEvent(id uint64, event RecordedEvent) {
	run := h.running(id)
	if run == nil {
		return
	}
	run.Events = append(run.Events, event)
	if len(run.Events) > maxHistoryEvents {
		copy(run.Events, run.Events[len(run.Events)-maxHistoryEvents:])
		run.Events = run.Events[:maxHistoryEvents]
	}
}

func (h *History) finish(id uint64, status RunStatus, lines []string, errText string, external *ExternalProcess) {
	run := h.running(id)
	if run == nil {
		return
	}
	run.Status = status
	run.Lines = append([]string(nil), lines...)
	if len(run.Lines) > maxHistoryEvents {
		run.Lines = append([]string(nil), run.Lines[len(run.Lines)-maxHistoryEvents:]...)
	}
	run.Error = errText
	run.External = external
}

func (h *History) running(id uint64) *RunRecord {
	for i := len(h.Runs) - 1; i >= 0; i-- {
		if h.Runs[i].ID == id && h.Runs[i].Status == RunRunning {
			return &h.Runs[i]
		}
	}
	return nil
}

func (h *History) active() *RunRecord {
	for i := len(h.Runs) - 1; i >= 0; i-- {
		if h.Runs[i].Status == RunRunning {
			return &h.Runs[i]
		}
	}
	return nil
}

func (h History) selected() *RunRecord {
	if h.Selected < 0 || h.Selected >= len(h.Runs) {
		return nil
	}
	return &h.Runs[h.Selected]
}

func (h *History) selectRun(delta int) {
	if len(h.Runs) == 0 {
		return
	}
	h.Selected += delta
	if h.Selected < 0 {
		h.Selected = 0
	}
	if h.Selected >= len(h.Runs) {
		h.Selected = len(h.Runs) - 1
	}
	h.Scroll = 0
}

func (h *History) toggleLevel(level LogLevel) {
	index := int(level)
	if index < 0 || index >= len(h.Levels) {
		return
	}
	h.Levels[index] = !h.Levels[index]
	any := false
	for _, enabled := range h.Levels {
		any = any || enabled
	}
	if !any {
		h.Levels[index] = true
	}
	h.Scroll = 0
}

func (h *History) enableAll() {
	for i := range h.Levels {
		h.Levels[i] = true
	}
	h.Scroll = 0
}

func (h History) visibleEvents(run *RunRecord) []RecordedEvent {
	if run == nil {
		return nil
	}
	result := make([]RecordedEvent, 0, len(run.Events))
	for _, event := range run.Events {
		if int(event.Level) < len(h.Levels) && h.Levels[event.Level] {
			result = append(result, event)
		}
	}
	return result
}
