package paritytest_test

import (
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/tui"
)

func TestTUIHotkeyViewAndModalTransitions(t *testing.T) {
	model := tui.NewModel(tui.Dependencies{Root: "/isolated/root"})
	if model.CurrentView() != tui.Dashboard {
		t.Fatalf("initial view = %v", model.CurrentView())
	}
	for _, transition := range []struct {
		key  string
		view tui.View
	}{
		{key: "2", view: tui.Workspaces},
		{key: "3", view: tui.Work},
		{key: "4", view: tui.PullRequests},
		{key: "5", view: tui.Data},
		{key: "6", view: tui.Composer},
		{key: "tab", view: tui.Dashboard},
		{key: "shift+tab", view: tui.Composer},
		{key: "left", view: tui.Data},
		{key: "right", view: tui.Composer},
	} {
		model.HandleKey(tui.Key{Code: transition.key})
		if model.CurrentView() != transition.view {
			t.Fatalf("key %q selected %v, want %v", transition.key, model.CurrentView(), transition.view)
		}
	}

	model.HandleKey(tui.Key{Code: "?"})
	if got, want := model.ModalStack(), []string{"help"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("help modal stack = %#v, want %#v", got, want)
	}
	model.HandleKey(tui.Key{Code: "esc"})
	if got := model.ModalStack(); len(got) != 0 {
		t.Fatalf("escape did not close help: %#v", got)
	}
	model.HandleKey(tui.Key{Code: "m"})
	model.HandleKey(tui.Key{Code: "enter"})
	if got, want := model.ModalStack(), []string{"menu", "menu-section"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("nested menu stack = %#v, want %#v", got, want)
	}
	model.HandleKey(tui.Key{Code: "esc"})
	if got, want := model.ModalStack(), []string{"menu"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("escape did not return to parent menu: %#v", got)
	}
	model.HandleKey(tui.Key{Code: "m"})
	if got := model.ModalStack(); len(got) != 0 {
		t.Fatalf("menu hotkey did not close menu: %#v", got)
	}
}

func TestTUIReleaseReloadAndQuitHotkeys(t *testing.T) {
	model := tui.NewModel(tui.Dependencies{})
	if effects := model.HandleKey(tui.Key{Code: "2", Kind: tui.KeyRelease}); len(effects) != 0 || model.CurrentView() != tui.Dashboard {
		t.Fatalf("key release changed state: effects=%#v view=%v", effects, model.CurrentView())
	}
	effects := model.HandleKey(tui.Key{Code: "r"})
	if len(effects) != 1 || effects[0].Kind != tui.ReloadEffect {
		t.Fatalf("reload effects = %#v", effects)
	}
	effects = model.HandleKey(tui.Key{Code: "c", Ctrl: true})
	if len(effects) != 1 || effects[0].Kind != tui.QuitEffect || !model.ShouldQuit() {
		t.Fatalf("ctrl+c effects = %#v quit=%v", effects, model.ShouldQuit())
	}
}

func TestTUIFilterAndDestructiveConfirmationTransitions(t *testing.T) {
	snapshot := tui.Snapshot{Workspaces: []tui.Workspace{{
		Path: "/isolated/root/workspaces/42",
		Actions: []tui.Action{
			{ID: tui.WorkspaceOpenSlot, Label: "Open", Hotkey: "o", Risk: tui.Safe, Active: true},
			{ID: tui.WorkspaceRemoveSlot, Label: "Remove", Hotkey: "x", Risk: tui.Destructive, Active: true},
		},
	}}}
	model := tui.NewModelWithSnapshot(tui.Dependencies{Root: "/isolated/root"}, snapshot)
	model.HandleKey(tui.Key{Code: "2"})
	model.HandleKey(tui.Key{Code: "/"})
	model.HandleKey(tui.Key{Text: "f"})
	model.HandleKey(tui.Key{Text: "r"})
	if value, active := model.Filter(); !active || value != "fr" {
		t.Fatalf("filter = %q active=%v", value, active)
	}
	model.HandleKey(tui.Key{Code: "esc"})
	if value, active := model.Filter(); active || value != "" {
		t.Fatalf("escape filter = %q active=%v", value, active)
	}

	effects := model.HandleKey(tui.Key{Code: "x"})
	if len(effects) != 0 || !model.ConfirmationOpen() {
		t.Fatalf("destructive key effects=%#v confirmation=%v", effects, model.ConfirmationOpen())
	}
	model.HandleKey(tui.Key{Code: "esc"})
	if model.ConfirmationOpen() {
		t.Fatal("escape did not cancel destructive confirmation")
	}
	effects = model.HandleKey(tui.Key{Code: "o"})
	if len(effects) != 1 || effects[0].Kind != tui.StartActionEffect || effects[0].Action.ID != tui.WorkspaceOpenSlot {
		t.Fatalf("safe action effects = %#v", effects)
	}
}
