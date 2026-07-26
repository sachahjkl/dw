package dataapp

import (
	"errors"
	"os"

	"github.com/sachahjkl/dw/internal/l10n"
)

func localized(id l10n.ID, args ...l10n.Arg) error {
	return errors.New(l10n.Render(l10n.M(id, args...)))
}

func configReadError(path string, err error) error {
	if errors.Is(err, os.ErrNotExist) {
		return localized("data.error.config_missing", l10n.A("path", path))
	}
	return localized("data.error.config_read", l10n.A("path", path), l10n.A("error", err))
}
