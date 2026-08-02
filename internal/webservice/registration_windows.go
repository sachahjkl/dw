//go:build windows

package webservice

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"syscall"

	"github.com/sachahjkl/dw/internal/config"
	"golang.org/x/sys/windows"
)

const scheduledTaskName = `\DevWorkflow\dw-web`

type taskSchedulerManager struct{}

func newNativeManager(config.PlatformBaseDirs) (NativeManager, error) {
	return &taskSchedulerManager{}, nil
}

func (manager *taskSchedulerManager) Registration() Registration { return RegistrationTaskScheduler }
func (manager *taskSchedulerManager) Register(ctx context.Context, executable string) error {
	command := `"` + strings.ReplaceAll(executable, `"`, `\"`) + `" web serve`
	if err := schtasks(ctx, "/Create", "/TN", scheduledTaskName, "/SC", "ONLOGON", "/TR", command, "/RL", "LIMITED", "/F"); err != nil {
		return err
	}
	if err := manager.Start(ctx); err != nil {
		_ = schtasks(ctx, "/Delete", "/TN", scheduledTaskName, "/F")
		return err
	}
	return nil
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
func (manager *taskSchedulerManager) Running(ctx context.Context) (bool, error) {
	err := schtasks(ctx, "/Query", "/TN", scheduledTaskName)
	if err == nil {
		return true, nil
	}
	var exit *exec.ExitError
	if errors.As(err, &exit) && exit.ExitCode() == 1 {
		return false, nil
	}
	return false, err
}

func schtasks(ctx context.Context, arguments ...string) error {
	command := exec.CommandContext(ctx, "schtasks.exe", arguments...)
	output, err := command.CombinedOutput()
	if err != nil {
		return fmt.Errorf("web.schtasks:%s:%w", string(output), err)
	}
	return nil
}

func detachedProcessAttributes() *syscall.SysProcAttr {
	return &syscall.SysProcAttr{CreationFlags: windows.CREATE_NEW_PROCESS_GROUP | windows.DETACHED_PROCESS}
}
