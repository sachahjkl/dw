//go:build !linux && !windows

package webservice

import (
	"context"
	"fmt"
	"syscall"

	"github.com/sachahjkl/dw/internal/config"
)

type unsupportedNativeManager struct{}

func newNativeManager(config.PlatformBaseDirs) (NativeManager, error) {
	return unsupportedNativeManager{}, nil
}

func (unsupportedNativeManager) Registration() Registration { return RegistrationNone }
func (unsupportedNativeManager) Register(context.Context, string) error {
	return fmt.Errorf("web.registration-unsupported")
}
func (unsupportedNativeManager) Unregister(context.Context) error {
	return fmt.Errorf("web.registration-unsupported")
}

func (unsupportedNativeManager) Restart(context.Context) error {
	return fmt.Errorf("web.registration-unsupported")
}
func (unsupportedNativeManager) Start(context.Context) error {
	return fmt.Errorf("web.registration-unsupported")
}
func (unsupportedNativeManager) Stop(context.Context) error {
	return fmt.Errorf("web.registration-unsupported")
}
func (unsupportedNativeManager) Running(context.Context) (bool, error) {
	return false, fmt.Errorf("web.registration-unsupported")
}
func detachedProcessAttributes() *syscall.SysProcAttr { return nil }
