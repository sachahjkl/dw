package parse

import (
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/cli/spec"
)

func TestDiagnosticIncludesSuggestionsAndContextualHelp(t *testing.T) {
	root := spec.Root(nil)
	_, problem := Parse(root, []string{"provider", "auth", "login"})
	if problem == nil {
		t.Fatal("expected a missing provider error")
	}

	output := Diagnostic(root, problem,
		Suggestion{Value: "azure-devops"},
		Suggestion{Value: "github"},
		Suggestion{Value: "atlassian"},
	)
	for _, expected := range []string{
		`required argument "provider" was not provided`,
		"Suggestions:",
		"github",
		"Connect a provider account.",
		"Usage: dw provider auth login [OPTIONS] <PROVIDER>",
		"Arguments:",
	} {
		if !strings.Contains(output, expected) {
			t.Fatalf("diagnostic does not contain %q:\n%s", expected, output)
		}
	}
	if strings.Contains(output, "try '--help'") {
		t.Fatalf("diagnostic still defers guidance to another invocation:\n%s", output)
	}
}

func TestMissingCommandDiagnosticShowsAvailableCommands(t *testing.T) {
	root := spec.Root(nil)
	_, problem := Parse(root, []string{"provider", "auth"})
	if problem == nil {
		t.Fatal("expected a missing command error")
	}

	output := Diagnostic(root, problem)
	for _, expected := range []string{"Manage provider authentication.", "Commands:", "login", "logout", "status"} {
		if !strings.Contains(output, expected) {
			t.Fatalf("diagnostic does not contain %q:\n%s", expected, output)
		}
	}
}
