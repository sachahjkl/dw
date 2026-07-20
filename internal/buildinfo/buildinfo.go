// Package buildinfo exposes link-time build metadata without coupling the
// product version used by machine contracts to human-facing provenance.
package buildinfo

import "strings"

const product = "dw"

// Version is the release version. Release builds set it with -ldflags -X.
var Version = "0.0.0-dev"

// Commit is the source revision. Release builds set it with -ldflags -X.
var Commit string

// Product returns the stable executable and release-asset product name.
func Product() string { return product }

// Informational returns Version with a short source revision when available.
// Version itself remains unchanged for update and machine-format comparisons.
func Informational() string {
	commit := strings.TrimSpace(Commit)
	if commit == "" {
		return Version
	}
	if len(commit) > 7 {
		commit = commit[:7]
	}
	return Version + "+" + commit
}
