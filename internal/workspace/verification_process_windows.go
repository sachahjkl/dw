//go:build windows

package workspace

import (
	"os/exec"
	"syscall"

	"golang.org/x/sys/windows"
)

func configureVerificationProcess(process *exec.Cmd) {
	process.SysProcAttr = &syscall.SysProcAttr{
		CreationFlags: windows.CREATE_NO_WINDOW,
		HideWindow:    true,
	}
}
