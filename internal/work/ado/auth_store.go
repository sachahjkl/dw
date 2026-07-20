package ado

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"strconv"
	"strings"
	"unicode/utf16"

	"github.com/sachahjkl/dw/internal/contract"
)

const (
	KeyringService       = "dw.azure-devops"
	KeyringAccount       = "oauth-refresh-token"
	keyringChunkPrefix   = "dw-refresh-token-v1"
	keyringChunkUTF16Max = 1000
)

func (a *Authenticator) storeRefreshToken(ctx context.Context, refreshToken string) error {
	if a == nil || a.Store == nil {
		return &Error{Kind: ErrorKeyring, Detail: "credential store is not configured"}
	}
	previousValue, exists, err := a.Store.Get(ctx, contract.SecretKey(KeyringAccount))
	if err != nil {
		return keyringError(err)
	}
	previous := previousValue.Reveal()
	previousGeneration, previousCount, previousIsManifest := parseChunkManifest(previous)
	chunks := splitKeyringChunks(refreshToken)
	if len(chunks) == 1 {
		if err := a.Store.Set(ctx, contract.SecretKey(KeyringAccount), contract.NewSecretValue(refreshToken)); err != nil {
			return keyringError(err)
		}
	} else {
		generation, err := randomGeneration()
		if err != nil {
			return keyringError(err)
		}
		for index, chunk := range chunks {
			if err := a.Store.Set(ctx, contract.SecretKey(chunkAccount(generation, index)), contract.NewSecretValue(chunk)); err != nil {
				return keyringError(err)
			}
		}
		if err := a.Store.Set(ctx, contract.SecretKey(KeyringAccount), contract.NewSecretValue(chunkManifest(generation, len(chunks)))); err != nil {
			return keyringError(err)
		}
	}
	if exists && previousIsManifest {
		if err := a.deleteKeyringChunks(ctx, previousGeneration, previousCount); err != nil {
			return err
		}
	}
	return nil
}

func (a *Authenticator) readRefreshToken(ctx context.Context) (string, bool, error) {
	if a == nil || a.Store == nil {
		return "", false, &Error{Kind: ErrorKeyring, Detail: "credential store is not configured"}
	}
	storedValue, ok, err := a.Store.Get(ctx, contract.SecretKey(KeyringAccount))
	if err != nil {
		return "", false, keyringError(err)
	}
	stored := storedValue.Reveal()
	if !ok || stored == "" {
		return "", false, nil
	}
	generation, count, manifest := parseChunkManifest(stored)
	if !manifest {
		return stored, true, nil
	}
	var value strings.Builder
	for index := 0; index < count; index++ {
		chunkValue, found, err := a.Store.Get(ctx, contract.SecretKey(chunkAccount(generation, index)))
		if err != nil {
			return "", false, keyringError(err)
		}
		if !found {
			return "", false, &Error{Kind: ErrorKeyring, Detail: "Stored refresh token chunk " + strconv.Itoa(index) + " is missing."}
		}
		value.WriteString(chunkValue.Reveal())
	}
	return value.String(), true, nil
}

func (a *Authenticator) deleteStoredRefreshToken(ctx context.Context) (bool, error) {
	if a == nil || a.Store == nil {
		return false, &Error{Kind: ErrorKeyring, Detail: "credential store is not configured"}
	}
	storedValue, _, err := a.Store.Get(ctx, contract.SecretKey(KeyringAccount))
	if err != nil {
		return false, keyringError(err)
	}
	generation, count, manifest := parseChunkManifest(storedValue.Reveal())
	deleted, err := a.Store.Delete(ctx, contract.SecretKey(KeyringAccount))
	if err != nil {
		return false, keyringError(err)
	}
	if manifest {
		if err := a.deleteKeyringChunks(ctx, generation, count); err != nil {
			return false, err
		}
	}
	return deleted, nil
}

func (a *Authenticator) deleteKeyringChunks(ctx context.Context, generation string, count int) error {
	for index := 0; index < count; index++ {
		if _, err := a.Store.Delete(ctx, contract.SecretKey(chunkAccount(generation, index))); err != nil {
			return keyringError(err)
		}
	}
	return nil
}

func splitKeyringChunks(value string) []string {
	chunks := make([]string, 0, len(value)/keyringChunkUTF16Max+1)
	var chunk strings.Builder
	units := 0
	for _, character := range value {
		characterUnits := 1
		if utf16.RuneLen(character) == 2 {
			characterUnits = 2
		}
		if units+characterUnits > keyringChunkUTF16Max && chunk.Len() != 0 {
			chunks = append(chunks, chunk.String())
			chunk.Reset()
			units = 0
		}
		chunk.WriteRune(character)
		units += characterUnits
	}
	chunks = append(chunks, chunk.String())
	return chunks
}

func chunkManifest(generation string, count int) string {
	return keyringChunkPrefix + ":" + generation + ":" + strconv.Itoa(count)
}

func parseChunkManifest(value string) (string, int, bool) {
	parts := strings.Split(value, ":")
	if len(parts) != 3 || parts[0] != keyringChunkPrefix || parts[1] == "" {
		return "", 0, false
	}
	count, err := strconv.Atoi(parts[2])
	if err != nil || count < 2 {
		return "", 0, false
	}
	return parts[1], count, true
}

func chunkAccount(generation string, index int) string {
	return KeyringAccount + "." + generation + "." + strconv.Itoa(index)
}

func randomGeneration() (string, error) {
	var data [8]byte
	if _, err := rand.Read(data[:]); err != nil {
		return "", err
	}
	return hex.EncodeToString(data[:]), nil
}

func keyringError(err error) error {
	return &Error{Kind: ErrorKeyring, Detail: err.Error(), Cause: err}
}
