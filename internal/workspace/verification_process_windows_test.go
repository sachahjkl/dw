//go:build windows

package workspace

import (
	"os/exec"
	"testing"

	"golang.org/x/sys/windows"
)

func TestVerificationProcessDoesNotCreateWindow(t *testing.T) {
	process := exec.Command("powershell.exe")
	configureVerificationProcess(process)
	if process.SysProcAttr == nil {
		t.Fatal("verification process has no Windows process attributes")
	}
	if process.SysProcAttr.CreationFlags&windows.CREATE_NO_WINDOW == 0 {
		t.Fatal("verification process may create a console window")
	}
	if !process.SysProcAttr.HideWindow {
		t.Fatal("verification process window is not hidden")
	}
}
