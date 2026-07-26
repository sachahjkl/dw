package controller

import (
	"strings"
	"testing"

	tea "charm.land/bubbletea/v2"
)

func TestSelectionModelUsesNavigationInsteadOfNumericInput(t *testing.T) {
	model := &selectionModel{
		label:    "Choose a provider",
		choices:  []terminalChoice{{label: "Azure DevOps"}, {label: "GitHub"}, {label: "Atlassian"}},
		selected: make(map[int]bool),
	}

	if command := model.handleKey("down"); command != nil {
		t.Fatal("moving the cursor should not quit")
	}
	if model.cursor != 1 {
		t.Fatalf("cursor = %d, want 1", model.cursor)
	}
	view := model.View().Content
	if !strings.Contains(view, "› GitHub") || strings.Contains(view, "1)") {
		t.Fatalf("selection view is not interactive:\n%s", view)
	}
	if command := model.handleKey("enter"); command == nil {
		t.Fatal("enter should finish the selection")
	}
	if view := strings.TrimSpace(model.View().Content); view != "Choose a provider: GitHub" {
		t.Fatalf("completed selection view = %q", view)
	}
}

func TestSelectionModelTogglesMultipleChoices(t *testing.T) {
	model := &selectionModel{
		label:    "Choose repositories",
		choices:  []terminalChoice{{label: "front"}, {label: "back"}},
		multi:    true,
		selected: make(map[int]bool),
	}

	model.handleKey("space")
	model.handleKey("down")
	model.handleKey("space")
	if !model.selected[0] || !model.selected[1] {
		t.Fatalf("selected = %#v", model.selected)
	}
	if command := model.handleKey("esc"); command == nil || !model.canceled {
		t.Fatal("escape should cancel the selection")
	}
}

var _ tea.Model = (*selectionModel)(nil)
