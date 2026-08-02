package web

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/execution"
)

func decodePromptView(record execution.Record, csrf string) *promptView {
	encoded := record.PendingPrompt
	if encoded == nil {
		return nil
	}
	view := &promptView{ID: string(encoded.ID), Kind: string(encoded.Kind), Signal: "dw_prompt_" + record.ExecutionID.String()}
	endpoint := fmt.Sprintf("/executions/%s/responses/%s", record.ExecutionID.String(), encoded.ID)
	switch encoded.Kind {
	case action.PromptText:
		var prompt action.TextPrompt
		if json.Unmarshal(encoded.JSON, &prompt) != nil {
			return nil
		}
		setPromptMeta(view, prompt.Meta, prompt.Required)
		view.Submit = responseExpression(endpoint, csrf, fmt.Sprintf("{schema:1,value:String($%s)}", view.Signal))
	case action.PromptSecret:
		var prompt action.SecretPrompt
		if json.Unmarshal(encoded.JSON, &prompt) != nil {
			return nil
		}
		setPromptMeta(view, prompt.Meta, prompt.Required)
		view.Submit = fmt.Sprintf("@post(%q, {contentType:'form', headers:{'X-DW-CSRF':%q}}).finally(() => evt.target.reset())", endpoint, csrf)
	case action.PromptConfirm:
		var prompt action.ConfirmPrompt
		if json.Unmarshal(encoded.JSON, &prompt) != nil {
			return nil
		}
		setPromptMeta(view, prompt.Meta, false)
		view.Submit = responseExpression(endpoint, csrf, fmt.Sprintf("{schema:1,accepted:Boolean($%s)}", view.Signal))
	case action.PromptSelectOne:
		var prompt action.SelectOnePrompt
		if json.Unmarshal(encoded.JSON, &prompt) != nil {
			return nil
		}
		setPromptMeta(view, prompt.Meta, prompt.Required)
		view.Choices = promptChoices(prompt.Choices)
		view.Submit = responseExpression(endpoint, csrf, fmt.Sprintf("{schema:1,value:String($%s)}", view.Signal))
	case action.PromptSelectMany:
		var prompt action.SelectManyPrompt
		if json.Unmarshal(encoded.JSON, &prompt) != nil {
			return nil
		}
		setPromptMeta(view, prompt.Meta, prompt.Required)
		view.Choices = promptChoicesWithSignals(prompt.Choices, view.Signal)
		values := make([]string, len(view.Choices))
		for index, choice := range view.Choices {
			values[index] = fmt.Sprintf("($%s ? %q : null)", choice.Signal, choice.Value)
		}
		view.Submit = responseExpression(endpoint, csrf, fmt.Sprintf("{schema:1,values:[%s].filter(value => value !== null)}", strings.Join(values, ",")))
	default:
		return nil
	}
	return view
}

func setPromptMeta(view *promptView, meta action.PromptMeta, required bool) {
	view.Label = string(meta.Label.ID)
	view.Required = required
	if meta.Help != nil {
		view.Help = string(meta.Help.ID)
	}
}

func promptChoices(choices []action.Choice) []choiceView {
	values := make([]choiceView, 0, len(choices))
	for _, choice := range choices {
		values = append(values, choiceView{Value: string(choice.Value), Label: string(choice.Label.ID)})
	}
	return values
}

func promptChoicesWithSignals(choices []action.Choice, prefix string) []choiceView {
	values := promptChoices(choices)
	for index := range values {
		values[index].Signal = fmt.Sprintf("%s_%d", prefix, index)
	}
	return values
}

func responseExpression(endpoint, csrf, payload string) string {
	return fmt.Sprintf("@post(%q, {contentType:'json', headers:{'X-DW-CSRF':%q}, payload:%s})", endpoint, csrf, payload)
}
