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
	if route.Key != "agent.open" && route.Key != "workspace.open" || invocation.Values.Bool("json") {
		return nil
	}
	report, ok := result.Result.(workapp.OpenReport)
	if !ok {
		return fmt.Errorf("cli.invalid-external-result:%s:%T", route.Key, result.Result)
	}
	launch, ok := report.Launch.(agent.Launch)
	if !ok {
		return fmt.Errorf("cli.invalid-external-launch:%s:%T", route.Key, report.Launch)
	}
	return agent.RunLaunch(ctx, launch, execution.Policy.Streams.Stdin, execution.Policy.Streams.Stdout, execution.Policy.Streams.Stderr)
}
