package contract

import (
	"context"
	"encoding/json"
	"fmt"
)

// SecretKey is a stable keyring identifier and does not itself contain secret
// material.
type SecretKey string

func (k SecretKey) String() string { return string(k) }

// SecretValue deliberately cannot be formatted or JSON encoded. Reveal is an
// explicit operation reserved for the provider boundary that consumes it.
type SecretValue struct {
	value string
}

// NewSecretValue wraps secret material without copying the string data.
func NewSecretValue(value string) SecretValue { return SecretValue{value: value} }

// Reveal returns the wrapped material for use at an authentication or driver
// boundary. Never place the result in errors, events, or report DTOs.
func (s SecretValue) Reveal() string { return s.value }

// Empty reports whether the secret contains no bytes.
func (s SecretValue) Empty() bool { return s.value == "" }

func (SecretValue) String() string   { return "[REDACTED]" }
func (SecretValue) GoString() string { return "contract.SecretValue([REDACTED])" }
func (SecretValue) MarshalText() ([]byte, error) {
	return nil, fmt.Errorf("contract.secret-value-not-serializable")
}
func (SecretValue) MarshalJSON() ([]byte, error) {
	return nil, fmt.Errorf("contract.secret-value-not-serializable")
}

// UnmarshalJSON is intentionally rejected: secret material must enter through
// an explicit secret or credential boundary, not an ordinary config DTO.
func (*SecretValue) UnmarshalJSON([]byte) error {
	return fmt.Errorf("contract.secret-value-not-deserializable")
}

var _ json.Marshaler = SecretValue{}

// SecretStore is the cross-platform keyring boundary. Missing is distinct from
// an empty stored value and errors must not include secret material.
type SecretStore interface {
	Get(context.Context, SecretKey) (value SecretValue, found bool, err error)
	Set(context.Context, SecretKey, SecretValue) error
	Delete(context.Context, SecretKey) (deleted bool, err error)
}
