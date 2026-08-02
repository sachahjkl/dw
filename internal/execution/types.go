package execution

import (
	"encoding/json"
	"fmt"
	"time"

	"github.com/sachahjkl/dw/internal/action"
)

type EventSequence uint64
type PrincipalID string
type Origin string
type SchemaVersion uint16
type ErrorCode string
type Status string

const (
	StatusQueued       Status = "queued"
	StatusRunning      Status = "running"
	StatusWaitingInput Status = "waiting-input"
	StatusCanceling    Status = "canceling"
	StatusCanceled     Status = "canceled"
	StatusSucceeded    Status = "succeeded"
	StatusFailed       Status = "failed"
	StatusInterrupted  Status = "interrupted"
)

const (
	OriginCLI Origin = "cli"
	OriginTUI Origin = "tui"
	OriginWeb Origin = "web"
)

func (status Status) Valid() bool {
	switch status {
	case StatusQueued, StatusRunning, StatusWaitingInput, StatusCanceling, StatusCanceled, StatusSucceeded, StatusFailed, StatusInterrupted:
		return true
	default:
		return false
	}
}

func (status Status) Terminal() bool {
	switch status {
	case StatusCanceled, StatusSucceeded, StatusFailed, StatusInterrupted:
		return true
	default:
		return false
	}
}

func ValidateTransition(from, to Status) error {
	valid := false
	switch from {
	case StatusQueued:
		valid = to == StatusRunning || to == StatusCanceled || to == StatusInterrupted
	case StatusRunning:
		valid = to == StatusWaitingInput || to == StatusCanceling || to == StatusSucceeded || to == StatusFailed || to == StatusInterrupted
	case StatusWaitingInput:
		valid = to == StatusRunning || to == StatusCanceling || to == StatusInterrupted
	case StatusCanceling:
		valid = to == StatusCanceled || to == StatusSucceeded || to == StatusFailed || to == StatusInterrupted
	}
	if !valid {
		return fmt.Errorf("execution.invalid-transition:%s:%s", from, to)
	}
	return nil
}

func (origin Origin) Valid() bool {
	return origin == OriginCLI || origin == OriginTUI || origin == OriginWeb
}

type Actor struct {
	Principal PrincipalID `json:"principal"`
	Origin    Origin      `json:"origin"`
}

type Subject struct {
	Kind     string `json:"kind"`
	Project  string `json:"project,omitempty"`
	Key      string `json:"key"`
	Relation string `json:"relation"`
}

func (subject Subject) Valid() bool {
	return subject.Kind != "" && subject.Key != "" && subject.Relation != ""
}

type Encoded struct {
	Schema   SchemaVersion   `json:"schema"`
	JSON     json.RawMessage `json:"json"`
	Redacted bool            `json:"redacted"`
}

type EncodedEventData struct {
	Type   action.EventDataType `json:"type"`
	Schema SchemaVersion        `json:"schema"`
	JSON   json.RawMessage      `json:"json"`
}

type EncodedPrompt struct {
	ID       action.PromptID   `json:"id"`
	Kind     action.PromptKind `json:"kind"`
	Schema   SchemaVersion     `json:"schema"`
	JSON     json.RawMessage   `json:"json"`
	Redacted bool              `json:"redacted"`
}

type Failure struct {
	Code    ErrorCode `json:"code"`
	Message MessageV1 `json:"message"`
}

type Record struct {
	ExecutionID   ExecutionID    `json:"execution_id"`
	AttemptID     AttemptID      `json:"attempt_id"`
	ActionID      action.ID      `json:"action_id"`
	Status        Status         `json:"status"`
	Root          string         `json:"root"`
	Subject       *Subject       `json:"subject,omitempty"`
	Principal     PrincipalID    `json:"principal"`
	Origin        Origin         `json:"origin"`
	Request       Encoded        `json:"request"`
	Result        *Encoded       `json:"result,omitempty"`
	TypedResult   action.Result  `json:"-"`
	Failure       *Failure       `json:"failure,omitempty"`
	PendingPrompt *EncodedPrompt `json:"pending_prompt,omitempty"`
	CreatedAt     time.Time      `json:"created_at"`
	StartedAt     *time.Time     `json:"started_at,omitempty"`
	FinishedAt    *time.Time     `json:"finished_at,omitempty"`
}

type EventKind string

const (
	EventQueued        EventKind = "queued"
	EventStarted       EventKind = "started"
	EventProgress      EventKind = "progress"
	EventWarning       EventKind = "warning"
	EventLog           EventKind = "log"
	EventInputRequired EventKind = "input-required"
	EventCanceling     EventKind = "canceling"
	EventCanceled      EventKind = "canceled"
	EventSucceeded     EventKind = "succeeded"
	EventFailed        EventKind = "failed"
	EventInterrupted   EventKind = "interrupted"
)

type Event struct {
	ExecutionID ExecutionID       `json:"execution_id"`
	AttemptID   AttemptID         `json:"attempt_id"`
	Sequence    EventSequence     `json:"sequence"`
	At          time.Time         `json:"at"`
	Kind        EventKind         `json:"kind"`
	ActionID    action.ID         `json:"action_id"`
	Message     MessageV1         `json:"message"`
	Payload     *EncodedEventData `json:"payload,omitempty"`
	TypedData   action.EventData  `json:"-"`
}

type ListFilter struct {
	Root     string   `json:"root"`
	Statuses []Status `json:"statuses"`
	Limit    uint16   `json:"limit"`
}

type Subscription struct {
	Events <-chan Event
	Errors <-chan error
}
