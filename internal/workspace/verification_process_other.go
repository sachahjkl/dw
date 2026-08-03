//go:build !windows

package workspace

import "os/exec"

func configureVerificationProcess(_ *exec.Cmd) {}
