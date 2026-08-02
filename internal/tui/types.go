package tui

import (
	"context"
	"io"
	"os"
	"os/exec"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
)

// View identifies one of the six stable TUI views.
type View uint8

const (
	Dashboard View = iota
	Workspaces
	Work
	PullRequests
	Data
	Composer
)

var allViews = [...]View{Dashboard, Workspaces, Work, PullRequests, Data, Composer}

// Risk controls confirmation and visual treatment. Machine action identifiers
// remain separate from these human-facing labels.
type Risk uint8

const (
	Safe Risk = iota
	External
	Preview
	Destructive
)

// Parameter is an ordered form argument. The controller converts FormRequest
// to the concrete domain request registered with the shared action dispatcher.
type Parameter struct {
	Name  string
	Value any
}

// FormRequest is the concrete request emitted by the 17 generic TUI forms.
type FormRequest struct {
	Action     action.ID
	Parameters []Parameter
}

func (r FormRequest) ActionID() action.ID { return r.Action }

// Action is a fully projected operation. Action slices retain presentation
// order; IDs and hotkeys are machine tokens and are never localized.
type Action struct {
	ID                  action.ID
	Label               string
	Description         string
	Risk                Risk
	MenuSection         string
	Hotkey              string
	Active              bool
	Request             action.Request
	RefreshAfterSuccess bool
	BlocksUntilDone     bool
}

// StateEffect describes local state that can be applied after success.
type StateEffect struct {
	Root         *string
	DefaultAgent *string
	ColorMode    *string
	Initialized  bool
}

// ExternalProcess is a portable process launch plan.
type ExternalProcess struct {
	Program   string
	Arguments []string
	Directory string
	Env       []string
}

func (p ExternalProcess) command() *exec.Cmd {
	cmd := exec.Command(p.Program, p.Arguments...)
	cmd.Dir = p.Directory
	if p.Env != nil {
		cmd.Env = append(os.Environ(), p.Env...)
	}
	return cmd
}

type RequestBuilder func(context.Context, action.Request) (action.Request, error)

// EventProjection and ResultProjection share presentation with console while
// retaining action envelopes and concrete results as the source of truth.
type EventProjection func(action.EventEnvelope) (LogLevel, string, string)
type ResultProjection func(action.Result) []string
type ExternalProjection func(action.Result) (ExternalProcess, bool)
type StateEffectProjection func(action.Result) *StateEffect

type Dependencies struct {
	Root            string
	Executor        execution.Executor
	Actor           execution.Actor
	RequestBuilder  RequestBuilder
	Cockpit         *cockpit.Service
	ProjectEvent    EventProjection
	ProjectResult   ResultProjection
	ProjectExternal ExternalProjection
	ProjectState    StateEffectProjection
	Localizer       l10n.Localizer
	Input           io.Reader
	Output          io.Writer
}

func actionFromOperation(operation cockpit.Operation) Action {
	risk := Safe
	switch operation.Risk {
	case cockpit.RiskPreview:
		risk = Preview
	case cockpit.RiskDestructive:
		risk = Destructive
	case cockpit.RiskExternal:
		risk = External
	}
	return Action{
		ID: action.ID(operation.Relation), Label: operation.Label, Description: operation.Description,
		Risk: risk, Active: operation.Active, Request: operation.Request, RefreshAfterSuccess: true,
	}
}

func findAction(operations []cockpit.Operation, id action.ID) (Action, bool) {
	for i := range operations {
		if operations[i].Relation == cockpit.Relation(id) && operations[i].Active {
			return actionFromOperation(operations[i]), true
		}
	}
	return Action{}, false
}
