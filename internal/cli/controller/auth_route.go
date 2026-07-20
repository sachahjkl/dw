package controller

import (
	"context"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workapp"
)

func authLoginRoute() Route {
	return Route{Key: "auth.login", Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
		mode := workapp.AuthLoginEnvironmentPAT
		if execution.Policy.Interactive() {
			modes := []struct {
				value workapp.AuthLoginMode
				label l10n.ID
			}{
				{workapp.AuthLoginBrowser, promptAuthBrowser},
				{workapp.AuthLoginDeviceCode, promptAuthDevice},
				{workapp.AuthLoginEnvironmentPAT, promptAuthPAT},
			}
			choices := make([]action.Choice, len(modes))
			for index, candidate := range modes {
				choices[index] = action.Choice{Value: action.ChoiceValue(candidate.value), Label: l10n.M(candidate.label)}
			}
			response, err := NewTerminalInput(execution.Policy.Streams, execution.Localizer).Request(ctx, action.Prompt{ID: "auth-login-mode", Kind: action.PromptSelectOne, Label: l10n.M(promptAuthMode), Required: true, Choices: choices})
			if err != nil {
				return Outcome{}, err
			}
			mode = workapp.AuthLoginMode(response.Value)
		}
		request := workapp.AuthLoginRequest{Root: resolvedRoot(invocation.Values), Mode: mode}
		result, err := dispatchDirect(ctx, execution, invocation, request)
		if err != nil {
			return Outcome{}, err
		}
		output, err := execution.Console.RenderResultKind(console.NewRenderContextForFormat(execution.Policy, execution.Localizer, console.FormatHuman), result, "auth.login", console.FormatHuman, nil)
		if err != nil {
			return Outcome{}, err
		}
		return success(output), nil
	}}
}
