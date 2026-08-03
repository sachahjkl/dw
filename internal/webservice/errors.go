package webservice

import "github.com/sachahjkl/dw/internal/l10n"

func localizedError(id l10n.ID, args ...l10n.Arg) error {
	return l10n.NewError(id, args...)
}
