//go:build linux

package webservice

import (
	"errors"
	"strings"
	"testing"
)

func TestSystemctlErrorClassifiesUnavailableUserManager(t *testing.T) {
	tests := []string{
		"Failed to connect to bus: No medium found",
		"Failed to connect to user scope bus via local transport: No such file or directory",
	}
	for _, output := range tests {
		err := systemctlError([]byte(output), errors.New("exit status 1"))
		if !strings.HasPrefix(err.Error(), "web.service-manager-unavailable:") {
			t.Fatalf("systemctl error for %q = %q", output, err)
		}
	}
}

func TestSystemctlErrorPreservesOtherFailures(t *testing.T) {
	err := systemctlError([]byte("permission denied"), errors.New("exit status 1"))
	if !strings.HasPrefix(err.Error(), "web.systemctl:") {
		t.Fatalf("systemctl error = %q", err)
	}
}
