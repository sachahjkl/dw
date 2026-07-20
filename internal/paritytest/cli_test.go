package paritytest_test

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/cli/complete"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
)

func TestRootAndDeepHelpAreStableAndSorted(t *testing.T) {
	root := spec.Root(nil)
	rootHelp, problem := parse.Help(root, nil, "2026.07.17.3+26b737a")
	if problem != nil {
		t.Fatal(problem)
	}
	again, problem := parse.Help(root, nil, "2026.07.17.3+26b737a")
	if problem != nil || again != rootHelp {
		t.Fatalf("root help is not byte stable: %v", problem)
	}
	for _, required := range []string{
		"Dev Workflow 2026.07.17.3+26b737a\n\n",
		"Usage: dw",
		"Commands:\n",
		"  ado",
		"  completion",
		"  work",
		"Options:\n",
		"-h, --help",
		"-v, --verbose",
		"-V, --version",
	} {
		if !strings.Contains(rootHelp, required) {
			t.Errorf("root help lacks %q:\n%s", required, rootHelp)
		}
	}
	assertOrderedText(t, rootHelp, "  ado", "  agent", "  auth", "  completion", "  config", "  db", "  doctor", "  guide", "  init", "  refresh", "  secret", "  tui", "  upgrade", "  version", "  work")

	deepHelp, problem := parse.Help(root, []string{"work", "open"}, "ignored")
	if problem != nil {
		t.Fatal(problem)
	}
	for _, required := range []string{
		"Open or resume a task workspace with the configured agent.",
		"Usage: dw work open",
		"Arguments:\n",
		"WORK_ITEM",
		"Options:\n",
		"--workspace <WORKSPACE>",
		"--work-item <WORK_ITEM>",
	} {
		if !strings.Contains(deepHelp, required) {
			t.Errorf("deep help lacks %q:\n%s", required, deepHelp)
		}
	}
	assertOrderedText(t, deepHelp, "--agent", "--continue", "--verbose", "--help", "--json", "--pr", "--project", "--repo", "--root", "--version", "--work-item", "--workspace")
}

func assertOrderedText(t *testing.T, text string, values ...string) {
	t.Helper()
	position := -1
	for _, value := range values {
		next := strings.Index(text[position+1:], value)
		if next < 0 {
			t.Fatalf("%q is absent after byte %d in:\n%s", value, position, text)
		}
		position += next + 1
	}
}

func TestParserFailureClassesMatchCLIExitContract(t *testing.T) {
	root := spec.Root(nil)
	tests := []struct {
		name string
		args []string
		kind parse.ErrorKind
	}{
		{name: "root command required", kind: parse.MissingCommand},
		{name: "unknown root option", args: []string{"--definitely-invalid"}, kind: parse.UnknownOption},
		{name: "deep command required", args: []string{"work"}, kind: parse.MissingCommand},
		{name: "missing option value", args: []string{"work", "open", "--workspace"}, kind: parse.MissingValue},
		{name: "conflicting upgrade options", args: []string{"upgrade", "--check", "--rid", "linux-x64"}, kind: parse.Conflict},
		{name: "version rejects arguments", args: []string{"version", "unexpected"}, kind: parse.UnexpectedArgument},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			result, problem := parse.Parse(root, test.args)
			if result != nil || problem == nil {
				t.Fatalf("Parse() = result %#v, error %#v", result, problem)
			}
			if problem.Kind != test.kind || problem.ExitClass() != parse.ExitUsage {
				t.Fatalf("parse error = %#v, want kind %s and usage exit", problem, test.kind)
			}
			if strings.TrimSpace(problem.Error()) == "" {
				t.Fatal("parser failure has an empty diagnostic")
			}
		})
	}
}

func TestHelpAndVersionFlagsShortCircuitWithoutRunningActions(t *testing.T) {
	root := spec.Root(nil)
	for _, test := range []struct {
		args   []string
		intent parse.Intent
	}{
		{args: []string{"--help"}, intent: parse.IntentHelp},
		{args: []string{"-h"}, intent: parse.IntentHelp},
		{args: []string{"--version"}, intent: parse.IntentVersion},
		{args: []string{"-V"}, intent: parse.IntentVersion},
		{args: []string{"version"}, intent: parse.IntentRun},
		{args: []string{"work", "open", "--help"}, intent: parse.IntentHelp},
	} {
		result, problem := parse.Parse(root, test.args)
		if problem != nil {
			t.Fatalf("Parse(%q): %v", test.args, problem)
		}
		if result.Intent != test.intent {
			t.Fatalf("Parse(%q) intent = %v, want %v", test.args, result.Intent, test.intent)
		}
	}
	result, problem := parse.Parse(root, []string{"version"})
	if problem != nil {
		t.Fatal(problem)
	}
	if got, want := result.Path, []string{"version"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("explicit version path = %#v, want %#v", got, want)
	}
	if got, want := parse.Version("dw", "2026.07.17.3+26b737a"), "dw 2026.07.17.3+26b737a\n"; got != want {
		t.Fatalf("version flag output = %q, want %q", got, want)
	}
}

func TestCompletionRootOrderAndJSONWireFormat(t *testing.T) {
	items, err := complete.Complete(spec.Root(nil), nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	labels := make([]string, len(items))
	for index := range items {
		labels[index] = items[index].Label
		if strings.TrimSpace(items[index].Description) == "" {
			t.Errorf("completion %q has no description", items[index].Label)
		}
	}
	wantLabels := []string{"version", "guide", "doctor", "init", "refresh", "tui", "agent", "auth", "completion", "config", "ado", "db", "secret", "upgrade", "work"}
	if !reflect.DeepEqual(labels, wantLabels) {
		t.Fatalf("completion root labels = %#v, want %#v", labels, wantLabels)
	}
	encoded, err := json.Marshal(items[:1])
	if err != nil {
		t.Fatal(err)
	}
	if got, want := string(encoded), `[{"label":"version","description":"Show the CLI version"}]`; got != want {
		t.Fatalf("completion JSON = %s, want %s", got, want)
	}
}

func TestCompletionShellCandidatesIncludeElvish(t *testing.T) {
	items, err := complete.Complete(spec.Root(nil), []string{"completion", "install", ""}, nil)
	if err != nil {
		t.Fatal(err)
	}
	labels := make([]string, len(items))
	for index := range items {
		labels[index] = items[index].Label
	}
	want := []string{"bash", "fish", "zsh", "powershell", "elvish"}
	if !reflect.DeepEqual(labels, want) {
		t.Fatalf("shell completion labels = %#v, want %#v", labels, want)
	}
}

func TestCompletionWireFormatsKeepExactRowsAndNewline(t *testing.T) {
	items := []complete.Item{
		{Label: "alpha", Description: "First choice"},
		{Label: "beta", Description: ""},
	}
	tests := []struct {
		format complete.Format
		want   string
	}{
		{format: complete.FormatBash, want: "alpha\nbeta\n"},
		{format: complete.FormatFish, want: "alpha\tFirst choice\nbeta\n"},
		{format: complete.FormatZsh, want: "alpha\tFirst choice\nbeta\n"},
		{format: complete.FormatElvish, want: "alpha\tFirst choice\nbeta\n"},
		{format: complete.FormatJSON, want: "[{\"label\":\"alpha\",\"description\":\"First choice\"},{\"label\":\"beta\",\"description\":\"\"}]\n"},
		{format: complete.FormatPowerShell, want: "[{\"label\":\"alpha\",\"description\":\"First choice\"},{\"label\":\"beta\",\"description\":\"\"}]\n"},
	}
	for _, test := range tests {
		t.Run(string(test.format), func(t *testing.T) {
			got, err := complete.Render(test.format, items)
			if err != nil {
				t.Fatal(err)
			}
			if string(got) != test.want {
				t.Fatalf("Render(%s) = %q, want %q", test.format, got, test.want)
			}
		})
	}
}
