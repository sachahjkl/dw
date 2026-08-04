package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work/ado"
)

func TestFindRootUsesNearestWorkflowConfiguration(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "config", "workflow.json")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(`{"schema":1}`), 0o644); err != nil {
		t.Fatal(err)
	}
	child := filepath.Join(root, "projects", "ha", "workspaces", "task", "api")
	if found, ok := FindRoot(child); !ok || found != NormalizePathLossy(root) {
		t.Fatalf("found root = %q, %t", found, ok)
	}
}

func TestProviderMergeMatchesWorkItemTypesCaseInsensitively(t *testing.T) {
	base, err := wirejson.Parse([]byte(`{"contentFields":{"workItemTypes":{"Bug":{"description":"Custom.Global","acceptanceCriteria":"Custom.Done"}}}}`))
	if err != nil {
		t.Fatal(err)
	}
	override, err := wirejson.Parse([]byte(`{"contentFields":{"workItemTypes":{"bug":{"description":"Custom.Project"}}}}`))
	if err != nil {
		t.Fatal(err)
	}
	raw, err := wirejson.Compact(mergeProviderValues(base, override))
	if err != nil {
		t.Fatal(err)
	}
	var options ado.Options
	if err = json.Unmarshal(raw, &options); err != nil {
		t.Fatal(err)
	}
	if len(options.ContentFields.WorkItemTypes) != 1 {
		t.Fatalf("work item types = %#v", options.ContentFields.WorkItemTypes)
	}
	mapping := options.ContentFields.WorkItemTypes["Bug"]
	if mapping.Description != "Custom.Project" || mapping.AcceptanceCriteria != "Custom.Done" {
		t.Fatalf("bug mapping = %#v", mapping)
	}
}
