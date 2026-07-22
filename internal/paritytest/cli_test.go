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

func TestGreenfieldCommandTreeHasExactNamespaceLeavesAndActionKeys(t *testing.T) {
	root := spec.Root(nil)
	expected := map[string][]string{
		"work": {
			"changelog", "context ai", "context show", "item child create", "item doing",
			"item list", "item show", "item state set", "pr list",
		},
		"workspace": {
			"commit", "current", "finish", "handoff validate", "item add", "item remove", "list",
			"open", "pr start", "preflight", "prune", "rename", "repo add", "repo latest", "start",
			"status", "sync", "teardown",
		},
		"data": {
			"catalog", "describe", "guard", "query", "read", "source collect", "source list",
		},
		"provider": {
			"auth login", "auth logout", "auth status", "capabilities", "list", "show",
		},
	}
	for namespace, want := range expected {
		command, ok := spec.Lookup(root, []string{namespace})
		if !ok {
			t.Fatalf("missing namespace %q", namespace)
		}
		got := leafPaths(command, nil)
		if !reflect.DeepEqual(got, want) {
			t.Errorf("%s leaf paths = %#v, want %#v", namespace, got, want)
		}
		for _, relative := range got {
			path := append([]string{namespace}, strings.Fields(relative)...)
			leaf, found := spec.Lookup(root, path)
			if !found {
				t.Fatalf("leaf disappeared at %q", path)
			}
			if wantKey := strings.Join(path, "."); leaf.Key != wantKey {
				t.Errorf("%s action key = %q, want %q", strings.Join(path, " "), leaf.Key, wantKey)
			}
			if len(leaf.Aliases) != 0 {
				t.Errorf("%s has compatibility aliases %#v", strings.Join(path, " "), leaf.Aliases)
			}
		}
	}
}

func leafPaths(command *spec.Command, prefix []string) []string {
	if len(command.Children) == 0 {
		return []string{strings.Join(prefix, " ")}
	}
	paths := make([]string, 0)
	for _, child := range command.VisibleChildren() {
		paths = append(paths, leafPaths(child, append(prefix, child.Name))...)
	}
	return paths
}

func TestRootAndDeepHelpAreStableAndProviderNeutral(t *testing.T) {
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
		"  data",
		"  provider",
		"  work",
		"  workspace",
		"Options:\n",
		"-h, --help",
		"-v, --verbose",
		"-V, --version",
	} {
		if !strings.Contains(rootHelp, required) {
			t.Errorf("root help lacks %q:\n%s", required, rootHelp)
		}
	}
	for _, removed := range []string{"  ado", "  auth", "  db"} {
		if strings.Contains(rootHelp, removed) {
			t.Errorf("root help advertises removed namespace %q:\n%s", removed, rootHelp)
		}
	}
	assertOrderedText(t, rootHelp, "  agent", "  completion", "  config", "  data", "  doctor", "  guide", "  init", "  provider", "  refresh", "  secret", "  tui", "  upgrade", "  version", "  work", "  workspace")

	deepHelp, problem := parse.Help(root, []string{"workspace", "open"}, "ignored")
	if problem != nil {
		t.Fatal(problem)
	}
	for _, required := range []string{
		"Usage: dw workspace open",
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

func TestParserFailureClassesAndCleanCutover(t *testing.T) {
	root := spec.Root(nil)
	tests := []struct {
		name string
		args []string
		kind parse.ErrorKind
	}{
		{name: "root command required", kind: parse.MissingCommand},
		{name: "unknown root option", args: []string{"--definitely-invalid"}, kind: parse.UnknownOption},
		{name: "deep command required", args: []string{"work"}, kind: parse.MissingCommand},
		{name: "missing option value", args: []string{"workspace", "open", "--workspace"}, kind: parse.MissingValue},
		{name: "conflicting upgrade options", args: []string{"upgrade", "--check", "--rid", "linux-x64"}, kind: parse.Conflict},
		{name: "version rejects arguments", args: []string{"version", "unexpected"}, kind: parse.UnexpectedArgument},
		{name: "old ado namespace is unknown", args: []string{"ado"}, kind: parse.UnknownCommand},
		{name: "old db namespace is unknown", args: []string{"db"}, kind: parse.UnknownCommand},
		{name: "old top-level auth is unknown", args: []string{"auth"}, kind: parse.UnknownCommand},
		{name: "local lifecycle under work is unknown", args: []string{"work", "start"}, kind: parse.UnknownCommand},
		{name: "local nested lifecycle under work is unknown", args: []string{"work", "repo", "add"}, kind: parse.UnknownCommand},
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

func TestGenericCommandsExposeProviderSelection(t *testing.T) {
	root := spec.Root(nil)
	for _, relative := range leafPaths(mustCommand(t, root, "work"), nil) {
		path := append([]string{"work"}, strings.Fields(relative)...)
		assertOption(t, mustCommand(t, root, path...), "provider")
	}
	for _, path := range [][]string{
		{"data", "source", "list"},
		{"data", "source", "collect"},
		{"data", "guard"},
		{"data", "catalog"},
		{"data", "describe"},
		{"data", "query"},
	} {
		assertOption(t, mustCommand(t, root, path...), "provider")
	}
	for _, path := range [][]string{{"data", "catalog"}, {"data", "describe"}, {"data", "query"}} {
		assertOption(t, mustCommand(t, root, path...), "source")
	}
	assertOption(t, mustCommand(t, root, "data", "guard"), "query")
	assertOption(t, mustCommand(t, root, "data", "query"), "query")
	if positionals := mustCommand(t, root, "data", "describe").Positionals(); len(positionals) != 1 || positionals[0].ValueName != "RESOURCE" {
		t.Fatalf("data describe positionals = %#v, want optional RESOURCE", positionals)
	}
	for _, relative := range leafPaths(mustCommand(t, root, "data"), nil) {
		path := append([]string{"data"}, strings.Fields(relative)...)
		for _, option := range mustCommand(t, root, path...).Options(true) {
			if option.Long == "database" || option.Long == "sql" {
				t.Errorf("%s exposes removed --%s option", strings.Join(path, " "), option.Long)
			}
		}
	}
	for _, path := range [][]string{
		{"workspace", "open"},
		{"workspace", "start"},
		{"workspace", "pr", "start"},
		{"workspace", "sync"},
		{"workspace", "item", "add"},
		{"workspace", "finish"},
		{"workspace", "prune"},
	} {
		assertOption(t, mustCommand(t, root, path...), "provider")
	}
	for _, path := range [][]string{{"workspace", "start"}, {"workspace", "item", "add"}, {"workspace", "finish"}} {
		command := mustCommand(t, root, path...)
		assertOption(t, command, "skip-provider")
		for _, option := range command.Options(true) {
			if option.Long == "skip-ado" {
				t.Errorf("%s exposes removed provider-specific --skip-ado", command.Key)
			}
		}
	}

	for _, operation := range []string{"login", "status", "logout"} {
		result, problem := parse.Parse(root, []string{"provider", "auth", operation, "azure-devops"})
		if problem != nil {
			t.Fatalf("provider auth %s positional selection: %v", operation, problem)
		}
		if got := result.Values.String("provider"); got != "azure-devops" {
			t.Errorf("provider auth %s selected %q, want azure-devops", operation, got)
		}
	}
}

func mustCommand(t *testing.T, root *spec.Command, path ...string) *spec.Command {
	t.Helper()
	command, ok := spec.Lookup(root, path)
	if !ok {
		t.Fatalf("missing command %q", path)
	}
	return command
}

func assertOption(t *testing.T, command *spec.Command, name string) {
	t.Helper()
	for _, option := range command.Options(true) {
		if option.Long == name {
			return
		}
	}
	t.Errorf("%s does not expose --%s", command.Key, name)
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
		{args: []string{"workspace", "open", "--help"}, intent: parse.IntentHelp},
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
	wantLabels := []string{"version", "guide", "doctor", "init", "refresh", "tui", "agent", "completion", "config", "work", "workspace", "data", "provider", "secret", "upgrade"}
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

func TestCompletionAdvertisesExactGenericNamespaces(t *testing.T) {
	root := spec.Root(nil)
	tests := []struct {
		namespace string
		want      []string
	}{
		{namespace: "work", want: []string{"changelog", "context", "item", "pr"}},
		{namespace: "workspace", want: []string{"commit", "current", "finish", "handoff", "item", "list", "open", "pr", "preflight", "prune", "rename", "repo", "start", "status", "sync", "teardown"}},
		{namespace: "data", want: []string{"catalog", "describe", "guard", "query", "read", "source"}},
		{namespace: "provider", want: []string{"auth", "capabilities", "list", "show"}},
	}
	for _, test := range tests {
		t.Run(test.namespace, func(t *testing.T) {
			items, err := complete.Complete(root, []string{test.namespace, ""}, nil)
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
			if !reflect.DeepEqual(labels, test.want) {
				t.Fatalf("%s completion labels = %#v, want %#v", test.namespace, labels, test.want)
			}
		})
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
