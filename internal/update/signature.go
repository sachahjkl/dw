package update

import (
	"bytes"
	"crypto/ed25519"
	"encoding/base64"
	"fmt"
	"strings"

	"golang.org/x/crypto/blake2b"
)

// TrustedPublicKeys lists the minisign public keys allowed to sign release manifests.
// Add the next key here before rotating the CI secret, then remove the old one later.
var TrustedPublicKeys = []string{
	"RWTdsGAfmDUoLMf4q+Gp4eoOnn0+n721Rryg2Up3E1VKBKbDo+3+E84A",
}

const (
	minisignAlgorithmLegacy    = "Ed"
	minisignAlgorithmPrehashed = "ED"
	minisignKeyIDSize          = 8
	trustedCommentPrefix       = "trusted comment: "
)

type minisignPublicKey struct {
	keyID [minisignKeyIDSize]byte
	key   ed25519.PublicKey
}

func parseMinisignPublicKey(encoded string) (minisignPublicKey, error) {
	raw, err := base64.StdEncoding.DecodeString(strings.TrimSpace(encoded))
	if err != nil || len(raw) != 2+minisignKeyIDSize+ed25519.PublicKeySize || string(raw[:2]) != minisignAlgorithmLegacy {
		return minisignPublicKey{}, fmt.Errorf("update: invalid-minisign-public-key")
	}
	var key minisignPublicKey
	copy(key.keyID[:], raw[2:2+minisignKeyIDSize])
	key.key = ed25519.PublicKey(bytes.Clone(raw[2+minisignKeyIDSize:]))
	return key, nil
}

// VerifyManifestSignature checks a minisign signature (legacy or prehashed) against
// the trusted keys, including the global signature over the trusted comment.
func VerifyManifestSignature(manifest, signature []byte, trustedKeys []string) error {
	lines := strings.Split(strings.ReplaceAll(string(signature), "\r\n", "\n"), "\n")
	if len(lines) < 4 || !strings.HasPrefix(lines[2], trustedCommentPrefix) {
		return fmt.Errorf("update: invalid-manifest-signature-format")
	}
	signatureBlock, err := base64.StdEncoding.DecodeString(strings.TrimSpace(lines[1]))
	if err != nil || len(signatureBlock) != 2+minisignKeyIDSize+ed25519.SignatureSize {
		return fmt.Errorf("update: invalid-manifest-signature-format")
	}
	algorithm := string(signatureBlock[:2])
	var keyID [minisignKeyIDSize]byte
	copy(keyID[:], signatureBlock[2:2+minisignKeyIDSize])
	fileSignature := signatureBlock[2+minisignKeyIDSize:]
	trustedComment := strings.TrimPrefix(lines[2], trustedCommentPrefix)
	globalSignature, err := base64.StdEncoding.DecodeString(strings.TrimSpace(lines[3]))
	if err != nil || len(globalSignature) != ed25519.SignatureSize {
		return fmt.Errorf("update: invalid-manifest-signature-format")
	}

	var message []byte
	switch algorithm {
	case minisignAlgorithmPrehashed:
		digest := blake2b.Sum512(manifest)
		message = digest[:]
	case minisignAlgorithmLegacy:
		message = manifest
	default:
		return fmt.Errorf("update: unsupported-manifest-signature-algorithm")
	}

	for _, encoded := range trustedKeys {
		key, err := parseMinisignPublicKey(encoded)
		if err != nil {
			return err
		}
		if key.keyID != keyID {
			continue
		}
		if !ed25519.Verify(key.key, message, fileSignature) {
			return fmt.Errorf("update: invalid-manifest-signature")
		}
		if !ed25519.Verify(key.key, append(bytes.Clone(fileSignature), trustedComment...), globalSignature) {
			return fmt.Errorf("update: invalid-manifest-trusted-comment-signature")
		}
		return nil
	}
	return fmt.Errorf("update: manifest-signed-by-untrusted-key")
}
