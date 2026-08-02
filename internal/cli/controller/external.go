package controller

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/workapp"
)

func runExternalResult(ctx context.Context, execution Execution, route Route, invocation *parse.Result, result action.ResultEnvelope) error {
	if invocation.Values.Bool("json") {
		return nil
	}
	var report *workapp.OpenReport
	switch route.Key {
	case "agent.open", "workspace.open":
		value, ok := result.Result.(workapp.OpenReport)
		if !ok {
			return fmt.Errorf("cli.invalid-external-result:%s:%T", route.Key, result.Result)
		}
		report = &value
	case "workspace.start":
		value, ok := result.Result.(workapp.StartResult)
		if !ok {
			return fmt.Errorf("cli.invalid-external-result:%s:%T", route.Key, result.Result)
		}
		report = value.Open
	default:
		return nil
	}
	if report == nil {
		return nil
	}
	if report.Launch == nil {
		return fmt.Errorf("cli.invalid-external-launch:%s:nil", route.Key)
	}
	return agent.RunLaunch(ctx, *report.Launch, execution.Policy.Streams.Stdin, execution.Policy.Streams.Stdout, execution.Policy.Streams.Stderr)
}
