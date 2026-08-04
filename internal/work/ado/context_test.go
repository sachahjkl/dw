package ado

import (
	"encoding/json"
	"testing"
)

func TestAIContextContentFieldsUseDefaultsByWorkItemType(t *testing.T) {
	tests := []struct {
		name        string
		itemType    string
		description string
	}{
		{name: "user story description", itemType: "User Story", description: "Story description"},
		{name: "bug repro steps", itemType: "Bug", description: "Bug reproduction"},
		{name: "case insensitive bug", itemType: "bUG", description: "Bug reproduction"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			root := contextFixture(test.itemType)
			result := mapAIContext(root, Options{}, false, nil)
			if result.Content.Description == nil || *result.Content.Description != test.description {
				t.Fatalf("description = %#v, want %q", result.Content.Description, test.description)
			}
			if result.Content.AcceptanceCriteria == nil || *result.Content.AcceptanceCriteria != "Acceptance criteria" {
				t.Fatalf("acceptance criteria = %#v", result.Content.AcceptanceCriteria)
			}
		})
	}
}

func TestAIContextContentFieldsSupportPartialTypeOverrides(t *testing.T) {
	root := contextFixture("Incident")
	root["fields"].(map[string]any)["Custom.IncidentDetails"] = "<p>Incident details</p>"
	options := Options{ContentFields: ContentFields{WorkItemTypes: map[string]ContentFieldMapping{
		"incident": {Description: "Custom.IncidentDetails"},
	}}}
	result := mapAIContext(root, options, false, nil)
	if result.Content.Description == nil || *result.Content.Description != "Incident details" {
		t.Fatalf("description = %#v", result.Content.Description)
	}
	if result.Content.AcceptanceCriteria == nil || *result.Content.AcceptanceCriteria != "Acceptance criteria" {
		t.Fatalf("acceptance criteria = %#v", result.Content.AcceptanceCriteria)
	}
}

func TestContentFieldsRejectInvalidReferencesAndDuplicateTypes(t *testing.T) {
	for _, input := range []string{
		`{"contentFields":{"description":""}}`,
		`{"contentFields":{"workItemTypes":{"Bug":{"description":""}}}}`,
		`{"contentFields":{"workItemTypes":{"Bug":{"description":"Custom.One"},"bug":{"description":"Custom.Two"}}}}`,
		`{"contentFields":{"workItemTypes":{" ":{"description":"Custom.One"}}}}`,
	} {
		var options Options
		if err := json.Unmarshal([]byte(input), &options); err == nil {
			t.Fatalf("invalid options accepted: %s", input)
		}
	}
}

func TestResolveOptionsMergesContentFieldOverrides(t *testing.T) {
	workflow := Options{ContentFields: ContentFields{
		ContentFieldMapping: ContentFieldMapping{Description: "Custom.GlobalDescription"},
		WorkItemTypes: map[string]ContentFieldMapping{
			"Bug": {Description: "Custom.GlobalBug", AcceptanceCriteria: "Custom.GlobalDone"},
		},
	}}
	project := Options{ContentFields: ContentFields{WorkItemTypes: map[string]ContentFieldMapping{
		"bug":      {Description: "Custom.ProjectBug"},
		"Incident": {Description: "Custom.Incident"},
	}}}
	resolved, err := ResolveOptions(&workflow, &project)
	if err != nil {
		t.Fatal(err)
	}
	bug := contentFieldMapping(resolved, "BUG")
	if bug.Description != "Custom.ProjectBug" || bug.AcceptanceCriteria != "Custom.GlobalDone" {
		t.Fatalf("bug mapping = %#v", bug)
	}
	incident := contentFieldMapping(resolved, "Incident")
	if incident.Description != "Custom.Incident" || incident.AcceptanceCriteria != "Microsoft.VSTS.Common.AcceptanceCriteria" {
		t.Fatalf("incident mapping = %#v", incident)
	}
}

func contextFixture(itemType string) map[string]any {
	return map[string]any{
		"id": float64(42),
		"fields": map[string]any{
			"System.WorkItemType":                      itemType,
			"System.Description":                       "<p>Story description</p>",
			"Microsoft.VSTS.TCM.ReproSteps":            "<p>Bug reproduction</p>",
			"Microsoft.VSTS.Common.AcceptanceCriteria": "<p>Acceptance criteria</p>",
		},
	}
}
