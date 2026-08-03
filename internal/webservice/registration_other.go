//go:build !linux && !windows

package webservice

import (
	"context"
	"syscall"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/l10n"
)

type unsupportedNativeManager struct{}

func newNativeManager(config.PlatformBaseDirs) (NativeManager, error) {
	return unsupportedNativeManager{}, nil
}

func (unsupportedNativeManager) Registration() Registration { return RegistrationNone }
func (unsupportedNativeManager) Register(context.Context, string) error {
	return l10n.NewError("web.error.registration-unsupported")
}
func (unsupportedNativeManager) Unregister(context.Context) error {
	return l10n.NewError("web.error.registration-unsupported")
}

func (unsupportedNativeManager) Restart(context.Context) error {
	return l10n.NewError("web.error.registration-unsupported")
}
func (unsupportedNativeManager) Start(context.Context) error {
	return l10n.NewError("web.error.registration-unsupported")
}
func (unsupportedNativeManager) Stop(context.Context) error {
	return l10n.NewError("web.error.registration-unsupported")
}
func (unsupportedNativeManager) Running(context.Context) (bool, error) {
	return false, l10n.NewError("web.error.registration-unsupported")
}
func detachedProcessAttributes() *syscall.SysProcAttr { return nil }
