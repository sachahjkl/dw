// Package secret stores sensitive values in the operating system credential store.
package secret

import (
	"context"
	"errors"
	"os"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

const (
	// KeyringService and KeyPrefix preserve the account names used by the Rust client.
	KeyringService = "dw"
	KeyPrefix      = "dw/"
)

var (
	ErrEmptyKey                       = newLocalizedError("secret.empty-key", l10n.M("secret.empty-key"), nil)
	defaultStore contract.SecretStore = NewKeyringStore(KeyringService, KeyPrefix)
)

// KeyringStore maps a logical key to one account in the platform keyring. On Linux this is
// Secret Service; on Windows it is Credential Manager.
type KeyringStore struct {
	service string
	prefix  string
}

func NewKeyringStore(service, accountPrefix string) *KeyringStore {
	return &KeyringStore{service: service, prefix: accountPrefix}
}

func DefaultStore() contract.SecretStore { return defaultStore }

func (s *KeyringStore) Set(ctx context.Context, key contract.SecretKey, value contract.SecretValue) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	account, err := s.account(key)
	if err != nil {
		return err
	}
	plaintext := value.Reveal()
	if err := validatePlatformValue(plaintext); err != nil {
		return err
	}
	if err := setCredential(s.service, account, plaintext); err != nil {
		return storeError(err)
	}
	return nil
}

func (s *KeyringStore) Get(ctx context.Context, key contract.SecretKey) (contract.SecretValue, bool, error) {
	if err := ctx.Err(); err != nil {
		return contract.SecretValue{}, false, err
	}
	account, err := s.account(key)
	if err != nil {
		return contract.SecretValue{}, false, err
	}
	value, err := getCredential(s.service, account)
	if errors.Is(err, errCredentialNotFound) {
		return contract.SecretValue{}, false, nil
	}
	if err != nil {
		return contract.SecretValue{}, false, storeError(err)
	}
	return contract.NewSecretValue(value), true, nil
}

func (s *KeyringStore) Delete(ctx context.Context, key contract.SecretKey) (bool, error) {
	if err := ctx.Err(); err != nil {
		return false, err
	}
	account, err := s.account(key)
	if err != nil {
		return false, err
	}
	if err := deleteCredential(s.service, account); err != nil {
		if errors.Is(err, errCredentialNotFound) {
			return false, nil
		}
		return false, storeError(err)
	}
	return true, nil
}

func (s *KeyringStore) account(key contract.SecretKey) (string, error) {
	normalized := strings.TrimSpace(string(key))
	if normalized == "" {
		return "", ErrEmptyKey
	}
	return s.prefix + normalized, nil
}

func storeError(err error) error {
	return newLocalizedError("secret.store-unavailable", l10n.M("secret.store-unavailable", l10n.A("detail", err)), err)
}

// FromEnvironment resolves a value without ever including the value in its error.
func FromEnvironment(name contract.EnvironmentVariable) (contract.SecretValue, error) {
	value, ok := os.LookupEnv(string(name))
	if !ok {
		return contract.SecretValue{}, newLocalizedError(
			"secret.missing-environment",
			l10n.M("secret.missing-environment", l10n.A("name", name)),
			nil,
		)
	}
	return contract.NewSecretValue(value), nil
}

var _ contract.SecretStore = (*KeyringStore)(nil)
