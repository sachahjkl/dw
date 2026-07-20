//go:build windows

package secret

import "github.com/sachahjkl/dw/internal/l10n"

const windowsCredentialMaxUTF16Units = 1279

func validatePlatformValue(value string) error {
	units := 0
	for _, character := range value {
		if character > 0xffff {
			units += 2
		} else {
			units++
		}
		if units > windowsCredentialMaxUTF16Units {
			return newLocalizedError("secret.windows-platform-limit", l10n.M("secret.windows-platform-limit"), nil)
		}
	}
	return nil
}
