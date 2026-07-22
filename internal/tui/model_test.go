package tui

import (
	"reflect"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
)

func TestConfirmEnterHonorsNegativeDefault(t *testing.T) {
	model := NewModel(Dependencies{})
	defaultValue := action.ChoiceValue("false")
	responses := make(chan action.Response, 1)
	model.prompt = &inputPrompt{
		prompt:   action.Prompt{Kind: action.PromptConfirm, Default: &defaultValue},
		response: responses,
	}

	effects := model.HandleKey(Key{Code: "enter"})

	if len(effects) != 1 || effects[0].Kind != AnswerInputEffect {
		t.Fatalf("enter effects = %#v, want one input answer", effects)
	}
	if effects[0].Response.Accepted {
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
