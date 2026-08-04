package tui

import (
	"testing"

	"github.com/sachahjkl/dw/internal/cockpit"
)

func TestWorkspaceFormDefaultsDoNotConflict(t *testing.T) {
	for _, templateID := range []string{"agent-open", "workspace-teardown", "workspace-add-item", "workspace-remove-item", "workspace-rename"} {
		t.Run(templateID, func(t *testing.T) {
			var fields []FormField
			for _, template := range formTemplates {
				if template.ID == templateID {
					fields = template.Fields(cockpit.Snapshot{
						Projects:   []string{"ha"},
						Workspaces: []cockpit.Workspace{{Path: "S:/dw/projects/ha/workspaces/task"}},
					})
					break
				}
			}
			values := make(map[string]string, len(fields))
			for _, field := range fields {
				values[field.ID] = field.Value
			}
			if values["workspace"] == "" || values["project"] != "" || values["continue"] != "false" {
				t.Fatalf("workspace defaults = %#v", values)
			}
		})
	}
}
