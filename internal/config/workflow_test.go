package config

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestWorkflowPreflightRuleLoadsAndMarshals(t *testing.T) {
	root := t.TempDir()
	configDirectory := filepath.Join(root, "config")
	if err := os.MkdirAll(configDirectory, 0o755); err != nil {
		t.Fatal(err)
	}
	data := []byte(`{"schema":1,"providers":{},"branchPrefixes":{},"preflight":{"requireChildTaskForWorkItemTypes":["User Story","Anomalie"]}}`)
	if err := os.WriteFile(filepath.Join(configDirectory, "workflow.json"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	loaded, err := LoadWorkflowConfigChecked(root)
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"User Story", "Anomalie"}
	if loaded.Preflight == nil || !reflect.DeepEqual(loaded.Preflight.RequireChildTaskForWorkItemTypes, want) {
		t.Fatalf("preflight = %#v", loaded.Preflight)
	}
	encoded, err := json.Marshal(WorkflowConfig{Schema: 1, Providers: []ProviderConfiguration{}, BranchPrefixes: []NamedString{}, Preflight: &PreflightOptions{RequireChildTaskForWorkItemTypes: want}})
	if err != nil {
		t.Fatal(err)
	}
	var roundTrip map[string]json.RawMessage
	if err := json.Unmarshal(encoded, &roundTrip); err != nil {
		t.Fatal(err)
	}
	var preflight PreflightOptions
	if err := json.Unmarshal(roundTrip["preflight"], &preflight); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(preflight.RequireChildTaskForWorkItemTypes, want) {
		t.Fatalf("round-trip preflight = %#v", preflight)
	}
}

func TestWorkflowPreflightRuleIsOptional(t *testing.T) {
	encoded, err := json.Marshal(WorkflowConfig{Schema: 1, Providers: []ProviderConfiguration{}, BranchPrefixes: []NamedString{}})
	if err != nil {
		t.Fatal(err)
	}
	var decoded map[string]any
	if err := json.Unmarshal(encoded, &decoded); err != nil {
		t.Fatal(err)
	}
	if _, exists := decoded["preflight"]; exists {
		t.Fatalf("optional preflight was encoded: %s", encoded)
	}
}
