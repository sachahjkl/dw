package workspace

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPreflightAcceptsRequiredParentWithChild(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "User Story", []string{"9001"}, false)
	task := "Task"
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{" user story "}, func(string, []string) ([]WorkItem, error) {
		return []WorkItem{{ID: "9001", Type: &task}}, nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if report.HasBlockingIssues {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestPreflightBlocksRequiredParentWithoutChild(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "Anomalie", nil, true)
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{"ANOMALIE"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !report.HasBlockingIssues || len(report.Issues) != 1 || report.Issues[0].Code != "work-item.child-task.missing" {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestPreflightRejectsNonTaskChild(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "User Story", []string{"9001"}, false)
	bug := "Bug"
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{"User Story"}, func(string, []string) ([]WorkItem, error) {
		return []WorkItem{{ID: "9001", Type: &bug}}, nil
	})
	if err != nil {
		t.Fatal(err)
	}
	if !report.HasBlockingIssues || len(report.Issues) != 1 || report.Issues[0].Code != "work-item.child-task.missing" {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestPreflightBlocksConfiguredTypeWithStaleUnconfiguredContext(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "User Story", nil, false)
	replacePreflightContextType(t, contextPath, "User Story", "Bug")
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{"User Story"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !report.HasBlockingIssues || len(report.Issues) != 1 || report.Issues[0].Code != "workspace.provider-context.stale" {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestPreflightBlocksUnconfiguredTypeWithStaleConfiguredContext(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "Bug", nil, false)
	replacePreflightContextType(t, contextPath, "Bug", "User Story")
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{"User Story"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !report.HasBlockingIssues || len(report.Issues) != 1 || report.Issues[0].Code != "workspace.provider-context.stale" {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestPreflightBlocksConfiguredParentWithoutProviderContext(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "User Story", nil, true)
	if err := os.WriteFile(contextPath, []byte(`[]`), 0o644); err != nil {
		t.Fatal(err)
	}
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{"User Story"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !report.HasBlockingIssues || len(report.Issues) != 1 || report.Issues[0].Code != "workspace.provider-context.missing" {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestPreflightIgnoresUnconfiguredParentType(t *testing.T) {
	workspace, contextPath := writePreflightFixture(t, "Bug", nil, false)
	report, err := BuildPreflight(workspace, []string{contextPath}, []string{"User Story", "Anomalie"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if report.HasBlockingIssues {
		t.Fatalf("issues = %#v", report.Issues)
	}
}

func TestApplySnapshotsRefreshesWorkspaceInstructions(t *testing.T) {
	workspace := t.TempDir()
	kind, oldTitle, newTitle := "User Story", "Old title", "New title"
	manifest := Manifest{Project: "he", WorkItemID: "42", WorkItemType: &kind, WorkItemTitle: &oldTitle}
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		t.Fatal(err)
	}
	if _, err := ApplySnapshots(workspace, []WorkItem{{ID: "42", Type: &kind, Title: &newTitle}}); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(workspace, "AGENTS.md"))
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, expected := range []string{newTitle, "requireChildTaskForWorkItemTypes", "refresh provider context"} {
		if !strings.Contains(text, expected) {
			t.Fatalf("workspace instructions do not contain %q:\n%s", expected, text)
		}
	}
}

func TestDecodeAIContextsRejectsMissingWorkItem(t *testing.T) {
	for _, data := range []string{"null", "[null]", `{"links":{}}`} {
		if _, err := decodeAIContexts([]byte(data)); err == nil {
			t.Fatalf("accepted invalid context %s", data)
		}
	}
}

func TestDiscoverAIContextFilesPrefersCanonicalProviderContext(t *testing.T) {
	workspace := t.TempDir()
	canonical, err := WriteProviderContext(workspace, []any{map[string]any{"workItem": map[string]any{"id": "42"}}})
	if err != nil {
		t.Fatal(err)
	}
	legacy := filepath.Join(workspace, "ai-context-42.json")
	if err := os.WriteFile(legacy, []byte(`[]`), 0o644); err != nil {
		t.Fatal(err)
	}
	files := DiscoverAIContextFiles(workspace)
	if len(files) != 2 || files[0] != canonical || files[1] != legacy {
		t.Fatalf("context files = %#v", files)
	}
}

func writePreflightFixture(t *testing.T, kind string, childIDs []string, array bool) (string, string) {
	t.Helper()
	workspace := t.TempDir()
	title, state := "Title", "Active"
	manifest := Manifest{Project: "he", WorkItemID: "42", WorkItems: []WorkItem{{ID: "42", Type: &kind, Title: &title, State: &state}}}
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		t.Fatal(err)
	}
	contextItem := map[string]any{
		"workItem":    map[string]any{"id": "42", "type": kind, "title": title, "state": state},
		"links":       map[string]any{"childIds": childIDs},
		"attachments": map[string]any{"directoryHint": "attachments", "items": []any{}},
	}
	var value any = contextItem
	if array {
		value = []any{contextItem}
	}
	data, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(workspace, "ai-context-42.json")
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatal(err)
	}
	return workspace, path
}

func replacePreflightContextType(t *testing.T, path, oldType, newType string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	updated := strings.Replace(string(data), `"type":"`+oldType+`"`, `"type":"`+newType+`"`, 1)
	if updated == string(data) {
		t.Fatalf("context type %q was not found", oldType)
	}
	if err := os.WriteFile(path, []byte(updated), 0o644); err != nil {
		t.Fatal(err)
	}
}
