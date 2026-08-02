package web

import (
	"encoding/json"
	"time"

	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/webservice"
)

const schemaV1 uint16 = 1

type ResourceRefV1 struct {
	Kind    string `json:"kind"`
	Root    string `json:"root"`
	Project string `json:"project,omitempty"`
	Key     string `json:"key"`
}

type OperationInputV1 struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

type OperationSubmitV1 struct {
	Schema         uint16             `json:"schema"`
	IdempotencyKey string             `json:"idempotencyKey"`
	Resource       ResourceRefV1      `json:"resource"`
	Relation       string             `json:"relation"`
	Inputs         []OperationInputV1 `json:"inputs"`
}

type ExecutionRefV1 struct {
	Schema      uint16 `json:"schema"`
	ExecutionID string `json:"executionId"`
	AttemptID   string `json:"attemptId"`
}

type PayloadV1 struct {
	Type   string          `json:"type"`
	Schema uint16          `json:"schema"`
	JSON   json.RawMessage `json:"json"`
}

type EventV1 struct {
	Schema      uint16                  `json:"schema"`
	ExecutionID string                  `json:"executionId"`
	AttemptID   string                  `json:"attemptId"`
	Sequence    execution.EventSequence `json:"sequence"`
	At          time.Time               `json:"at"`
	Kind        execution.EventKind     `json:"kind"`
	ActionID    string                  `json:"actionId"`
	Message     execution.MessageV1     `json:"message"`
	Payload     *PayloadV1              `json:"payload,omitempty"`
}

type RecordV1 struct {
	Schema        uint16                   `json:"schema"`
	ExecutionID   string                   `json:"executionId"`
	AttemptID     string                   `json:"attemptId"`
	ActionID      string                   `json:"actionId"`
	Status        execution.Status         `json:"status"`
	Root          string                   `json:"root"`
	Subject       *execution.Subject       `json:"subject,omitempty"`
	Origin        execution.Origin         `json:"origin"`
	Result        *execution.Encoded       `json:"result,omitempty"`
	Failure       *execution.Failure       `json:"failure,omitempty"`
	PendingPrompt *execution.EncodedPrompt `json:"pendingPrompt,omitempty"`
	CreatedAt     time.Time                `json:"createdAt"`
	StartedAt     *time.Time               `json:"startedAt,omitempty"`
	FinishedAt    *time.Time               `json:"finishedAt,omitempty"`
}

type TextResponseV1 struct {
	Schema uint16 `json:"schema"`
	Value  string `json:"value"`
}
type SecretResponseV1 struct {
	Schema uint16 `json:"schema"`
	Value  string `json:"value"`
}
type ConfirmResponseV1 struct {
	Schema   uint16 `json:"schema"`
	Accepted bool   `json:"accepted"`
}
type SelectOneResponseV1 struct {
	Schema uint16 `json:"schema"`
	Value  string `json:"value"`
}
type SelectManyResponseV1 struct {
	Schema uint16   `json:"schema"`
	Values []string `json:"values"`
}

type TicketRequestV1 struct {
	Schema   uint16 `json:"schema"`
	NoExpiry bool   `json:"noExpiry"`
}

type TicketV1 struct {
	Schema    uint16     `json:"schema"`
	Ticket    string     `json:"ticket"`
	ExpiresAt *time.Time `json:"expiresAt,omitempty"`
}

type ShutdownV1 struct {
	Schema   uint16              `json:"schema"`
	ServerID webservice.ServerID `json:"serverId"`
}
