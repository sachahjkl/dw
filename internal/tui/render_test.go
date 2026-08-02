package tui

import (
	"fmt"
	"strings"
	"testing"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cockpit"
)

func TestPanelHonorsRequestedDimensions(t *testing.T) {
	model := NewModel(Dependencies{})
	got := model.panel("Title", "body", 40, 8)
	if width := lipgloss.Width(got); width != 40 {
		t.Fatalf("panel width = %d, want 40", width)
	}
	if height := lipgloss.Height(got); height != 8 {
		t.Fatalf("panel height = %d, want 8", height)
	}
}

func TestResponsiveTableUsesHeadersAndCompactCards(t *testing.T) {
	model := NewModel(Dependencies{})
	columns := []tableColumn{
		{Header: "tui.column.project", MinWidth: 10, MaxWidth: 20, Priority: 0},
		{Header: "tui.column.title", MinWidth: 16, MaxWidth: 40, Priority: 1},
	}
	rows := [][]string{{"alpha", "A deliberately long title that must not wrap into an unlabeled row"}}

	wide := model.renderTable(columns, rows, 0, 72, 8)
	if !strings.Contains(wide, "Project") || !strings.Contains(wide, "Title") {
		t.Fatalf("wide table lacks headers:\n%s", wide)
	}
	if width := lipgloss.Width(wide); width > 72 {
		t.Fatalf("wide table width = %d, exceeds 72", width)
	}

	compact := model.renderTable(columns, rows, 0, 40, 8)
	if !strings.Contains(compact, "Project: alpha") || !strings.Contains(compact, "Title:") {
		t.Fatalf("compact table is not a labeled card:\n%s", compact)
	}
	if width := lipgloss.Width(compact); width > 40 {
		t.Fatalf("compact table width = %d, exceeds 40", width)
	}
}

func TestRenderProjectTabsKeepsSelectedProjectVisible(t *testing.T) {
	model := NewModelWithSnapshot(Dependencies{}, cockpit.Snapshot{WorkProjects: []cockpit.WorkProject{
		{Key: "first-very-long-project", Provider: "github"},
		{Key: "selected-project", Provider: "jira"},
		{Key: "third-very-long-project", Provider: "linear"},
	}})
	model.selectedWorkProject = 1

	got := model.renderProjectTabs(48)
	if !strings.Contains(got, "selected-project") {
		t.Fatalf("selected tab was truncated away: %q", got)
	}
	if width := lipgloss.Width(got); width > 48 {
		t.Fatalf("tabs width = %d, exceeds 48", width)
	}
}

func TestActionSuccessAlwaysOpensVisibleResult(t *testing.T) {
	model := NewModel(Dependencies{})
	item := Action{ID: "test.action", Label: "Test action", Active: true}
	effects := model.startOrQueue(item)
	if len(effects) != 1 || effects[0].Kind != StartActionEffect {
		t.Fatalf("start effects = %#v", effects)
	}

	model.finishActionSuccess(model.active.id, nil, nil, nil)
	if model.detail == nil || len(model.detail.lines) != 1 || model.detail.lines[0] != model.l10n.Text("tui.result.complete") {
		t.Fatalf("completion detail = %#v", model.detail)
	}
	if got := model.ModalStack(); len(got) != 1 || got[0] != "detail" {
		t.Fatalf("modal stack = %#v, want detail", got)
	}
}

func TestDataOperationsAreKeyboardFocusable(t *testing.T) {
	catalog := cockpit.Operation{Relation: cockpit.Relation(DataCatalogSlot), Label: "Catalog", Active: true}
	describe := cockpit.Operation{Relation: cockpit.Relation("data.describe"), Label: "Describe", Active: true}
	model := NewModelWithSnapshot(Dependencies{}, cockpit.Snapshot{DataSources: []cockpit.DataSource{{Key: "people", Provider: "csv", Operations: []cockpit.Operation{catalog, describe}}}})
	model.setView(Data)

	model.HandleKey(Key{Code: "right"})
	model.HandleKey(Key{Code: "down"})
	if model.dataFocus != 1 || model.selectedAction != 1 {
		t.Fatalf("data focus/action = %d/%d, want 1/1", model.dataFocus, model.selectedAction)
	}
	effects := model.HandleKey(Key{Code: "enter"})
	if len(effects) != 1 || effects[0].Kind != StartActionEffect || effects[0].Action.ID != action.ID(describe.Relation) {
		t.Fatalf("enter effects = %#v, want selected Describe action", effects)
	}
}

func TestFormTextEditingUsesCursorAndSpace(t *testing.T) {
	model := NewModel(Dependencies{})
	form := FormState{Mode: EditFields, Fields: []FormField{
		{ID: "text", Kind: TextField, Value: "ac", Cursor: 1},
		{ID: "toggle", Kind: ToggleField, Value: "false"},
	}}
	model.form = &form

	model.HandleKey(Key{Code: "text", Text: "b"})
	model.HandleKey(Key{Code: "left"})
	model.HandleKey(Key{Code: "space"})
	if got := model.form.Fields[0].Value; got != "a bc" {
		t.Fatalf("edited value = %q, want %q", got, "a bc")
	}
	model.HandleKey(Key{Code: "backspace"})
	model.HandleKey(Key{Code: "delete"})
	if got := model.form.Fields[0].Value; got != "ac" {
		t.Fatalf("value after backspace/delete = %q, want ac", got)
	}
	model.HandleKey(Key{Code: "tab"})
	model.HandleKey(Key{Code: "space"})
	if !model.form.Fields[1].enabled() {
		t.Fatal("space did not toggle the selected toggle field")
	}
}

func TestPromptChoiceWindowTracksSelection(t *testing.T) {
	model := NewModel(Dependencies{})
	choices := make([]string, 20)
	for index := range choices {
		choices[index] = "choice"
	}
	model.prompt = &inputPrompt{prompt: action.SelectOnePrompt{}, choices: choices, selected: 19}

	got := model.renderPromptChoices(4)
	if lines := strings.Count(got, "\n") + 1; lines != 4 {
		t.Fatalf("choice lines = %d, want 4:\n%s", lines, got)
	}
	if !strings.Contains(got, "›") {
		t.Fatalf("selected choice is not visible:\n%s", got)
	}
}

func TestScrollableEndUsesWrappedVisualLines(t *testing.T) {
	model := NewModel(Dependencies{})
	model.width, model.height = 50, 14
	model.detail = &detailState{title: "Detail", lines: []string{strings.Repeat("x", 240)}}
	model.pushModal(detailModal)

	model.HandleKey(Key{Code: "end"})
	want := model.maxDetailScroll()
	if want <= 0 || model.detail.scroll != want {
		t.Fatalf("detail scroll = %d, max = %d; wrapped content should scroll", model.detail.scroll, want)
	}
	model.HandleKey(Key{Code: "down"})
	if model.detail.scroll != want {
		t.Fatalf("detail scrolled beyond max: %d > %d", model.detail.scroll, want)
	}
}

func TestLoaderErrorsAppearInPrimaryView(t *testing.T) {
	model := NewModel(Dependencies{})
	model.width, model.height = 100, 30
	model.workLoad.errorText = "provider unavailable"

	got := model.render()
	if !strings.Contains(got, "Load failed:") || !strings.Contains(got, "provider unavailable") || !strings.Contains(got, "[r] retry") {
		t.Fatalf("primary view lacks loader error banner:\n%s", got)
	}
}

func TestSemanticStatusReplacesRawBooleans(t *testing.T) {
	model := NewModel(Dependencies{})
	if got := model.semanticStatus("true"); got != model.l10n.Text("tui.label.yes") {
		t.Fatalf("true status = %q", got)
	}
	if got := model.semanticStatus("false"); got != model.l10n.Text("tui.label.no") {
		t.Fatalf("false status = %q", got)
	}
}

func TestMouseReportingIsDisabled(t *testing.T) {
	model := NewModel(Dependencies{})
	if got := model.View().MouseMode; got != tea.MouseModeNone {
		t.Fatalf("mouse mode = %v, want none", got)
	}
}

func TestEveryViewFitsTerminalAtWideAndCompactSizes(t *testing.T) {
	operation := cockpit.Operation{Relation: cockpit.Relation(DataCatalogSlot), Label: "Catalog", Description: "Inspect available records", Active: true}
	snapshot := cockpit.Snapshot{
		Root:            "/tmp/example",
		ProjectCount:    1,
		RepositoryCount: 1,
		DefaultAgent:    "codex",
		Workspaces: []cockpit.Workspace{{
			Project: "example", WorkItems: []string{"WORK-123"}, Type: "feature", Slug: "responsive-layout", Branch: "feature/responsive-layout", Repositories: []string{"repository"}, Operations: []cockpit.Operation{operation},
		}},
		WorkProjects: []cockpit.WorkProject{{Key: "example", Provider: "provider", Items: []cockpit.WorkItem{{ID: "WORK-123", Type: "story", State: "active", Title: "Make every view fit its terminal", Operations: []cockpit.Operation{operation}}}}},
		PullRequests: []cockpit.PullRequest{{ID: "42", Project: "example", Repository: "repository", Branch: "feature/responsive-layout", Title: "Responsive layout", Draft: true, WorkItems: []string{"WORK-123"}, Operations: []cockpit.Operation{operation}}},
		DataSources:  []cockpit.DataSource{{Project: "example", Key: "people", Provider: "csv", Operations: []cockpit.Operation{operation}}},
		Cockpit:      []cockpit.CockpitItem{{Section: "Workspace", Title: "Responsive layout", Status: "true", Primary: operation, Subtitle: "WORK-123"}},
	}

	for _, size := range []struct{ width, height int }{{120, 30}, {50, 14}} {
		for _, view := range allViews {
			t.Run(fmt.Sprintf("view-%d-width-%d", view, size.width), func(t *testing.T) {
				model := NewModelWithSnapshot(Dependencies{}, snapshot)
				model.width, model.height, model.view = size.width, size.height, view
				got := model.render()
				if width := lipgloss.Width(got); width > size.width {
					t.Fatalf("rendered width = %d, terminal width = %d", width, size.width)
				}
				if height := lipgloss.Height(got); height > size.height {
					t.Fatalf("rendered height = %d, terminal height = %d", height, size.height)
				}
				if strings.Contains(got, " true ") || strings.Contains(got, " false ") {
					t.Fatalf("render contains raw boolean status:\\n%s", got)
				}
			})
		}
	}
}
