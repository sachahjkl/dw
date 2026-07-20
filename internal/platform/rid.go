package platform

import (
	"fmt"
	"runtime"
	"strings"
)

const (
	RIDLinuxX64   = "linux-x64"
	RIDWindowsX64 = "win-x64"
)

// RuntimeIdentifier returns the release runtime identifier for this process.
// dw publishes only Linux and Windows x86-64 release binaries.
func RuntimeIdentifier() (string, error) {
	if runtime.GOARCH != "amd64" {
		return "", fmt.Errorf("platform: unsupported-runtime %s/%s", runtime.GOOS, runtime.GOARCH)
	}
	switch runtime.GOOS {
	case "linux":
		return RIDLinuxX64, nil
	case "windows":
		return RIDWindowsX64, nil
	default:
		return "", fmt.Errorf("platform: unsupported-runtime %s/%s", runtime.GOOS, runtime.GOARCH)
	}
}

// NormalizeRuntimeIdentifier accepts the two published RIDs case-insensitively.
func NormalizeRuntimeIdentifier(rid string) (string, error) {
	switch {
	case strings.EqualFold(rid, RIDLinuxX64):
		return RIDLinuxX64, nil
	case strings.EqualFold(rid, RIDWindowsX64):
		return RIDWindowsX64, nil
	default:
		return "", fmt.Errorf("platform: unsupported-rid %q", rid)
	}
}

type Replacement struct {
	ExecutablePath             string `json:"executable_path"`
	DeferredWindowsReplacement bool   `json:"deferred_windows_replacement"`
}
