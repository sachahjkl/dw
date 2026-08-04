package controller

import (
	"context"
	"fmt"
	"io"

	"github.com/sachahjkl/dw/internal/cli/complete"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
)

type Execution struct {
	Executor  execution.Executor
	Actor     execution.Actor
	Console   console.Engine
	Localizer l10n.Localizer
	Policy    console.Policy
}

type Controller struct {
	root                 *spec.Command
	routes               *Registry
	execution            Execution
	completion           complete.Resolver
	packageVersion       string
	informationalVersion string
}

func New(root *spec.Command, routes *Registry, execution Execution, completion complete.Resolver, packageVersion, informationalVersion string) (*Controller, error) {
	if root == nil || routes == nil || execution.Executor == nil || execution.Actor.Principal == "" || execution.Console.Results == nil || execution.Localizer == nil || completion == nil {
		return nil, fmt.Errorf("cli.invalid-controller-dependencies")
	}
	if err := routes.ValidateComplete(root); err != nil {
		return nil, err
	}
	return &Controller{root: root, routes: routes, execution: execution, completion: completion, packageVersion: packageVersion, informationalVersion: informationalVersion}, nil
}

// Run parses, dispatches and presents one CLI invocation. It never terminates
// the process; cmd/dw is the sole os.Exit boundary.
func (controller *Controller) Run(ctx context.Context, args []string) console.ExitCode {
	invocation, parseErr := parse.Parse(controller.root, args)
	if parseErr != nil {
		if err := writeRaw(controller.execution.Policy.Streams.Stderr, parse.Diagnostic(controller.root, parseErr, controller.diagnosticSuggestions(args, parseErr)...)); err != nil {
			return ExitCode(err)
		}
		return console.ExitUsage
	}

	switch invocation.Intent {
	case parse.IntentHelp:
		help, err := parse.Help(controller.root, invocation.Path, controller.informationalVersion)
		if err != nil {
			return controller.fail(err)
		}
		if err := writeRaw(controller.execution.Policy.Streams.Stdout, help); err != nil {
			return ExitCode(err)
		}
		return console.ExitSuccess
	case parse.IntentVersion:
		if err := writeRaw(controller.execution.Policy.Streams.Stdout, parse.Version(controller.root.Name, controller.informationalVersion)); err != nil {
			return ExitCode(err)
		}
		return console.ExitSuccess
	}

	route, exists := controller.routes.Route(invocation.Command.Key)
	if !exists {
		return controller.fail(fmt.Errorf("cli.missing-route:%s", invocation.Command.Key))
	}
	policy := controller.execution.Policy
	if route.Machine != nil {
		policy = policy.WithMachine(route.Machine(invocation.Values))
	}
	execution := controller.execution
	execution.Policy = policy

	var outcome Outcome
	var err error
	if route.Direct != nil {
		outcome, err = route.Direct(ctx, execution, invocation)
	} else {
		outcome, err = controller.dispatch(ctx, execution, route, invocation)
	}
	if !outcome.Output.Empty() {
		if writeErr := console.WriteOutput(policy.Streams.Stdout, outcome.Output); writeErr != nil {
			return ExitCode(writeErr)
		}
	}
	if err != nil {
		return controller.fail(err)
	}
	return outcome.Code
}
func (controller *Controller) diagnosticSuggestions(args []string, problem *parse.Error) []parse.Suggestion {
	words := append([]string(nil), args...)
	switch problem.Kind {
	case parse.MissingArgument, parse.MissingValue:
		words = append(words, "")
	case parse.UnknownCommand, parse.InvalidValue:
	default:
		return nil
	}
	items, err := complete.Complete(controller.root, words, controller.completion)
	if err != nil {
		return nil
	}
	suggestions := make([]parse.Suggestion, len(items))
	for index, item := range items {
		suggestions[index] = parse.Suggestion{Value: item.Label, Description: item.Description}
	}
	return suggestions
}

func (controller *Controller) dispatch(ctx context.Context, execution Execution, route Route, invocation *parse.Result) (Outcome, error) {
	request, err := route.Build(invocation)
	if err != nil {
		return Outcome{}, err
	}
	request = applyCLIDefaults(route.Key, request, execution.Policy)
	result, dispatchErr := executeRequest(ctx, execution, invocation, request)
	if result.Result == nil {
		return Outcome{}, dispatchErr
	}
	format, projection, err := route.Project(result, invocation)
	if err != nil {
		return Outcome{}, err
	}
	output, err := execution.Console.RenderResultKind(
		console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format),
		result,
		result.Result.ActionID(),
		format,
		projection,
	)
	if err != nil {
		return Outcome{}, err
	}
	outcome := Outcome{Output: output, Code: console.ExitSuccess}
	if dispatchErr != nil {
		return outcome, dispatchErr
	}
	if err := runExternalResult(ctx, execution, route, invocation, result); err != nil {
		return outcome, err
	}
	if route.Status != nil {
		outcome.Code = route.Status(result)
	}
	return outcome, nil
}

func (controller *Controller) Close(ctx context.Context) error {
	return controller.execution.Executor.Close(ctx)
}

func (controller *Controller) fail(err error) console.ExitCode {
	code := ExitCode(err)
	if code == console.ExitSuccess {
		return code
	}
	controller.writeFailure(err)
	return code
}

func (controller *Controller) writeFailure(err error) {
	if err == nil || console.IsBrokenPipe(err) {
		return
	}
	line := console.ErrorLine(controller.execution.Localizer, console.NewTheme(controller.execution.Policy.StderrColor()), err)
	_ = console.WriteDiagnostic(controller.execution.Policy.Streams.Stderr, line)
}

func writeRaw(writer io.Writer, value string) error {
	_, err := io.WriteString(writer, value)
	return err
}
