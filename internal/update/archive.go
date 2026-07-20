package update

import (
	"archive/tar"
	"archive/zip"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/platform"
)

var windowsPESignature = [2]byte{'M', 'Z'}

// FileSHA256 hashes the downloaded asset itself. For .zip and .tar.gz releases
// this deliberately verifies the compressed archive, not the extracted binary.
func FileSHA256(fileName string) (string, error) {
	file, err := os.Open(fileName)
	if err != nil {
		return "", fmt.Errorf("update: open-checksum-file: %w", err)
	}
	defer file.Close()
	digest := sha256.New()
	if _, err := io.Copy(digest, file); err != nil {
		return "", fmt.Errorf("update: hash-asset: %w", err)
	}
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func PrepareReplacement(assetFileName, assetPath, rid, tempDir string) (string, error) {
	normalizedRID, err := ResolveInstallRID(rid)
	if err != nil {
		return "", err
	}
	switch {
	case strings.HasSuffix(assetFileName, ".zip"):
		if normalizedRID != platform.RIDWindowsX64 {
			_ = os.Remove(assetPath)
			return "", fmt.Errorf("update: zip-requires-win-x64")
		}
		return extractWindowsExecutable(assetPath, tempDir)
	case strings.HasSuffix(assetFileName, ".tar.gz"), strings.HasSuffix(assetFileName, ".tgz"):
		if normalizedRID != platform.RIDLinuxX64 {
			_ = os.Remove(assetPath)
			return "", fmt.Errorf("update: tar-gz-requires-linux-x64")
		}
		return extractUnixExecutable(assetPath, normalizedRID, tempDir)
	case strings.HasSuffix(assetFileName, ".exe"):
		if normalizedRID != platform.RIDWindowsX64 {
			_ = os.Remove(assetPath)
			return "", fmt.Errorf("update: exe-requires-win-x64")
		}
		if err := ensureWindowsExecutable(assetPath, assetFileName); err != nil {
			return "", err
		}
		return assetPath, nil
	case normalizedRID == platform.RIDWindowsX64:
		if err := ensureWindowsExecutable(assetPath, assetFileName); err != nil {
			return "", err
		}
		return assetPath, nil
	default:
		if err := ensureUnixExecutable(assetPath, assetFileName, normalizedRID); err != nil {
			return "", err
		}
		return assetPath, nil
	}
}

func extractWindowsExecutable(archivePath, tempDir string) (replacement string, resultErr error) {
	defer os.Remove(archivePath)
	archive, err := zip.OpenReader(archivePath)
	if err != nil {
		return "", fmt.Errorf("update: open-zip: %w", err)
	}
	defer archive.Close()
	for _, entry := range archive.File {
		if !strings.EqualFold(path.Base(entry.Name), "dw.exe") || entry.FileInfo().IsDir() {
			continue
		}
		input, err := entry.Open()
		if err != nil {
			return "", fmt.Errorf("update: open-zip-entry: %w", err)
		}
		output, err := os.CreateTemp(tempDir, "dw-upgrade-*.exe")
		if err != nil {
			input.Close()
			return "", fmt.Errorf("update: create-extracted-executable: %w", err)
		}
		replacement = output.Name()
		if _, err = io.Copy(output, input); err == nil {
			err = output.Sync()
		}
		closeOutputErr := output.Close()
		closeInputErr := input.Close()
		if err == nil {
			err = closeOutputErr
		}
		if err == nil {
			err = closeInputErr
		}
		if err != nil {
			_ = os.Remove(replacement)
			return "", fmt.Errorf("update: extract-zip-entry: %w", err)
		}
		if err := ensureWindowsExecutable(replacement, entry.Name); err != nil {
			_ = os.Remove(replacement)
			return "", err
		}
		return replacement, nil
	}
	return "", fmt.Errorf("update: invalid-archive-dw.exe-not-found")
}

func extractUnixExecutable(archivePath, rid, tempDir string) (replacement string, resultErr error) {
	defer os.Remove(archivePath)
	file, err := os.Open(archivePath)
	if err != nil {
		return "", fmt.Errorf("update: open-tar-gz: %w", err)
	}
	defer file.Close()
	decoder, err := gzip.NewReader(file)
	if err != nil {
		return "", fmt.Errorf("update: open-gzip: %w", err)
	}
	defer decoder.Close()
	archive := tar.NewReader(decoder)
	for {
		header, err := archive.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return "", fmt.Errorf("update: read-tar-entry: %w", err)
		}
		if path.Base(header.Name) != "dw" || !header.FileInfo().Mode().IsRegular() {
			continue
		}
		output, err := os.CreateTemp(tempDir, "dw-upgrade-*")
		if err != nil {
			return "", fmt.Errorf("update: create-extracted-executable: %w", err)
		}
		replacement = output.Name()
		if _, err = io.Copy(output, archive); err == nil {
			err = output.Sync()
		}
		closeErr := output.Close()
		if err == nil {
			err = closeErr
		}
		if err != nil {
			_ = os.Remove(replacement)
			return "", fmt.Errorf("update: extract-tar-entry: %w", err)
		}
		if err := ensureUnixExecutable(replacement, "dw", rid); err != nil {
			_ = os.Remove(replacement)
			return "", err
		}
		return replacement, nil
	}
	return "", fmt.Errorf("update: invalid-archive-dw-not-found")
}

func ensureWindowsExecutable(fileName, displayName string) error {
	file, err := os.Open(fileName)
	if err != nil {
		return fmt.Errorf("update: open-windows-executable: %w", err)
	}
	var signature [2]byte
	_, readErr := io.ReadFull(file, signature[:])
	closeErr := file.Close()
	if readErr != nil {
		_ = os.Remove(fileName)
		return fmt.Errorf("update: read-windows-signature: %w", readErr)
	}
	if closeErr != nil {
		_ = os.Remove(fileName)
		return fmt.Errorf("update: close-windows-executable: %w", closeErr)
	}
	if signature != windowsPESignature {
		_ = os.Remove(fileName)
		return fmt.Errorf("update: invalid-windows-executable %q", displayName)
	}
	return nil
}

func ensureUnixExecutable(fileName, displayName, rid string) error {
	if rid == platform.RIDWindowsX64 {
		return fmt.Errorf("update: invalid-windows-executable %q", displayName)
	}
	info, err := os.Stat(fileName)
	if err != nil {
		return fmt.Errorf("update: stat-unix-executable: %w", err)
	}
	if !info.Mode().IsRegular() {
		_ = os.Remove(fileName)
		return fmt.Errorf("update: invalid-executable-file %q", displayName)
	}
	if err := os.Chmod(fileName, 0o755); err != nil {
		return fmt.Errorf("update: chmod-executable: %w", err)
	}
	return nil
}

func extensionSuffix(fileName string) string {
	if strings.HasSuffix(fileName, ".tar.gz") {
		return ".tar.gz"
	}
	return filepath.Ext(fileName)
}
