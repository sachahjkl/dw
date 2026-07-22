package console

import (
	"testing"

	"github.com/sachahjkl/dw/internal/l10n"
)

func TestProviderResultMessagesAreRegistered(t *testing.T) {
	localizer := NewEnglishLocalizer()
	for _, id := range []l10n.ID{"result.provider", "result.kinds", "result.capabilities"} {
		if text := localizer.Text(id); text == "" {
			t.Fatalf("message %q is empty", id)
		}
	}
}
