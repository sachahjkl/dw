// Package contract contains provider-neutral value types shared across dw.
// Values in this package are machine contracts; presentation belongs in l10n.
package contract

import (
	"encoding/json"
	"fmt"
)

// String is the common behavior of strongly typed string values.
type String interface {
	~string
}

// NonEmpty validates a required string value without choosing human wording.
func NonEmpty[T String](value T) (T, error) {
	if value == "" {
		return value, fmt.Errorf("contract.empty-value")
	}
	return value, nil
}

type ActionID string

func (v ActionID) String() string { return string(v) }

type ProjectKey string

func (v ProjectKey) String() string { return string(v) }

type WorkspacePath string

func (v WorkspacePath) String() string { return string(v) }

type RepositoryName string

func (v RepositoryName) String() string { return string(v) }

type RepositoryPath string

func (v RepositoryPath) String() string { return string(v) }

type WorkItemID string

func (v WorkItemID) String() string { return string(v) }

type WorkItemType string

func (v WorkItemType) String() string { return string(v) }

type WorkItemState string

func (v WorkItemState) String() string { return string(v) }

type PullRequestID string

func (v PullRequestID) String() string { return string(v) }

type DatabaseKey string

func (v DatabaseKey) String() string { return string(v) }

type EnvironmentName string

func (v EnvironmentName) String() string { return string(v) }

type TableName string

func (v TableName) String() string { return string(v) }

type EnvironmentVariable string

func (v EnvironmentVariable) String() string { return string(v) }

type GitRevision string

func (v GitRevision) String() string { return string(v) }

type SemanticVersion string

func (v SemanticVersion) String() string { return string(v) }

type Timestamp string

func (v Timestamp) String() string { return string(v) }

// ColorMode is serialized exactly as its string value.
type ColorMode string

const (
	ColorAuto   ColorMode = "auto"
	ColorAlways ColorMode = "always"
	ColorNever  ColorMode = "never"
)

func (v ColorMode) Valid() bool {
	return v == ColorAuto || v == ColorAlways || v == ColorNever
}

// Agent is the stable configured external-agent token.
type Agent string

const (
	AgentOpenCode    Agent = "opencode"
	AgentCursor      Agent = "cursor"
	AgentCursorAgent Agent = "cursor-agent"
	AgentGeneric     Agent = "agent"
	AgentClaude      Agent = "claude"
	AgentCodexCLI    Agent = "codex-cli"
	AgentCodex       Agent = "codex"
	AgentCopilot     Agent = "copilot"
)

func (v Agent) Valid() bool {
	switch v {
	case AgentOpenCode, AgentCursor, AgentCursorAgent, AgentGeneric, AgentClaude, AgentCodexCLI, AgentCodex, AgentCopilot:
		return true
	default:
		return false
	}
}

// Optional distinguishes an absent field from a present zero value. JSON null
// is represented by wirejson rather than Optional.
type Optional[T any] struct {
	value T
	set   bool
}

func Some[T any](value T) Optional[T] { return Optional[T]{value: value, set: true} }
func None[T any]() Optional[T]        { return Optional[T]{} }
func (o Optional[T]) Get() (T, bool)  { return o.value, o.set }
func (o Optional[T]) IsSet() bool     { return o.set }
func (o Optional[T]) IsZero() bool    { return !o.set }
func (o Optional[T]) MarshalJSON() ([]byte, error) {
	if !o.set {
		return []byte("null"), nil
	}
	return json.Marshal(o.value)
}
func (o Optional[T]) Or(fallback T) T {
	if o.set {
		return o.value
	}
	return fallback
}
