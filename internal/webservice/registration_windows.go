//go:build windows

package webservice

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"syscall"
	"unicode/utf8"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/l10n"
	"golang.org/x/sys/windows"
)

const (
	scheduledTaskPath = `\DevWorkflow\`
	scheduledTaskName = scheduledTaskPath + `dw-web`
)

type taskSchedulerManager struct{}

func newNativeManager(config.PlatformBaseDirs) (NativeManager, error) {
	return &taskSchedulerManager{}, nil
}

func (manager *taskSchedulerManager) Registration() Registration { return RegistrationTaskScheduler }
func (manager *taskSchedulerManager) Register(ctx context.Context, executable string) error {
	command, err := scheduledTaskCommand(executable)
	if err != nil {
		return err
	}
	if err := schtasks(ctx, "/Create", "/TN", scheduledTaskName, "/SC", "ONLOGON", "/TR", command, "/RL", "LIMITED", "/F"); err != nil {
		return err
	}
	if err := manager.Start(ctx); err != nil {
		_ = schtasks(ctx, "/Delete", "/TN", scheduledTaskName, "/F")
		return err
	}
	return nil
}

func scheduledTaskCommand(executable string) (string, error) {
	if executable == "" || strings.ContainsAny(executable, "\"\r\n") {
		return "", fmt.Errorf("web.invalid-executable-path:%s", executable)
	}
	return `"` + executable + `" web serve`, nil
}

func (manager *taskSchedulerManager) Unregister(ctx context.Context) error {
	_ = manager.Stop(ctx)
	return schtasks(ctx, "/Delete", "/TN", scheduledTaskName, "/F")
}
func (manager *taskSchedulerManager) Start(ctx context.Context) error {
	return schtasks(ctx, "/Run", "/TN", scheduledTaskName)
}

func (manager *taskSchedulerManager) Restart(ctx context.Context) error {
	_ = manager.Stop(ctx)
	return manager.Start(ctx)
}
func (manager *taskSchedulerManager) Stop(ctx context.Context) error {
	return schtasks(ctx, "/End", "/TN", scheduledTaskName)
}

// Running reads the task state enum through PowerShell because schtasks prints
// a localized status column.
func (manager *taskSchedulerManager) Running(ctx context.Context) (bool, error) {
	script := fmt.Sprintf("(Get-ScheduledTask -TaskPath '%s' -TaskName '%s' -ErrorAction Stop).State", scheduledTaskPath, strings.TrimPrefix(scheduledTaskName, scheduledTaskPath))
	command := exec.CommandContext(ctx, "powershell.exe", "-NoProfile", "-NonInteractive", "-Command", script)
	output, err := command.Output()
	if err != nil {
		var exit *exec.ExitError
		if errors.As(err, &exit) && exit.ExitCode() == 1 {
			return false, nil
		}
		return false, err
	}
	return taskStateRunning(decodeConsoleOutput(output)), nil
}

func taskStateRunning(state string) bool { return strings.TrimSpace(state) == "Running" }

func schtasks(ctx context.Context, arguments ...string) error {
	command := exec.CommandContext(ctx, "schtasks.exe", arguments...)
	output, err := command.CombinedOutput()
	if err != nil {
		return l10n.WrapError(err, "web.error.system-service", l10n.A("detail", strings.TrimSpace(decodeConsoleOutput(output))))
	}
	return nil
}

const utf8CodePage = 65001

var procGetOEMCP = windows.NewLazySystemDLL("kernel32.dll").NewProc("GetOEMCP")

func decodeConsoleOutput(output []byte) string {
	if len(output) == 0 {
		return ""
	}
	codePage, err := windows.GetConsoleOutputCP()
	if err != nil || codePage == 0 {
		if procGetOEMCP.Find() == nil {
			value, _, _ := procGetOEMCP.Call()
			codePage = uint32(value)
		}
	}
	if codePage == 0 || codePage == utf8CodePage {
		if utf8.Valid(output) {
			return string(output)
		}
		if codePage == utf8CodePage {
			return strings.ToValidUTF8(string(output), "\uFFFD")
		}
		codePage = windows.GetACP()
	}
	return decodeCodePage(codePage, output)
}

func decodeCodePage(codePage uint32, output []byte) string {
	size, err := windows.MultiByteToWideChar(codePage, 0, &output[0], int32(len(output)), nil, 0)
	if err != nil || size == 0 {
		return strings.ToValidUTF8(string(output), "\uFFFD")
	}
	wide := make([]uint16, size)
	if _, err = windows.MultiByteToWideChar(codePage, 0, &output[0], int32(len(output)), &wide[0], size); err != nil {
		return strings.ToValidUTF8(string(output), "\uFFFD")
	}
	return windows.UTF16ToString(wide)
}

func detachedProcessAttributes() *syscall.SysProcAttr {
	return &syscall.SysProcAttr{CreationFlags: windows.CREATE_NEW_PROCESS_GROUP | windows.DETACHED_PROCESS}
}
