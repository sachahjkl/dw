package webservice

import (
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/console"
)

func TestAuthenticationErrorsRenderHumanMessages(t *testing.T) {
	err := localizedError("web.error.authentication-option-unavailable")
	text := console.LocalizedErrorText(console.NewEnglishLocalizer(), err)
	if strings.Contains(text, "web.error.") || !strings.Contains(text, "authentication option") {
		t.Fatalf("localized error = %q", text)
	}
}
