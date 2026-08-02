package action

import (
	"context"
	"fmt"
)

// EventSink receives handler events.
type EventSink interface {
	Emit(context.Context, EventEnvelope) error
}

// EventSinkFunc adapts a function to EventSink.
type EventSinkFunc func(context.Context, EventEnvelope) error

func (f EventSinkFunc) Emit(ctx context.Context, event EventEnvelope) error { return f(ctx, event) }

// InputPort obtains a response from an explicit presentation adapter.
type InputPort interface {
	Request(context.Context, Prompt) (Response, error)
}

// InputPortFunc adapts a function to InputPort.
type InputPortFunc func(context.Context, Prompt) (Response, error)

func (f InputPortFunc) Request(ctx context.Context, prompt Prompt) (Response, error) {
	return f(ctx, prompt)
}

// Runtime contains the interactive side channels available to handlers.
type Runtime struct {
	Events EventSink
	Input  InputPort
}

// Emit safely emits when a sink is configured.
func (r Runtime) Emit(ctx context.Context, event EventEnvelope) error {
	if r.Events == nil {
		return nil
	}
	return r.Events.Emit(ctx, event)
}

// Ask validates the prompt and its concrete response.
func (r Runtime) Ask(ctx context.Context, prompt Prompt) (Response, error) {
	if prompt == nil {
		return nil, fmt.Errorf("action.invalid-prompt")
	}
	if err := prompt.Validate(); err != nil {
		return nil, err
	}
	if r.Input == nil {
		return nil, fmt.Errorf("action.input-unavailable")
	}
	response, err := r.Input.Request(ctx, prompt)
	if err != nil {
		return nil, err
	}
	if response == nil || response.PromptKind() != prompt.PromptKind() {
		responseKind := PromptKind("")
		if response != nil {
			responseKind = response.PromptKind()
		}
		return nil, fmt.Errorf("action.input-kind-mismatch:%s:%s", prompt.PromptKind(), responseKind)
	}
	if err := ValidateResponse(prompt, response); err != nil {
		return nil, err
	}
	return response, nil
}

// ValidateResponse validates one concrete response against its prompt.
func ValidateResponse(prompt Prompt, response Response) error {
	if prompt == nil || response == nil {
		return fmt.Errorf("action.input-kind-mismatch")
	}
	if prompt.PromptKind() != response.PromptKind() {
		return kindMismatch(prompt, response)
	}
	switch typedPrompt := prompt.(type) {
	case TextPrompt:
		typedResponse, ok := response.(TextResponse)
		if !ok {
			return kindMismatch(prompt, response)
		}
		if typedPrompt.Required && typedResponse.Value == "" {
			return fmt.Errorf("action.input-required:%s", prompt.PromptID())
		}
	case SecretPrompt:
		typedResponse, ok := response.(SecretResponse)
		if !ok {
			return kindMismatch(prompt, response)
		}
		if typedPrompt.Required && typedResponse.Value.Empty() {
			return fmt.Errorf("action.input-required:%s", prompt.PromptID())
		}
	case ConfirmPrompt:
		if _, ok := response.(ConfirmResponse); !ok {
			return kindMismatch(prompt, response)
		}
	case SelectOnePrompt:
		typedResponse, ok := response.(SelectOneResponse)
		if !ok {
			return kindMismatch(prompt, response)
		}
		if typedPrompt.Required && typedResponse.Value == "" {
			return fmt.Errorf("action.input-required:%s", prompt.PromptID())
		}
		if typedResponse.Value != "" && !choiceAllowed(typedPrompt.Choices, typedResponse.Value) {
			return fmt.Errorf("action.input-invalid-choice:%s", typedResponse.Value)
		}
	case SelectManyPrompt:
		typedResponse, ok := response.(SelectManyResponse)
		if !ok {
			return kindMismatch(prompt, response)
		}
		if typedPrompt.Required && len(typedResponse.Values) == 0 {
			return fmt.Errorf("action.input-required:%s", prompt.PromptID())
		}
		seen := make(map[ChoiceValue]struct{}, len(typedResponse.Values))
		for _, value := range typedResponse.Values {
			if _, duplicate := seen[value]; duplicate {
				return fmt.Errorf("action.input-duplicate-choice:%s", value)
			}
			if !choiceAllowed(typedPrompt.Choices, value) {
				return fmt.Errorf("action.input-invalid-choice:%s", value)
			}
			seen[value] = struct{}{}
		}
	default:
		return fmt.Errorf("action.invalid-prompt")
	}
	return nil
}

func choiceAllowed(choices []Choice, value ChoiceValue) bool {
	for _, choice := range choices {
		if choice.Value == value {
			return true
		}
	}
	return false
}

func kindMismatch(prompt Prompt, response Response) error {
	return fmt.Errorf("action.input-kind-mismatch:%s:%s", prompt.PromptKind(), response.PromptKind())
}
