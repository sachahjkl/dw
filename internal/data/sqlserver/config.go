package sqlserver

import (
	"context"
	"errors"
	"os"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	ProviderName       = "sql-server"
	LegacyProviderName = "sqlserver"
)

func IsProviderName(value string) bool {
	value = strings.TrimSpace(value)
	return strings.EqualFold(value, ProviderName) || strings.EqualFold(value, LegacyProviderName)
}

const (
	DefaultMaxRows        = 500
	DefaultTimeoutSeconds = 600
)

type Secret = contract.SecretValue
type SecretStore = contract.SecretStore

func NewSecret(value string) Secret { return contract.NewSecretValue(value) }

type Defaults struct {
	ReadOnly       bool `json:"readonly"`
	MaxRows        int  `json:"maxRows"`
	TimeoutSeconds int  `json:"timeoutSeconds"`
}

func DefaultSettings() Defaults {
	return Defaults{ReadOnly: true, MaxRows: DefaultMaxRows, TimeoutSeconds: DefaultTimeoutSeconds}
}

// ConnectionConfig cannot serialize its inline secret; configuration loading is an explicit
// dbcompat boundary and all report DTOs contain only masked source metadata.
type ConnectionConfig struct {
	Provider                            string               `json:"provider"`
	ConnectionString                    contract.SecretValue `json:"-"`
	ConnectionStringEnvironmentVariable string               `json:"connectionStringEnvironmentVariable,omitempty"`
	CredentialKey                       string               `json:"credentialKey,omitempty"`
	ReadOnly                            *bool                `json:"readonly,omitempty"`
	MaxRows                             *int                 `json:"maxRows,omitempty"`
	TimeoutSeconds                      *int                 `json:"timeoutSeconds,omitempty"`
}

type ResolvedConnection struct {
	Config   ConnectionConfig
	Defaults Defaults
}

type ErrorKind string

const (
	ErrorUnsupportedProvider ErrorKind = "unsupported-provider"
	ErrorBlockedQuery        ErrorKind = "blocked-query"
	ErrorMissingConnection   ErrorKind = "missing-connection-string"
	ErrorMissingSecret       ErrorKind = "missing-secret"
	ErrorSecretStore         ErrorKind = "secret-store"
	ErrorTimeout             ErrorKind = "timeout"
	ErrorSQL                 ErrorKind = "sql"
	ErrorReadOnlyRequired    ErrorKind = "readonly-required"
)

type ProviderError struct {
	Kind     ErrorKind
	Provider string
	Reason   string
	Key      string
	Seconds  int
}

func (problem *ProviderError) Error() string {
	switch problem.Kind {
	case ErrorUnsupportedProvider:
		return l10n.Render(l10n.M("db.error.unsupported_provider", l10n.A("provider", problem.Provider)))
	case ErrorBlockedQuery:
		return l10n.Render(l10n.M("db.error.blocked", l10n.A("reason", problem.Reason)))
	case ErrorMissingConnection:
		return l10n.Text("db.error.missing_connection")
	case ErrorMissingSecret:
		return l10n.Render(l10n.M("db.error.missing_secret", l10n.A("key", problem.Key)))
	case ErrorSecretStore:
		return l10n.Text("db.error.secret_store")
	case ErrorTimeout:
		return l10n.Render(l10n.M("db.error.timeout", l10n.A("seconds", problem.Seconds)))
	case ErrorReadOnlyRequired:
		return l10n.Text("db.error.readonly")
	default:
		return l10n.Render(l10n.M("db.error.sql", l10n.A("error", problem.Reason)))
	}
}

func IsErrorKind(err error, kind ErrorKind) bool {
	var problem *ProviderError
	return errors.As(err, &problem) && problem.Kind == kind
}

// Resolve validates the non-negotiable read-only invariant and applies compatibility defaults.
func Resolve(config ConnectionConfig, defaults Defaults) (ResolvedConnection, error) {
	if !defaults.ReadOnly || config.ReadOnly != nil && !*config.ReadOnly {
		return ResolvedConnection{}, &ProviderError{Kind: ErrorReadOnlyRequired}
	}
	return ResolvedConnection{Config: config, Defaults: defaults}, nil
}

// ResolveConnectionString applies the established precedence: nonblank inline, then a present and
// nonblank environment value, then keyring. A configured but absent environment value falls through.
func ResolveConnectionString(ctx context.Context, config ConnectionConfig, store contract.SecretStore) (contract.SecretValue, error) {
	if strings.TrimSpace(config.ConnectionString.Reveal()) != "" {
		return config.ConnectionString, nil
	}
	if variable := strings.TrimSpace(config.ConnectionStringEnvironmentVariable); variable != "" {
		if value, found := os.LookupEnv(variable); found && strings.TrimSpace(value) != "" {
			return contract.NewSecretValue(value), nil
		}
	}
	if key := strings.TrimSpace(config.CredentialKey); key != "" {
		if store == nil {
			return contract.SecretValue{}, &ProviderError{Kind: ErrorSecretStore}
		}
		secret, found, err := store.Get(ctx, contract.SecretKey(key))
		if err != nil {
			return contract.SecretValue{}, &ProviderError{Kind: ErrorSecretStore, Key: key}
		}
		if !found || strings.TrimSpace(secret.Reveal()) == "" {
			return contract.SecretValue{}, &ProviderError{Kind: ErrorMissingSecret, Key: key}
		}
		return secret, nil
	}
	return contract.SecretValue{}, &ProviderError{Kind: ErrorMissingConnection}
}

// EqualSecrets is used only for conservative keyring conflict checks.
func EqualSecrets(left, right contract.SecretValue) bool { return left.Reveal() == right.Reveal() }
