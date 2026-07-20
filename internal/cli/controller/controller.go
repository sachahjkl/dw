package controller

import (
	"context"
	"fmt"
	"io"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
)

type Execution struct {
	Dispatcher    *action.Dispatcher
	Console       console.Engine
	Localizer     l10n.Localizer
	Policy        console.Policy
	SafetyGranted bool
}

type Controller struct {
	root                 *spec.Command
	routes               *Registry
	execution            Execution
	packageVersion       string
	informationalVersion string
}

func New(root *spec.Command, routes *Registry, execution Execution, packageVersion, informationalVersion string) (*Controller, error) {
	if root == nil || routes == nil || execution.Dispatcher == nil || execution.Console.Results == nil || execution.Localizer == nil {
		return nil, fmt.Errorf("cli.invalid-controller-dependencies")
	}
	if err := routes.ValidateComplete(root); err != nil {
		return nil, err
	}
	return &Controller{root: root, routes: routes, execution: execution, packageVersion: packageVersion, informationalVersion: informationalVersion}, nil
}

// Run parses, dispatches and presents one CLI invocation. It never terminates
// the process; cmd/dw is the sole os.Exit boundary.
func (controller *Controller) Run(ctx context.Context, args []string) console.ExitCode {
	invocation, parseErr := parse.Parse(controller.root, args)
	if parseErr != nil {
		if err := writeRaw(controller.execution.Policy.Streams.Stderr, parse.Diagnostic(controller.root, parseErr)); err != nil {
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
	if route.Grant != nil {
		if err := route.Grant(ctx, execution, invocation); err != nil {
			return controller.fail(err)
		}
		execution.SafetyGranted = true
	}

	var outcome Outcome
	var err error
	if route.Direct != nil {
		outcome, err = route.Direct(ctx, execution, invocation)
	} else {
		outcome, err = controller.dispatch(ctx, execution, route, invocation)
	}
	if err != nil {
		return controller.fail(err)
	}
	if err := console.WriteOutput(policy.Streams.Stdout, outcome.Output); err != nil {
		return ExitCode(err)
	}
	return outcome.Code
}

func (controller *Controller) dispatch(ctx context.Context, execution Execution, route Route, invocation *parse.Result) (Outcome, error) {
	request, err := route.Build(invocation)
	if err != nil {
		return Outcome{}, err
	}
	runtime := action.Runtime{
		Events: NewEventSink(execution.Console, execution.Policy, execution.Localizer, invocation.Verbosity),
		Input:  NewTerminalInput(execution.Policy.Streams, execution.Localizer),
	}
	result, err := execution.Dispatcher.Dispatch(ctx, request, runtime)
	if err != nil {
		return Outcome{}, err
	}
	format, projection, err := route.Project(result, invocation)
	if err != nil {
		return Outcome{}, err
	}
	output, err := execution.Console.RenderResultKind(
		console.NewRenderContextForFormat(execution.Policy, execution.Localizer, format),
		result,
		action.ID(route.Key),
		format,
		projection,
	)
	if err != nil {
		return Outcome{}, err
	}
	if err := runExternalResult(ctx, execution, route, invocation, result); err != nil {
		return Outcome{}, err
	}
	code := console.ExitSuccess
	if route.Status != nil {
		code = route.Status(result)
	}
	return Outcome{Output: output, Code: code}, nil
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
