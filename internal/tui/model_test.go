package tui

import (
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
)

func TestConfirmEnterHonorsNegativeDefault(t *testing.T) {
	model := NewModel(Dependencies{})
	model.prompt = &inputPrompt{
		prompt: action.ConfirmPrompt{Default: false},
	}

	effects := model.HandleKey(Key{Code: "enter"})

	if len(effects) != 1 || effects[0].Kind != AnswerInputEffect {
		t.Fatalf("enter effects = %#v, want one input answer", effects)
	}
	response, ok := effects[0].Response.(action.ConfirmResponse)
	if !ok || response.Accepted {
		t.Fatalf("enter accepted a confirmation whose default is false: %#v", effects[0].Response)
	}
}

func TestInformationalMenuHotkeysOpenTheirModals(t *testing.T) {
	for _, test := range []struct {
		key  string
		want []string
	}{
		{key: "h", want: []string{"menu", "menu-section", "journal"}},
		{key: "i", want: []string{"menu", "menu-section", "state"}},
	} {
		t.Run(test.key, func(t *testing.T) {
			model := NewModel(Dependencies{})
			model.HandleKey(Key{Code: "m"})
			model.HandleKey(Key{Code: "enter"})

			effects := model.HandleKey(Key{Code: test.key})

			if len(effects) != 0 {
				t.Fatalf("hotkey %q effects = %#v, want none", test.key, effects)
			}
			if got := model.ModalStack(); !reflect.DeepEqual(got, test.want) {
				t.Fatalf("hotkey %q modal stack = %#v, want %#v", test.key, got, test.want)
			}
		})
	}
}

func TestEscapeUnwindsOneLayerWithoutQuitting(t *testing.T) {
	model := NewModel(Dependencies{})
	model.HandleKey(Key{Code: "m"})
	model.HandleKey(Key{Code: "enter"})
	model.HandleKey(Key{Code: "h"})

	for _, want := range [][]string{
		{"menu", "menu-section"},
		{"menu"},
		{},
	} {
		effects := model.HandleKey(Key{Code: "esc"})
		if len(effects) != 0 {
			t.Fatalf("escape effects = %#v, want none", effects)
		}
		if got := model.ModalStack(); !reflect.DeepEqual(got, want) {
			t.Fatalf("modal stack = %#v, want %#v", got, want)
		}
		if model.ShouldQuit() {
			t.Fatal("escape quit while unwinding layers")
		}
	}

	if effects := model.HandleKey(Key{Code: "esc"}); len(effects) != 0 || model.ShouldQuit() {
		t.Fatalf("root escape effects/quit = %#v/%v, want no-op", effects, model.ShouldQuit())
	}
}

func TestEscapeReturnsComposerToPreviousView(t *testing.T) {
	model := NewModel(Dependencies{})
	model.setView(Work)
	model.setView(Composer)
	model.composer.begin(model.snapshot)

	model.HandleKey(Key{Code: "esc"})
	if model.CurrentView() != Composer || model.composer.Mode != ChooseTemplate {
		t.Fatalf("first escape view/mode = %v/%v, want composer chooser", model.CurrentView(), model.composer.Mode)
	}
	model.HandleKey(Key{Code: "esc"})
	if model.CurrentView() != Work {
		t.Fatalf("second escape view = %v, want Work", model.CurrentView())
	}
}
