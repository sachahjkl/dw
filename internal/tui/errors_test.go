package tui

import (
	"errors"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/l10n"
)

type localizedTestError struct{}

func (localizedTestError) Error() string { return "internal.test-code" }
func (localizedTestError) Localized() l10n.Message {
	return l10n.M("execution.unclassified-error")
}

func TestModelLocalizesErrorsBeforeDisplay(t *testing.T) {
	model := NewModel(Dependencies{})
	text := model.errorText(localizedTestError{})
	if strings.Contains(text, "internal.test-code") || text != "The action failed." {
		t.Fatalf("localized error = %q", text)
	}
	if text := model.errorText(errors.New("plain failure")); text != "plain failure" {
		t.Fatalf("plain error = %q", text)
	}
}
