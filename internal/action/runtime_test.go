package action

import (
	"context"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
)

type wrongTextResponse struct{}

func (wrongTextResponse) PromptKind() PromptKind { return PromptText }
func (wrongTextResponse) isResponse()            {}

func TestRuntimeAskValidatesConcreteResponse(t *testing.T) {
	prompt := TextPrompt{Meta: testPromptMeta("text"), Required: true}
	runtime := Runtime{Input: InputPortFunc(func(context.Context, Prompt) (Response, error) {
		return wrongTextResponse{}, nil
	})}

	_, err := runtime.Ask(context.Background(), prompt)
	if err == nil || !strings.HasPrefix(err.Error(), "action.input-kind-mismatch:") {
		t.Fatalf("Ask error = %v, want action.input-kind-mismatch", err)
	}
}

func TestRuntimeAskValidatesRequiredValues(t *testing.T) {
	tests := []struct {
		name     string
		prompt   Prompt
		response Response
	}{
		{name: "text", prompt: TextPrompt{Meta: testPromptMeta("text"), Required: true}, response: TextResponse{}},
		{name: "secret", prompt: SecretPrompt{Meta: testPromptMeta("secret"), Required: true}, response: SecretResponse{Value: contract.NewSecretValue("")}},
		{name: "select one", prompt: SelectOnePrompt{Meta: testPromptMeta("one"), Required: true, Choices: testChoices()}, response: SelectOneResponse{}},
		{name: "select many", prompt: SelectManyPrompt{Meta: testPromptMeta("many"), Required: true, Choices: testChoices()}, response: SelectManyResponse{}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			runtime := Runtime{Input: InputPortFunc(func(context.Context, Prompt) (Response, error) {
				return test.response, nil
			})}
			_, err := runtime.Ask(context.Background(), test.prompt)
			if err == nil || !strings.HasPrefix(err.Error(), "action.input-required:") {
				t.Fatalf("Ask error = %v, want action.input-required", err)
			}
		})
	}
}

func TestRuntimeAskValidatesChoiceResponses(t *testing.T) {
	tests := []struct {
		name        string
		prompt      Prompt
		response    Response
		errorPrefix string
	}{
		{name: "invalid one", prompt: SelectOnePrompt{Meta: testPromptMeta("one"), Choices: testChoices()}, response: SelectOneResponse{Value: "three"}, errorPrefix: "action.input-invalid-choice:"},
		{name: "invalid many", prompt: SelectManyPrompt{Meta: testPromptMeta("many"), Choices: testChoices()}, response: SelectManyResponse{Values: []ChoiceValue{"one", "three"}}, errorPrefix: "action.input-invalid-choice:"},
		{name: "duplicate many", prompt: SelectManyPrompt{Meta: testPromptMeta("many"), Choices: testChoices()}, response: SelectManyResponse{Values: []ChoiceValue{"one", "one"}}, errorPrefix: "action.input-duplicate-choice:"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			runtime := Runtime{Input: InputPortFunc(func(context.Context, Prompt) (Response, error) {
				return test.response, nil
			})}
			_, err := runtime.Ask(context.Background(), test.prompt)
			if err == nil || !strings.HasPrefix(err.Error(), test.errorPrefix) {
				t.Fatalf("Ask error = %v, want prefix %q", err, test.errorPrefix)
			}
		})
	}
}

func TestPromptValidateChecksDefaults(t *testing.T) {
	tests := []struct {
		name        string
		prompt      Prompt
		errorPrefix string
	}{
		{name: "invalid one", prompt: SelectOnePrompt{Meta: testPromptMeta("one"), Choices: testChoices(), Default: choicePointer("three")}, errorPrefix: "action.input-invalid-choice:"},
		{name: "invalid many", prompt: SelectManyPrompt{Meta: testPromptMeta("many"), Choices: testChoices(), Defaults: []ChoiceValue{"three"}}, errorPrefix: "action.input-invalid-choice:"},
		{name: "duplicate many", prompt: SelectManyPrompt{Meta: testPromptMeta("many"), Choices: testChoices(), Defaults: []ChoiceValue{"one", "one"}}, errorPrefix: "action.input-duplicate-choice:"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := test.prompt.Validate()
			if err == nil || !strings.HasPrefix(err.Error(), test.errorPrefix) {
				t.Fatalf("Validate error = %v, want prefix %q", err, test.errorPrefix)
			}
		})
	}
}

func testPromptMeta(id PromptID) PromptMeta {
	return PromptMeta{ID: id, Label: l10n.M("test.prompt")}
}

func testChoices() []Choice {
	return []Choice{
		{Value: "one", Label: l10n.M("test.choice.one")},
		{Value: "two", Label: l10n.M("test.choice.two")},
	}
}

func choicePointer(value ChoiceValue) *ChoiceValue { return &value }
