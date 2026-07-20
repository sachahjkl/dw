// Package action defines the typed execution boundary shared by CLI and TUI.
// Requests and results remain concrete domain values behind non-generic
// interfaces so controllers can dispatch without reflection or type erasure.
package action

import (
	"fmt"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

type ID = contract.ActionID

// Request is a typed action request.
type Request interface{ ActionID() ID }

// Result is a typed action result.
type Result interface{ ActionID() ID }

// ResultEnvelope carries a typed result with its stable discriminator for UI
// history and controller boundaries.
type ResultEnvelope struct {
	Action ID
	Result Result
}

// Domain and Risk are stable machine tokens used for policy decisions.
type Domain string
type Risk string

const (
	DomainConfig  Domain = "config"
	DomainAgent   Domain = "agent"
	DomainAuth    Domain = "auth"
	DomainWork    Domain = "work"
	DomainData    Domain = "data"
	DomainTask    Domain = "task"
	DomainSecret  Domain = "secret"
	DomainUpgrade Domain = "upgrade"

	RiskReadOnly       Risk = "read-only"
	RiskPreview        Risk = "preview"
	RiskMutating       Risk = "mutating"
	RiskDestructive    Risk = "destructive"
	RiskExternalLaunch Risk = "external-launch"
)

// Descriptor contains action policy and localized presentation references.
type Descriptor struct {
	ID                  ID
	Domain              Domain
	Risk                Risk
	Label               l10n.ID
	Description         l10n.ID
	RefreshAfterSuccess bool
}

// PromptID and ChoiceValue are distinct to prevent labels entering responses.
type PromptID string
type ChoiceValue string
type PromptKind string

const (
	PromptText       PromptKind = "text"
	PromptSecret     PromptKind = "secret"
	PromptSelectOne  PromptKind = "select-one"
	PromptSelectMany PromptKind = "select-many"
	PromptConfirm    PromptKind = "confirm"
)

type Choice struct {
	Value       ChoiceValue
	Label       l10n.Message
	Description *l10n.Message
}

// Prompt is a stable dialogue contract. Default and Choices are interpreted
// according to Kind; secret defaults are prohibited.
type Prompt struct {
	ID       PromptID
	Kind     PromptKind
	Label    l10n.Message
	Help     *l10n.Message
	Required bool
	Choices  []Choice
	Default  *ChoiceValue
}

// Validate checks structural invariants without generating presentation text.
func (p Prompt) Validate() error {
	if p.ID == "" || p.Label.ID == "" {
		return fmt.Errorf("action.invalid-prompt")
	}
	switch p.Kind {
	case PromptText, PromptConfirm:
		if len(p.Choices) != 0 {
			return fmt.Errorf("action.prompt-unexpected-choices")
		}
	case PromptSecret:
		if len(p.Choices) != 0 || p.Default != nil {
			return fmt.Errorf("action.secret-prompt-default")
		}
	case PromptSelectOne, PromptSelectMany:
		if len(p.Choices) == 0 {
			return fmt.Errorf("action.prompt-missing-choices")
		}
		seen := make(map[ChoiceValue]struct{}, len(p.Choices))
		for _, choice := range p.Choices {
			if choice.Value == "" || choice.Label.ID == "" {
				return fmt.Errorf("action.invalid-choice")
			}
			if _, exists := seen[choice.Value]; exists {
				return fmt.Errorf("action.duplicate-choice:%s", choice.Value)
			}
			seen[choice.Value] = struct{}{}
		}
	default:
		return fmt.Errorf("action.unknown-prompt-kind:%s", p.Kind)
	}
	return nil
}

// Response carries exactly one response shape selected by Kind.
type Response struct {
	Kind     PromptKind
	Accepted bool
	Value    ChoiceValue
	Values   []ChoiceValue
	Text     string
	Secret   contract.SecretValue
}

// EventKind is a stable machine discriminator; Message is localized by the
// presentation layer and Data is an optional typed machine DTO.
type EventKind string

const (
	EventStarted   EventKind = "started"
	EventProgress  EventKind = "progress"
	EventInput     EventKind = "input-required"
	EventCompleted EventKind = "completed"
	EventWarning   EventKind = "warning"
	EventLog       EventKind = "log"
)

// EventEnvelope is the event DTO consumed identically by CLI and TUI.
type EventEnvelope struct {
	Action   ID
	Kind     EventKind
	Sequence uint64
	Message  l10n.Message
	Data     any
}

type Event = EventEnvelope
