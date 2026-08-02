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
	Value       ChoiceValue   `json:"value"`
	Label       l10n.Message  `json:"label"`
	Description *l10n.Message `json:"description,omitempty"`
}

type PromptMeta struct {
	ID    PromptID      `json:"id"`
	Label l10n.Message  `json:"label"`
	Help  *l10n.Message `json:"help,omitempty"`
}

func (m PromptMeta) validate() error {
	if m.ID == "" || m.Label.ID == "" {
		return fmt.Errorf("action.invalid-prompt")
	}
	return nil
}

// Prompt is the closed sum of dialogue contracts accepted by Runtime.
type Prompt interface {
	PromptID() PromptID
	PromptKind() PromptKind
	Validate() error
	isPrompt()
}

type TextPrompt struct {
	Meta     PromptMeta `json:"meta"`
	Required bool       `json:"required"`
}

func (p TextPrompt) PromptID() PromptID   { return p.Meta.ID }
func (TextPrompt) PromptKind() PromptKind { return PromptText }
func (p TextPrompt) Validate() error      { return p.Meta.validate() }
func (TextPrompt) isPrompt()              {}

type SecretPrompt struct {
	Meta     PromptMeta `json:"meta"`
	Required bool       `json:"required"`
}

func (p SecretPrompt) PromptID() PromptID   { return p.Meta.ID }
func (SecretPrompt) PromptKind() PromptKind { return PromptSecret }
func (p SecretPrompt) Validate() error      { return p.Meta.validate() }
func (SecretPrompt) isPrompt()              {}

type ConfirmPrompt struct {
	Meta    PromptMeta `json:"meta"`
	Default bool       `json:"default"`
}

func (p ConfirmPrompt) PromptID() PromptID   { return p.Meta.ID }
func (ConfirmPrompt) PromptKind() PromptKind { return PromptConfirm }
func (p ConfirmPrompt) Validate() error      { return p.Meta.validate() }
func (ConfirmPrompt) isPrompt()              {}

type SelectOnePrompt struct {
	Meta     PromptMeta   `json:"meta"`
	Required bool         `json:"required"`
	Choices  []Choice     `json:"choices"`
	Default  *ChoiceValue `json:"default,omitempty"`
}

func (p SelectOnePrompt) PromptID() PromptID   { return p.Meta.ID }
func (SelectOnePrompt) PromptKind() PromptKind { return PromptSelectOne }
func (p SelectOnePrompt) Validate() error {
	allowed, err := validateChoices(p.Meta, p.Choices)
	if err != nil {
		return err
	}
	if p.Default != nil {
		if _, ok := allowed[*p.Default]; !ok {
			return fmt.Errorf("action.input-invalid-choice:%s", *p.Default)
		}
	}
	return nil
}
func (SelectOnePrompt) isPrompt() {}

type SelectManyPrompt struct {
	Meta     PromptMeta    `json:"meta"`
	Required bool          `json:"required"`
	Choices  []Choice      `json:"choices"`
	Defaults []ChoiceValue `json:"defaults,omitempty"`
}

func (p SelectManyPrompt) PromptID() PromptID   { return p.Meta.ID }
func (SelectManyPrompt) PromptKind() PromptKind { return PromptSelectMany }
func (p SelectManyPrompt) Validate() error {
	allowed, err := validateChoices(p.Meta, p.Choices)
	if err != nil {
		return err
	}
	seen := make(map[ChoiceValue]struct{}, len(p.Defaults))
	for _, value := range p.Defaults {
		if _, duplicate := seen[value]; duplicate {
			return fmt.Errorf("action.input-duplicate-choice:%s", value)
		}
		if _, ok := allowed[value]; !ok {
			return fmt.Errorf("action.input-invalid-choice:%s", value)
		}
		seen[value] = struct{}{}
	}
	return nil
}
func (SelectManyPrompt) isPrompt() {}

func validateChoices(meta PromptMeta, choices []Choice) (map[ChoiceValue]struct{}, error) {
	if err := meta.validate(); err != nil {
		return nil, err
	}
	if len(choices) == 0 {
		return nil, fmt.Errorf("action.prompt-missing-choices")
	}
	allowed := make(map[ChoiceValue]struct{}, len(choices))
	for _, choice := range choices {
		if choice.Value == "" || choice.Label.ID == "" {
			return nil, fmt.Errorf("action.invalid-choice")
		}
		if _, duplicate := allowed[choice.Value]; duplicate {
			return nil, fmt.Errorf("action.input-duplicate-choice:%s", choice.Value)
		}
		allowed[choice.Value] = struct{}{}
	}
	return allowed, nil
}

// Response is the closed sum of prompt responses accepted by Runtime.
type Response interface {
	PromptKind() PromptKind
	isResponse()
}

type TextResponse struct {
	Value string `json:"value"`
}

func (TextResponse) PromptKind() PromptKind { return PromptText }
func (TextResponse) isResponse()            {}

type SecretResponse struct {
	Value contract.SecretValue `json:"-"`
}

func (SecretResponse) PromptKind() PromptKind { return PromptSecret }
func (SecretResponse) isResponse()            {}

type ConfirmResponse struct {
	Accepted bool `json:"accepted"`
}

func (ConfirmResponse) PromptKind() PromptKind { return PromptConfirm }
func (ConfirmResponse) isResponse()            {}

type SelectOneResponse struct {
	Value ChoiceValue `json:"value"`
}

func (SelectOneResponse) PromptKind() PromptKind { return PromptSelectOne }
func (SelectOneResponse) isResponse()            {}

type SelectManyResponse struct {
	Values []ChoiceValue `json:"values"`
}

func (SelectManyResponse) PromptKind() PromptKind { return PromptSelectMany }
func (SelectManyResponse) isResponse()            {}

// EventKind is a stable machine discriminator for handler-owned events.
type EventKind string

const (
	EventProgress EventKind = "progress"
	EventWarning  EventKind = "warning"
	EventLog      EventKind = "log"
)

type EventDataType string

// EventData is a registered, versioned machine payload.
type EventData interface {
	EventDataType() EventDataType
	EventDataSchema() uint16
}

// EventEnvelope is the handler event consumed by execution adapters.
type EventEnvelope struct {
	Action  ID
	Kind    EventKind
	Message l10n.Message
	Data    EventData
}

type Event = EventEnvelope
