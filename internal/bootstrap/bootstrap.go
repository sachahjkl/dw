package bootstrap

import (
	"context"
	"fmt"
	"io"
	"os"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/buildinfo"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
)

// Run composes and executes one CLI invocation. Process termination remains in cmd/dw.
func Run(ctx context.Context, args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	streams := bootstrapStreams(stdin, stdout, stderr)
	localizer, err := englishCatalog()
	if err != nil {
		_, _ = fmt.Fprintln(streams.Stderr, err)
		return int(console.ExitFailure)
	}
	policy := console.NewPolicy(streams, configuredColorMode(), os.LookupEnv)
	application, err := newController(localizer, policy)
	if err != nil {
		line := console.ErrorLine(localizer, console.NewTheme(policy.StderrColor()), err)
		_ = console.WriteDiagnostic(streams.Stderr, line)
		return int(console.ExitFailure)
	}
	return int(application.Run(ctx, args))
}

func newController(localizer l10n.Localizer, policy console.Policy) (*controller.Controller, error) {
	services, err := newServices()
	if err != nil {
		return nil, err
	}
	dispatcher := action.NewDispatcher()
	if err = registerHandlers(dispatcher, services); err != nil {
		return nil, err
	}
	results := console.NewRegistry()
	events := console.NewEventRegistry()
	if err = registerConsole(results, events); err != nil {
		return nil, err
	}
	engine := console.NewEngine(results, events)
	grammar := spec.Root(localizer)
	routes := controller.NewRegistry()
	if err = controller.RegisterRoutes(routes, controller.Integration{
		Root:                 grammar,
		InformationalVersion: buildinfo.Informational(),
		PackageVersion:       buildinfo.Version,
		Completion:           services.completion,
		RunTUI:               runTUI(services, dispatcher, routes, grammar),
	}); err != nil {
		return nil, err
	}
	return controller.New(grammar, routes, controller.Execution{
		Dispatcher: dispatcher,
		Console:    engine,
		Localizer:  localizer,
		Policy:     policy,
	}, buildinfo.Version, buildinfo.Informational())
}

func englishCatalog() (*l10n.Catalog, error) {
	catalog, err := l10n.NewEnglish().Extend(console.EnglishEntries...)
	if err != nil {
		return nil, err
	}
	catalog, err = catalog.Extend(controller.SafetyEnglishEntries...)
	if err != nil {
		return nil, err
	}
	catalog, err = catalog.Extend(bootstrapTUIEnglishEntries...)
	if err != nil {
		return nil, err
	}
	return catalog.Extend(spec.EnglishEntries()...)
}

func bootstrapStreams(stdin io.Reader, stdout, stderr io.Writer) console.Streams {
	if stdin == nil {
		stdin = os.Stdin
	}
	if stdout == nil {
		stdout = os.Stdout
	}
	if stderr == nil {
		stderr = os.Stderr
	}
	inputFile, _ := stdin.(*os.File)
	outputFile, _ := stdout.(*os.File)
	errorFile, _ := stderr.(*os.File)
	streams := console.DetectStreams(inputFile, outputFile, errorFile)
	streams.Stdin = stdin
	streams.Stdout = stdout
	streams.Stderr = stderr
	return streams
}

func configuredColorMode() console.ColorMode {
	mode := config.NormalizeColorMode(config.LoadUserSettings().Color)
	switch mode {
	case config.ColorAlways:
		return console.ColorAlways
	case config.ColorNever:
		return console.ColorNever
	default:
		return console.ColorAuto
	}
}
