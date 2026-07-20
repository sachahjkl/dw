package controller

import (
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/workapp"
)

func assignedProject(envelope action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	report, ok := envelope.Result.(workapp.AssignedReport)
	if !ok {
		return projectionTypeError("work.item.list", envelope)
	}
	if report.GroupByParent {
		return marshalProjection(report.Groups)
	}
	return marshalProjection(report.Items)
}

func pullRequestsProject(envelope action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	report, ok := envelope.Result.(workapp.PullRequestsReport)
	if !ok {
		return projectionTypeError("work.pr.list", envelope)
	}
	return marshalProjection(report.Items)
}

func workItemsProject(envelope action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	report, ok := envelope.Result.(workapp.ItemShowReport)
	if !ok {
		return projectionTypeError("work.item.show", envelope)
	}
	return marshalProjection(report.Items)
}

func contextProject(envelope action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	report, ok := envelope.Result.(workapp.ContextReport)
	if !ok {
		return projectionTypeError("work.context.show", envelope)
	}
	return marshalProjection(report.Expanded)
}

func aiContextProject(envelope action.ResultEnvelope, _ *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	report, ok := envelope.Result.(workapp.AIContextResult)
	if !ok {
		return projectionTypeError("work.context.ai", envelope)
	}
	projection, err := console.WorkAIContextJSONProjection(report.Items)
	if err != nil {
		return 0, nil, err
	}
	return console.FormatJSON, &projection, nil
}

func workListProject(envelope action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	report, ok := envelope.Result.(WorkspaceListResult)
	if !ok {
		return projectionTypeError("workspace.list", envelope)
	}
	return marshalProjection(report.Items)
}

func workspacePhaseProject(envelope action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	switch report := envelope.Result.(type) {
	case WorkspaceItemUpdateResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case WorkspaceRenameResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case WorkspaceRepoAddResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case WorkspaceCommitResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case WorkspaceTeardownResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case workapp.StartResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case workapp.StartPullRequestResult:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case workapp.FinishReport:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	case workapp.PruneReport:
		if report.Execution != nil {
			return marshalProjection(*report.Execution)
		}
		return marshalProjection(report.Plan)
	default:
		return marshalProjection(envelope.Result)
	}
}

func projectionTypeError(route string, envelope action.ResultEnvelope) (console.OutputFormat, *console.JSONProjection, error) {
	return 0, nil, fmt.Errorf("cli.invalid-result:%s:%T", route, envelope.Result)
}
