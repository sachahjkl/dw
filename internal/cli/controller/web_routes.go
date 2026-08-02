package controller

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/webservice"
)

type WebLifecycle interface {
	Start(context.Context, webservice.StartOptions) (webservice.StatusV1, error)
	Stop(context.Context) error
	Status(context.Context) (webservice.StatusV1, error)
	Open(context.Context) error
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
	return map[string]Route{
		"web.start": {Key: "web.start", Direct: func(ctx context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
			status, err := lifecycle.Start(ctx, webservice.StartOptions{Root: optionalStringValue(invocation, "root"), Port: optionalPortValue(invocation, "port"), NoOpen: invocation.Values.Bool("no_open")})
			if err != nil {
				return Outcome{}, err
			}
			return statusOutcome(status, false)
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
		"web.open": {Key: "web.open", Direct: func(ctx context.Context, _ Execution, _ *parse.Result) (Outcome, error) {
			return success(console.Output{}), lifecycle.Open(ctx)
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
