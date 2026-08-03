package controller

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/webservice"
)

type WebLifecycle interface {
	Start(context.Context, webservice.StartOptions) (webservice.StartResult, error)
	Stop(context.Context) error
	Status(context.Context) (webservice.StatusV1, error)
	Open(context.Context, webservice.OpenOptions) (webservice.OpenResult, error)
	Register(context.Context, webservice.RegisterOptions) (webservice.StatusV1, error)
	Unregister(context.Context) error
}

func webRoutes(lifecycle WebLifecycle, serve func(context.Context) error) map[string]Route {
	statusOutcome := func(status webservice.StatusV1, machine bool) (Outcome, error) {
		if machine {
			body, err := json.Marshal(status)
			if err != nil {
				return Outcome{}, err
			}
			return success(console.Output{Format: console.FormatJSON, Body: append(body, '\n')}), nil
		}
		state := "stopped"
		if status.Running {
			state = "running"
		}
		if status.Stale {
			state = "stale"
		}
		text := fmt.Sprintf("Web service: %s\nAddress: %s\n", state, status.Address)
		return success(console.TextOutput(console.FormatHuman, text)), nil
	}
	openOutcome := func(result webservice.OpenResult) Outcome {
		browser := "not opened; use the URL above"
		if result.Opened {
			browser = "opened"
		}
		return success(console.TextOutput(console.FormatHuman, fmt.Sprintf("URL: %s\nBrowser: %s\n", result.Location, browser)))
	}
	return map[string]Route{
		"web.start": {Key: "web.start", Direct: func(ctx context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
			result, err := lifecycle.Start(ctx, webservice.StartOptions{
				Root:            optionalStringValue(invocation, "root"),
				Port:            optionalPortValue(invocation, "port"),
				Open:            invocation.Values.Bool("open"),
				NoExpiry:        invocation.Values.Bool("no_expiry"),
				Unauthenticated: invocation.Values.Bool("unauthenticated"),
				Token:           optionalStringValue(invocation, "token"),
			})
			if err != nil {
				return Outcome{}, err
			}
			outcome, err := statusOutcome(result.Status, false)
			if err == nil && result.Open != nil {
				opened := openOutcome(*result.Open)
				outcome.Output.Body = append(outcome.Output.Body, opened.Output.Body...)
			}
			return outcome, err
		}},
		"web.stop": {Key: "web.stop", Direct: func(ctx context.Context, _ Execution, _ *parse.Result) (Outcome, error) {
			return success(console.Output{}), lifecycle.Stop(ctx)
		}},
		"web.status": {Key: "web.status", Machine: jsonMachine, Direct: func(ctx context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
			status, err := lifecycle.Status(ctx)
			if err != nil {
				return Outcome{}, err
			}
			return statusOutcome(status, invocation.Values.Bool("json"))
		}},
		"web.open": {Key: "web.open", Direct: func(ctx context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
			options := webservice.OpenOptions{}
			if invocation != nil {
				options.NoExpiry = invocation.Values.Bool("no_expiry")
				options.Token = optionalStringValue(invocation, "token")
			}
			if options.Token != nil {
				result, err := lifecycle.Start(ctx, webservice.StartOptions{Open: true, Token: options.Token})
				if err != nil {
					return Outcome{}, err
				}
				if result.Open == nil {
					return Outcome{}, l10n.NewError("web.error.open-result-missing")
				}
				return openOutcome(*result.Open), nil
			}
			result, err := lifecycle.Open(ctx, options)
			if err != nil {
				return Outcome{}, err
			}
			return openOutcome(result), nil
		}},
		"web.register": {Key: "web.register", Direct: func(ctx context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
			status, err := lifecycle.Register(ctx, webservice.RegisterOptions{Root: optionalStringValue(invocation, "root"), Port: optionalPortValue(invocation, "port")})
			if err != nil {
				return Outcome{}, err
			}
			return statusOutcome(status, false)
		}},
		"web.unregister": {Key: "web.unregister", Direct: func(ctx context.Context, _ Execution, _ *parse.Result) (Outcome, error) {
			return success(console.Output{}), lifecycle.Unregister(ctx)
		}},
		"web.serve": {Key: "web.serve", Direct: func(ctx context.Context, _ Execution, _ *parse.Result) (Outcome, error) {
			return success(console.Output{}), serve(ctx)
		}},
	}
}

func optionalStringValue(invocation *parse.Result, name string) *string {
	if invocation == nil || !invocation.Values.Has(name) {
		return nil
	}
	value := invocation.Values.String(name)
	return &value
}

func optionalPortValue(invocation *parse.Result, name string) *uint16 {
	if invocation == nil || !invocation.Values.Has(name) {
		return nil
	}
	value := uint16(invocation.Values.Int(name))
	return &value
}
