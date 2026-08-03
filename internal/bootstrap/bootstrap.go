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
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	"github.com/sachahjkl/dw/internal/web"
	"github.com/sachahjkl/dw/internal/webservice"
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
	dirs := config.ResolvePlatformBaseDirs()
	runtimeSettings, err := runtimeconfig.Load(dirs)
	if err != nil {
		line := console.ErrorLine(localizer, console.NewTheme(policy.StderrColor()), err)
		_ = console.WriteDiagnostic(streams.Stderr, line)
		return int(console.ExitFailure)
	}
	application, err := newController(localizer, policy, dirs, runtimeSettings)
	if err != nil {
		line := console.ErrorLine(localizer, console.NewTheme(policy.StderrColor()), err)
		_ = console.WriteDiagnostic(streams.Stderr, line)
		return int(console.ExitFailure)
	}
	code := application.Run(ctx, args)
	closeContext, cancel := context.WithTimeout(context.Background(), runtimeconfig.Milliseconds(runtimeSettings.Execution.CloseTimeoutMilliseconds))
	defer cancel()
	if err := application.Close(closeContext); err != nil && code == console.ExitSuccess {
		line := console.ErrorLine(localizer, console.NewTheme(policy.StderrColor()), err)
		_ = console.WriteDiagnostic(streams.Stderr, line)
		return int(console.ExitFailure)
	}
	return int(code)
}

func newController(localizer l10n.Localizer, policy console.Policy, dirs config.PlatformBaseDirs, runtimeSettings runtimeconfig.Config) (*controller.Controller, error) {
	services, err := newServices()
	if err != nil {
		return nil, err
	}
	dispatcher := action.NewDispatcher()
	if err = registerHandlers(dispatcher, services); err != nil {
		return nil, err
	}
	services.executionRegistry, services.eventDataRegistry, err = executionRegistries(dispatcher)
	if err != nil {
		return nil, err
	}
	executor := newLazyExecutor(func() (execution.Executor, error) {
		executionStore, openErr := execution.OpenSQLiteStore("")
		if openErr != nil {
			return nil, openErr
		}
		service, serviceErr := execution.NewServiceWithConfig(dispatcher, services.executionRegistry, services.eventDataRegistry, executionStore, runtimeSettings.Execution)
		if serviceErr != nil {
			_ = executionStore.Close()
			return nil, serviceErr
		}
		return service, nil
	})
	services.executor = executor
	principal, err := execution.CurrentPrincipal()
	if err != nil {
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
	executable, err := os.Executable()
	if err != nil {
		return nil, err
	}
	webManager, err := webservice.NewManagerWithSettings(dirs, executable, runtimeSettings.WebService)
	if err != nil {
		return nil, err
	}
	actor := execution.Actor{Principal: principal, Origin: execution.OriginCLI}
	runWebServe := func(ctx context.Context) error {
		webConfig, loadErr := webManager.Store().LoadConfig()
		if loadErr != nil {
			return loadErr
		}
		runningExecutor, executorErr := executor.start()
		if executorErr != nil {
			return executorErr
		}
		cockpitService, cockpitErr := newCockpitService(services, localizer)
		if cockpitErr != nil {
			return cockpitErr
		}
		server, serverErr := web.New(web.Dependencies{
			Executor:  runningExecutor,
			Actor:     actor,
			Localizer: localizer,
			Cockpit:   cockpitService,
			ProjectResult: func(result action.Result) []string {
				output, renderErr := engine.Results.Render(console.NewRenderContext(policy, localizer), result.ActionID(), result)
				if renderErr != nil {
					return []string{console.LocalizedErrorText(localizer, renderErr)}
				}
				return console.Lines(output)
			},
			ProjectPage: func(result action.Result) (console.Page, bool, error) {
				return engine.Results.ProjectPage(result.ActionID(), result)
			},
			Store:    webManager.Store(),
			Config:   webConfig,
			Settings: runtimeSettings.Web,
		})
		if serverErr != nil {
			return serverErr
		}
		return server.Serve(ctx)
	}
	if err = controller.RegisterRoutes(routes, controller.Integration{
		Root:                 grammar,
		InformationalVersion: buildinfo.Informational(),
		PackageVersion:       buildinfo.Version,
		Completion:           services.completion,
		RunTUI:               runTUI(services, routes, grammar),
		Web:                  webManager,
		RunWebServe:          runWebServe,
	}); err != nil {
		return nil, err
	}
	application, err := controller.New(grammar, routes, controller.Execution{
		Executor:  executor,
		Actor:     actor,
		Console:   engine,
		Localizer: localizer,
		Policy:    policy,
	}, services.completion, buildinfo.Version, buildinfo.Informational())
	if err != nil {
		return nil, err
	}
	return application, nil
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
