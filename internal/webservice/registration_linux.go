//go:build linux

package webservice

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"

	"github.com/sachahjkl/dw/internal/config"
)

const systemdUnitName = "dw-web.service"

type systemdManager struct{ unitFile string }

func newNativeManager(dirs config.PlatformBaseDirs) (NativeManager, error) {
	base := dirs.ConfigDir
	if base == "" {
		base = filepath.Join(dirs.HomeDir, ".config")
	}
	return &systemdManager{unitFile: filepath.Join(base, "systemd", "user", systemdUnitName)}, nil
}

func (manager *systemdManager) Registration() Registration { return RegistrationSystemdUser }

func (manager *systemdManager) Register(ctx context.Context, executable string) error {
	content := []byte("[Unit]\nDescription=Dev Workflow web service\n\n[Service]\nType=simple\nExecStart=" + strconv.Quote(executable) + " web serve\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n")
	if err := writeAtomic(manager.unitFile, content); err != nil {
		return err
	}
	if err := systemctl(ctx, "daemon-reload"); err != nil {
		_ = os.Remove(manager.unitFile)
		return err
	}
	if err := systemctl(ctx, "enable", "--now", systemdUnitName); err != nil {
		_ = os.Remove(manager.unitFile)
		_ = systemctl(ctx, "daemon-reload")
		return err
	}
	return nil
}

func (manager *systemdManager) Unregister(ctx context.Context) error {
	_ = systemctl(ctx, "stop", systemdUnitName)
	if err := systemctl(ctx, "disable", systemdUnitName); err != nil {
		return err
	}
	if err := os.Remove(manager.unitFile); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return systemctl(ctx, "daemon-reload")
}

func (manager *systemdManager) Start(ctx context.Context) error {
	return systemctl(ctx, "start", systemdUnitName)
}

func (manager *systemdManager) Restart(ctx context.Context) error {
	return systemctl(ctx, "restart", systemdUnitName)
}

func (manager *systemdManager) Stop(ctx context.Context) error {
	return systemctl(ctx, "stop", systemdUnitName)
}

func (manager *systemdManager) Running(ctx context.Context) (bool, error) {
	command := exec.CommandContext(ctx, "systemctl", "--user", "is-active", "--quiet", systemdUnitName)
	output, err := command.CombinedOutput()
	if err == nil {
		return true, nil
	}
	var exit *exec.ExitError
	if errors.As(err, &exit) && exit.ExitCode() == 3 {
		return false, nil
	}
	return false, systemctlError(output, err)
}

func systemctl(ctx context.Context, arguments ...string) error {
	command := exec.CommandContext(ctx, "systemctl", append([]string{"--user"}, arguments...)...)
	output, err := command.CombinedOutput()
	if err != nil {
		return systemctlError(output, err)
	}
	return nil
}

func systemctlError(output []byte, err error) error {
	text := strings.ToLower(string(output))
	if errors.Is(err, exec.ErrNotFound) || strings.Contains(text, "failed to connect to bus") || strings.Contains(text, "failed to connect to user scope bus") || strings.Contains(text, "no medium found") {
		return fmt.Errorf("web.service-manager-unavailable:%w", err)
	}
	return fmt.Errorf("web.systemctl:%s:%w", string(output), err)
}

func detachedProcessAttributes() *syscall.SysProcAttr { return nil }
