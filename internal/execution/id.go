package execution

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
)

const identifierSize = 16

type ExecutorID struct{ value [identifierSize]byte }
type ExecutionID struct{ value [identifierSize]byte }
type AttemptID struct{ value [identifierSize]byte }
type IdempotencyKey struct{ value [identifierSize]byte }

func NewExecutorID() (ExecutorID, error) {
	value, err := newIdentifier()
	return ExecutorID{value: value}, err
}

func NewExecutionID() (ExecutionID, error) {
	value, err := newIdentifier()
	return ExecutionID{value: value}, err
}

func NewAttemptID() (AttemptID, error) {
	value, err := newIdentifier()
	return AttemptID{value: value}, err
}

func NewIdempotencyKey() (IdempotencyKey, error) {
	value, err := newIdentifier()
	return IdempotencyKey{value: value}, err
}

func ParseExecutorID(text string) (ExecutorID, error) {
	value, err := parseIdentifier("executor-id", text)
	return ExecutorID{value: value}, err
}

func ParseExecutionID(text string) (ExecutionID, error) {
	value, err := parseIdentifier("execution-id", text)
	return ExecutionID{value: value}, err
}

func ParseAttemptID(text string) (AttemptID, error) {
	value, err := parseIdentifier("attempt-id", text)
	return AttemptID{value: value}, err
}

func ParseIdempotencyKey(text string) (IdempotencyKey, error) {
	value, err := parseIdentifier("idempotency-key", text)
	return IdempotencyKey{value: value}, err
}

func (id ExecutorID) String() string     { return encodeIdentifier(id.value) }
func (id ExecutionID) String() string    { return encodeIdentifier(id.value) }
func (id AttemptID) String() string      { return encodeIdentifier(id.value) }
func (id IdempotencyKey) String() string { return encodeIdentifier(id.value) }

func (id ExecutorID) IsZero() bool     { return id.value == [identifierSize]byte{} }
func (id ExecutionID) IsZero() bool    { return id.value == [identifierSize]byte{} }
func (id AttemptID) IsZero() bool      { return id.value == [identifierSize]byte{} }
func (id IdempotencyKey) IsZero() bool { return id.value == [identifierSize]byte{} }

func (id ExecutorID) MarshalText() ([]byte, error)     { return []byte(id.String()), nil }
func (id ExecutionID) MarshalText() ([]byte, error)    { return []byte(id.String()), nil }
func (id AttemptID) MarshalText() ([]byte, error)      { return []byte(id.String()), nil }
func (id IdempotencyKey) MarshalText() ([]byte, error) { return []byte(id.String()), nil }

func (id *ExecutorID) UnmarshalText(text []byte) error {
	value, err := ParseExecutorID(string(text))
	if err == nil {
		*id = value
	}
	return err
}

func (id *ExecutionID) UnmarshalText(text []byte) error {
	value, err := ParseExecutionID(string(text))
	if err == nil {
		*id = value
	}
	return err
}

func (id *AttemptID) UnmarshalText(text []byte) error {
	value, err := ParseAttemptID(string(text))
	if err == nil {
		*id = value
	}
	return err
}

func (id *IdempotencyKey) UnmarshalText(text []byte) error {
	value, err := ParseIdempotencyKey(string(text))
	if err == nil {
		*id = value
	}
	return err
}

func newIdentifier() ([identifierSize]byte, error) {
	var value [identifierSize]byte
	if _, err := rand.Read(value[:]); err != nil {
		return value, fmt.Errorf("execution.random-identifier:%w", err)
	}
	return value, nil
}

func parseIdentifier(kind, text string) ([identifierSize]byte, error) {
	var value [identifierSize]byte
	if len(text) != hex.EncodedLen(identifierSize) {
		return value, fmt.Errorf("execution.invalid-%s", kind)
	}
	decoded, err := hex.DecodeString(text)
	if err != nil || hex.EncodeToString(decoded) != text {
		return value, fmt.Errorf("execution.invalid-%s", kind)
	}
	copy(value[:], decoded)
	return value, nil
}

func encodeIdentifier(value [identifierSize]byte) string {
	return hex.EncodeToString(value[:])
}
