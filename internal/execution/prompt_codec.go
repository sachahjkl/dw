package execution

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

type TextPromptV1 struct {
	Meta     action.PromptMeta `json:"meta"`
	Required bool              `json:"required"`
}

type SecretPromptV1 struct {
	Meta     action.PromptMeta `json:"meta"`
	Required bool              `json:"required"`
}

type ConfirmPromptV1 struct {
	Meta    action.PromptMeta `json:"meta"`
	Default bool              `json:"default"`
}

type SelectOnePromptV1 struct {
	Meta     action.PromptMeta   `json:"meta"`
	Required bool                `json:"required"`
	Choices  []action.Choice     `json:"choices"`
	Default  *action.ChoiceValue `json:"default,omitempty"`
}

type SelectManyPromptV1 struct {
	Meta     action.PromptMeta    `json:"meta"`
	Required bool                 `json:"required"`
	Choices  []action.Choice      `json:"choices"`
	Defaults []action.ChoiceValue `json:"defaults"`
}

type TextResponseV1 struct {
	Value string `json:"value"`
}

type ConfirmResponseV1 struct {
	Accepted bool `json:"accepted"`
}

type SelectOneResponseV1 struct {
	Value action.ChoiceValue `json:"value"`
}

type SelectManyResponseV1 struct {
	Values []action.ChoiceValue `json:"values"`
}

func EncodePrompt(prompt action.Prompt) (EncodedPrompt, error) {
	if prompt == nil {
		return EncodedPrompt{}, fmt.Errorf("execution.invalid-prompt")
	}
	if err := prompt.Validate(); err != nil {
		return EncodedPrompt{}, err
	}
	encoded := EncodedPrompt{ID: prompt.PromptID(), Kind: prompt.PromptKind(), Schema: 1}
	var err error
	switch typed := prompt.(type) {
	case action.TextPrompt:
		encoded.JSON, err = json.Marshal(TextPromptV1{Meta: typed.Meta, Required: typed.Required})
	case action.SecretPrompt:
		encoded.JSON, err = json.Marshal(SecretPromptV1{Meta: typed.Meta, Required: typed.Required})
		encoded.Redacted = true
	case action.ConfirmPrompt:
		encoded.JSON, err = json.Marshal(ConfirmPromptV1{Meta: typed.Meta, Default: typed.Default})
	case action.SelectOnePrompt:
		encoded.JSON, err = json.Marshal(SelectOnePromptV1{Meta: typed.Meta, Required: typed.Required, Choices: typed.Choices, Default: typed.Default})
	case action.SelectManyPrompt:
		encoded.JSON, err = json.Marshal(SelectManyPromptV1{Meta: typed.Meta, Required: typed.Required, Choices: typed.Choices, Defaults: typed.Defaults})
	default:
		return EncodedPrompt{}, fmt.Errorf("execution.invalid-prompt-kind:%T", prompt)
	}
	if err != nil {
		return EncodedPrompt{}, fmt.Errorf("execution.encode-prompt:%w", err)
	}
	return encoded, nil
}

func DecodePrompt(encoded EncodedPrompt) (action.Prompt, error) {
	if encoded.ID == "" || encoded.Schema != 1 {
		return nil, fmt.Errorf("execution.invalid-prompt")
	}
	var prompt action.Prompt
	switch encoded.Kind {
	case action.PromptText:
		var value TextPromptV1
		if err := decodePromptJSON(encoded.JSON, &value); err != nil {
			return nil, err
		}
		prompt = action.TextPrompt{Meta: value.Meta, Required: value.Required}
	case action.PromptSecret:
		value := SecretPromptV1{Meta: action.PromptMeta{ID: encoded.ID, Label: l10n.M(l10n.ID(encoded.ID))}, Required: true}
		if len(encoded.JSON) != 0 {
			if err := decodePromptJSON(encoded.JSON, &value); err != nil {
				return nil, err
			}
		}
		prompt = action.SecretPrompt{Meta: value.Meta, Required: value.Required}
	case action.PromptConfirm:
		var value ConfirmPromptV1
		if err := decodePromptJSON(encoded.JSON, &value); err != nil {
			return nil, err
		}
		prompt = action.ConfirmPrompt{Meta: value.Meta, Default: value.Default}
	case action.PromptSelectOne:
		var value SelectOnePromptV1
		if err := decodePromptJSON(encoded.JSON, &value); err != nil {
			return nil, err
		}
		prompt = action.SelectOnePrompt{Meta: value.Meta, Required: value.Required, Choices: value.Choices, Default: value.Default}
	case action.PromptSelectMany:
		var value SelectManyPromptV1
		if err := decodePromptJSON(encoded.JSON, &value); err != nil {
			return nil, err
		}
		prompt = action.SelectManyPrompt{Meta: value.Meta, Required: value.Required, Choices: value.Choices, Defaults: value.Defaults}
	default:
		return nil, fmt.Errorf("execution.invalid-prompt-kind:%s", encoded.Kind)
	}
	if prompt.PromptID() != encoded.ID || prompt.PromptKind() != encoded.Kind {
		return nil, fmt.Errorf("execution.prompt-discriminator-mismatch:%s", encoded.ID)
	}
	if err := prompt.Validate(); err != nil {
		return nil, err
	}
	return prompt, nil
}

type promptDTO interface {
	TextPromptV1 | SecretPromptV1 | ConfirmPromptV1 | SelectOnePromptV1 | SelectManyPromptV1
}

func decodePromptJSON[T promptDTO](data json.RawMessage, destination *T) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return fmt.Errorf("execution.decode-prompt:%w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		if err == nil {
			return fmt.Errorf("execution.decode-prompt-trailing-value")
		}
		return fmt.Errorf("execution.decode-prompt:%w", err)
	}
	return nil
}

func EncodeResponse(response action.Response) (json.RawMessage, bool, error) {
	switch typed := response.(type) {
	case action.TextResponse:
		value, err := json.Marshal(TextResponseV1{Value: typed.Value})
		return value, false, err
	case action.SecretResponse:
		return nil, true, nil
	case action.ConfirmResponse:
		value, err := json.Marshal(ConfirmResponseV1{Accepted: typed.Accepted})
		return value, false, err
	case action.SelectOneResponse:
		value, err := json.Marshal(SelectOneResponseV1{Value: typed.Value})
		return value, false, err
	case action.SelectManyResponse:
		value, err := json.Marshal(SelectManyResponseV1{Values: typed.Values})
		return value, false, err
	default:
		return nil, false, fmt.Errorf("execution.invalid-response-kind:%T", response)
	}
}
