// Package locales contains source catalogs embedded in the dw binary.
package locales

import _ "embed"

// ActiveEnglishTOML is the active English catalog used by internal/l10n.
//
//go:embed active.en.toml
var ActiveEnglishTOML string
