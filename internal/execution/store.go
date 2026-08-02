package execution

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"time"

	"github.com/sachahjkl/dw/internal/action"
)

type storedExecution struct {
	Record            Record
	RequestHash       [sha256.Size]byte
	IdempotencyKey    IdempotencyKey
	ExecutorID        ExecutorID
	LeaseExpiresAt    time.Time
	Resumable         bool
	LastSequence      EventSequence
	CancelRequestedAt *time.Time
}

type promptUpdate struct {
	PromptID     action.PromptID
	Prompt       *EncodedPrompt
	PromptStatus string
	ResponseJSON json.RawMessage
	RespondedAt  *time.Time
	Redacted     bool
}

type Store interface {
	Create(context.Context, storedExecution, Event) (storedExecution, bool, error)
	Get(context.Context, ExecutionID) (storedExecution, error)
	List(context.Context, PrincipalID, ListFilter) ([]storedExecution, error)
	Commit(context.Context, ExecutorID, storedExecution, *Event, *promptUpdate) error
	EventsAfter(context.Context, ExecutionID, EventSequence, uint16) ([]Event, error)
	Renew(context.Context, ExecutorID, []ExecutionID, time.Time) error
	Recover(context.Context, ExecutorID, time.Time, time.Duration) ([]storedExecution, error)
	Prune(context.Context, string, uint16) error
	Close() error
}

type Submission struct {
	Request        action.Request
	Root           string
	Actor          Actor
	IdempotencyKey IdempotencyKey
}

type Executor interface {
	Submit(context.Context, Submission) (ExecutionID, error)
	Get(context.Context, Actor, ExecutionID) (Record, error)
	List(context.Context, Actor, ListFilter) ([]Record, error)
	Cancel(context.Context, Actor, ExecutionID) error
	Respond(context.Context, Actor, ExecutionID, action.PromptID, action.Response) error
	Subscribe(context.Context, Actor, ExecutionID, EventSequence) (Subscription, error)
	Wait(context.Context, Actor, ExecutionID) (Record, error)
	Close(context.Context) error
}
