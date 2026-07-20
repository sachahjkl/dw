package paritytest_test

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/sachahjkl/dw/internal/platform"
	"github.com/sachahjkl/dw/internal/update"
)

func TestUpdatePreparationRejectsForeignRuntimeIdentifier(t *testing.T) {
	current, err := platform.RuntimeIdentifier()
	if err != nil {
		t.Skipf("published update runtime is unavailable: %v", err)
	}
	foreign := platform.RIDWindowsX64
	assetName := "dw.exe"
	contents := []byte{'M', 'Z'}
	if current == platform.RIDWindowsX64 {
		foreign = platform.RIDLinuxX64
		assetName = "dw"
		contents = []byte("#!/bin/sh\n")
	}
	assetPath := filepath.Join(t.TempDir(), assetName)
	if err := os.WriteFile(assetPath, contents, 0o755); err != nil {
		t.Fatal(err)
	}

	_, err = update.PrepareReplacement(assetName, assetPath, foreign, t.TempDir())
	if err == nil {
		t.Fatalf("foreign RID %q was accepted on host RID %q", foreign, current)
	}
	var mismatch *update.HostRIDMismatchError
	if !errors.As(err, &mismatch) || mismatch.Requested != foreign || mismatch.Host != current {
		t.Fatalf("foreign RID error = %#v, want requested=%q host=%q", err, foreign, current)
	}
	_, err = update.ResolveInstallRID(foreign)
	if !errors.As(err, &mismatch) || mismatch.Requested != foreign || mismatch.Host != current {
		t.Fatalf("install preflight error = %#v, want requested=%q host=%q", err, foreign, current)
	}
}
