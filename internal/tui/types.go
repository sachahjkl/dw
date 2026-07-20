package tui

import (
	"context"
	"io"
	"os"
	"os/exec"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

// View identifies one of the six stable TUI views.
type View uint8

const (
	Dashboard View = iota
	Workspaces
	ADO
	PullRequests
	Databases
	Composer
)

var allViews = [...]View{Dashboard, Workspaces, ADO, PullRequests, Databases, Composer}

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
	OpenResult          bool
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

// Runner executes the real shared action graph. The TUI supplies the shared
// Runtime to receive ordered events and input prompts without type erasure.
type Runner interface {
	Run(context.Context, action.Request, action.Runtime) (action.Result, error)
}

// EventProjection and ResultProjection share presentation with console while
// retaining action envelopes and concrete results as the source of truth.
type EventProjection func(action.EventEnvelope) (LogLevel, string, string)
type ResultProjection func(action.Result) []string
type ExternalProjection func(action.Result) (ExternalProcess, bool)
type StateEffectProjection func(action.Result) *StateEffect

// SnapshotLoader functions are independent and generation-safe in Model.
type SnapshotLoader func(context.Context, string) (Snapshot, error)
type AssignedLoader func(context.Context, Snapshot) ([]ADOProject, error)
type PullRequestLoader func(context.Context, Snapshot) ([]PullRequest, error)

// Dependencies are all side effects required by the TUI.
type Dependencies struct {
	Root            string
	Runner          Runner
	Snapshot        SnapshotLoader
	Assigned        AssignedLoader
	PullRequests    PullRequestLoader
	ProjectEvent    EventProjection
	ProjectResult   ResultProjection
	ProjectExternal ExternalProjection
	ProjectState    StateEffectProjection
	Localizer       l10n.Localizer
	Input           io.Reader
	Output          io.Writer
}

// Snapshot is the presentation projection shared with the application layer.
type Snapshot struct {
	Root            string
	NeedsInit       bool
	ProjectCount    int
	RepositoryCount int
	PruneCandidates int
	DefaultAgent    string
	ColorMode       string
	DoctorOK        bool
	Projects        []string
	Repositories    []string
	States          []string
	SecretKeys      []string
	Environment     []string
	Workspaces      []Workspace
	ADOProjects     []ADOProject
	PullRequests    []PullRequest
	Databases       []Database
	Cockpit         []CockpitItem
	Actions         []Action
	InitAction      *Action
}

type Workspace struct {
	Path         string
	Project      string
	WorkItems    []string
	Type         string
	Slug         string
	Branch       string
	Repositories []string
	Actions      []Action
}

type ADOProject struct {
	Key   string
	Label string
	Error string
	Items []ADOItem
}

type ADOItem struct {
	ID      string
	Type    string
	State   string
	Title   string
	URL     string
	Actions []Action
}

type PullRequest struct {
	ID           string
	Project      string
	Repository   string
	Branch       string
	TargetBranch string
	Title        string
	Draft        bool
	Workspace    string
	WorkItems    []string
	URL          string
	Error        string
	Actions      []Action
}

type Database struct {
	Project string
	Key     string
	Actions []Action
}

type CockpitItem struct {
	Section  string
	Title    string
	Subtitle string
	Status   string
	Severity Risk
	Primary  Action
}

func findAction(actions []Action, id action.ID) (Action, bool) {
	for i := range actions {
		if actions[i].ID == id && actions[i].Active {
			return actions[i], true
		}
	}
	return Action{}, false
}
