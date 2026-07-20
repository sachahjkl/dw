package dbcompat

import (
	"errors"

	"github.com/sachahjkl/dw/internal/l10n"
)

func localized(id l10n.ID, args ...l10n.Arg) error {
	return errors.New(l10n.Render(l10n.M(id, args...)))
}
