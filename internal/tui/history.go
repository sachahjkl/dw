package tui

import (
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/execution"
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

type RecordedEvent struct {
	At    time.Time
	Raw   action.EventEnvelope
	Level LogLevel
	Scope string
	Text  string
}

type RunRecord struct {
	ID       execution.ExecutionID
	Label    string
	Status   execution.Status
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

func (h *History) start(id execution.ExecutionID, label string, status execution.Status) {
	h.Runs = append(h.Runs, RunRecord{ID: id, Label: label, Status: status})
	h.Selected = len(h.Runs) - 1
	h.Scroll = 0
}
func (h *History) load(run RunRecord) {
	h.Runs = append(h.Runs, run)
	h.Selected = len(h.Runs) - 1
	h.Scroll = 0
}

func (h *History) appendEvent(id execution.ExecutionID, event RecordedEvent) {
	run := h.find(id)
	if run == nil {
		return
	}
	run.Events = append(run.Events, event)
}

func (h *History) finish(id execution.ExecutionID, status execution.Status, lines []string, errText string, external *ExternalProcess) {
	run := h.running(id)
	if run == nil {
		return
	}
	run.Status = status
	run.Lines = append([]string(nil), lines...)
	run.Error = errText
	run.External = external
}

func (h *History) find(id execution.ExecutionID) *RunRecord {
	for i := len(h.Runs) - 1; i >= 0; i-- {
		if h.Runs[i].ID == id {
			return &h.Runs[i]
		}
	}
	return nil
}

func (h *History) running(id execution.ExecutionID) *RunRecord {
	for i := len(h.Runs) - 1; i >= 0; i-- {
		if h.Runs[i].ID == id && !h.Runs[i].Status.Terminal() {
			return &h.Runs[i]
		}
	}
	return nil
}

func (h *History) active() *RunRecord {
	for i := len(h.Runs) - 1; i >= 0; i-- {
		if !h.Runs[i].Status.Terminal() {
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
