package controller

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
)

func TestSelectRepositoryPairsMapsExplicitLocalNames(t *testing.T) {
	configuredLocal := []string{"front", "api"}
	configuredProvider := []string{"platform/front", "api-service"}

	local, provider := selectRepositoryPairs(configuredLocal, configuredProvider, []string{"front", "unconfigured"})

	if want := []string{"front", "unconfigured"}; !reflect.DeepEqual(local, want) {
		t.Fatalf("local repositories = %#v, want %#v", local, want)
	}
	if want := []string{"platform/front", "unconfigured"}; !reflect.DeepEqual(provider, want) {
		t.Fatalf("provider repositories = %#v, want %#v", provider, want)
	}
}

func TestWorkContextAIDefaultsToCurrentWorkspace(t *testing.T) {
	root := t.TempDir()
	workspacePath := filepath.Join(root, "projects", "ha", "workspaces", "task")
	manifest := workspace.Manifest{Project: "ha", WorkItemID: "57143", WorkItems: []workspace.WorkItem{{ID: "57143"}, {ID: "57144"}}}
	if err := workspace.WriteManifest(filepath.Join(workspacePath, workspace.ManifestFile), manifest); err != nil {
		t.Fatal(err)
	}
	t.Chdir(workspacePath)
	invocation, problem := parse.Parse(spec.Root(nil), []string{"work", "context", "ai", "--root", root})
	if problem != nil {
		t.Fatal(problem)
	}
	request, err := buildWorkContextAI(invocation)
	if err != nil {
		t.Fatal(err)
	}
	contextRequest := request.(workapp.AIContextRequest).Request
	if contextRequest.Project != "ha" || !reflect.DeepEqual(contextRequest.IDs, []string{"57143", "57144"}) {
		t.Fatalf("context selection = %#v", contextRequest)
	}
}

func TestWorkContextAIWithoutIDOutsideWorkspaceFailsInBuilder(t *testing.T) {
	t.Chdir(t.TempDir())
	invocation, problem := parse.Parse(spec.Root(nil), []string{"work", "context", "ai"})
	if problem != nil {
		t.Fatalf("parser rejected contextual command: %v", problem)
	}
	if _, err := buildWorkContextAI(invocation); err == nil {
		t.Fatal("builder accepted a missing ID outside a workspace")
	}
}

func TestContextFallbackRejectsExplicitRootAndProjectMismatch(t *testing.T) {
	root := t.TempDir()
	otherRoot := t.TempDir()
	for _, configuredRoot := range []string{root, otherRoot} {
		path := filepath.Join(configuredRoot, "config", "workflow.json")
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(`{"schema":1}`), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	workspacePath := filepath.Join(root, "projects", "ha", "workspaces", "task")
	if err := workspace.WriteManifest(filepath.Join(workspacePath, workspace.ManifestFile), workspace.Manifest{Project: "ha", WorkItemID: "57143"}); err != nil {
		t.Fatal(err)
	}
	t.Chdir(workspacePath)
	for _, args := range [][]string{
		{"work", "context", "ai", "--root", otherRoot},
		{"work", "context", "ai", "--project", "other"},
	} {
		invocation, problem := parse.Parse(spec.Root(nil), args)
		if problem != nil {
			t.Fatal(problem)
		}
		if _, err := buildWorkContextAI(invocation); err == nil {
			t.Fatalf("mismatched context accepted: %v", args)
		}
	}
}

func TestWorkspaceSelectorsDoNotEnableAgentContinuation(t *testing.T) {
	for _, args := range [][]string{
		{"workspace", "open", "57143"},
		{"workspace", "open", "--project", "ha"},
		{"workspace", "open", "--project", "ha", "--continue"},
	} {
		invocation, problem := parse.Parse(spec.Root(nil), args)
		if problem != nil {
			t.Fatal(problem)
		}
		request, err := buildWorkspaceOpen(invocation)
		if err != nil {
			t.Fatal(err)
		}
		open := request.(workapp.OpenRequest)
		want := args[len(args)-1] == "--continue"
		if open.Continue != want {
			t.Fatalf("%v continuation = %t, want %t", args, open.Continue, want)
		}
	}
}

func TestCommandValidationRejectsUnsafeAndConflictingOptions(t *testing.T) {
	tests := [][]string{
		{"work", "item", "doing", "57143", "--json"},
		{"work", "item", "state", "set", "57143", "--state", "Active", "--json"},
		{"workspace", "teardown", "--workspace", "task", "--project", "ha"},
		{"workspace", "item", "remove", "57143", "--workspace", "task", "--continue"},
		{"work", "item", "child", "create", "--repo", "api", "--title", "Child", "--workspace", "task", "--project", "ha"},
	}
	for _, args := range tests {
		if _, problem := parse.Parse(spec.Root(nil), args); problem == nil {
			t.Fatalf("invalid command was accepted: %v", args)
		}
	}
}

func TestDataReadJSONUsesMachineMode(t *testing.T) {
	if !routeUsesJSONOption("data.read") {
		t.Fatal("data.read --json is not registered as machine mode")
	}
}

func TestRootAwareUtilityCommandsAcceptRootOption(t *testing.T) {
	root := t.TempDir()
	for _, args := range [][]string{{"doctor", "--root", root}, {"upgrade", "--root", root, "--check"}} {
		if _, problem := parse.Parse(spec.Root(nil), args); problem != nil {
			t.Fatalf("root-aware command %v failed: %v", args, problem)
		}
	}
}

func TestSelectRepositoryPairsKeepsConfiguredDefaults(t *testing.T) {
	configuredLocal := []string{"front"}
	configuredProvider := []string{"platform/front"}

	local, provider := selectRepositoryPairs(configuredLocal, configuredProvider, nil)

	if !reflect.DeepEqual(local, configuredLocal) || !reflect.DeepEqual(provider, configuredProvider) {
		t.Fatalf("default pairs = %#v/%#v, want %#v/%#v", local, provider, configuredLocal, configuredProvider)
	}
}
