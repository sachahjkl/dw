package bootstrap

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/cli/complete"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
	"github.com/sachahjkl/dw/internal/data/csv"
	"github.com/sachahjkl/dw/internal/data/excel"
	"github.com/sachahjkl/dw/internal/data/sqlite"
	"github.com/sachahjkl/dw/internal/data/sqlserver"
	"github.com/sachahjkl/dw/internal/dataapp"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/providerapp"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/update"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/work/atlassian"
	"github.com/sachahjkl/dw/internal/work/github"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
	"github.com/sachahjkl/dw/internal/workspaceapp"
)

type services struct {
	secrets           contract.SecretStore
	work              *work.Registry
	data              *data.Registry
	workspace         *workspace.Engine
	workapp           *workapp.Service
	dataApplication   *dataapp.Service
	doctor            *doctor.Service
	secret            *secret.Service
	provider          *providerapp.Service
	update            *update.Service
	completion        complete.Resolver
	executionRegistry *execution.Registry
	eventDataRegistry *execution.EventDataRegistry
	executor          execution.Executor
}

func newServices() (*services, error) {
	store := secret.DefaultStore()
	adoProvider := newScopedADOProvider()
	workRegistry := work.NewRegistry()
	if err := workRegistry.Register(adoProvider); err != nil {
		return nil, err
	}
	if err := workRegistry.Register(github.New(github.Options{}, store)); err != nil {
		return nil, err
	}
	if err := workRegistry.Register(atlassian.New(atlassian.Options{}, store)); err != nil {
		return nil, err
	}

	dataRegistry := data.NewRegistry()
	if err := dataRegistry.Register(sqlserver.New(store)); err != nil {
		return nil, err
	}
	if err := dataRegistry.Register(sqlite.New()); err != nil {
		return nil, err
	}
	if err := dataRegistry.Register(csv.New()); err != nil {
		return nil, err
	}
	if err := dataRegistry.Register(excel.New()); err != nil {
		return nil, err
	}
	providerService := providerapp.New(workRegistry, dataRegistry)

	workPort := workspace.CapabilityWorkPort{
		Providers: workRegistry,
		ResolveProvider: func(ctx context.Context, project string) work.ProviderName {
			return work.ProviderName(config.ResolveWorkProvider(contextRoot(ctx), project))
		},
	}
	workspaceEngine := workspace.NewEngine(workspace.FileConfigPort{}, workspace.NewNativeGitPort(), store, workPort)
	workspacePorts := workapp.NewWorkspacePorts(workspaceEngine)
	workspacePorts.OpenFunc = openWorkspace
	workService := workapp.New(workRegistry)
	workService.ResolveProvider = config.ResolveWorkProvider
	workService.Choices = interactiveCatalog{}
	workService.Lookup = workspacePorts
	workService.Starter = workspacePorts
	workService.Syncer = workspacePorts
	workService.Children = workspacePorts
	workService.Opener = workspacePorts
	workService.Pruner = workspacePorts
	workService.Finisher = workspacePorts
	workService.GitChangelog = gitChangelogResolver{providers: workRegistry}

	return &services{
		secrets:         store,
		work:            workRegistry,
		data:            dataRegistry,
		workspace:       workspaceEngine,
		workapp:         workService,
		dataApplication: dataapp.NewService(dataRegistry, store),
		doctor:          doctor.NewSystem(),
		secret:          secret.NewService(store),
		update:          update.NewService(nil),
		provider:        providerService,
		completion:      completionResolver{workspace: workspaceEngine, providers: providerService},
	}, nil
}

func registerHandlers(dispatcher *action.Dispatcher, services *services) error {
	currentDirectory, err := os.Getwd()
	if err != nil {
		return err
	}
	handlers := make([]action.Handler, 0, 40)
	handlers = append(handlers, controller.IntegrationHandlers()...)
	handlers = append(handlers, workspaceapp.Handlers(services.workspace, services.workapp, currentDirectory)...)
	handlers = append(handlers, bootstrapHandlers()...)
	handlers = append(handlers, config.Handlers()...)
	handlers = append(handlers, dataapp.Handlers(services.dataApplication)...)
	handlers = append(handlers, providerapp.Handlers(services.provider)...)
	handlers = append(handlers, doctor.Handlers(services.doctor)...)
	handlers = append(handlers, secret.Handlers(services.secret, config.ResolveRoot)...)
	handlers = append(handlers, workapp.Handlers(services.workapp)...)
	handlers = append(handlers, update.NewHandler(services.update))
	for _, handler := range handlers {
		if err := dispatcher.Register(scopedHandler{Handler: handler}); err != nil {
			return err
		}
	}
	return nil
}

type interactiveCatalog struct{}

func (interactiveCatalog) ProjectChoices(_ context.Context, root string) ([]workapp.ChoiceOption, error) {
	projects, err := config.LoadProjectsConfigChecked(config.ResolveRoot(root))
	if err != nil {
		return nil, err
	}
	choices := make([]workapp.ChoiceOption, 0, len(projects.Projects))
	for _, project := range projects.Projects {
		label := project.Project.DisplayName
		if label == "" {
			label = project.Key
		}
		choices = append(choices, workapp.ChoiceOption{Value: project.Key, Label: l10n.M("prompt.choice.value", l10n.A("value", label))})
	}
	return choices, nil
}

func (interactiveCatalog) RepositoryChoices(_ context.Context, root, projectKey string) ([]workapp.ChoiceOption, error) {
	projects, err := config.LoadProjectsConfigChecked(config.ResolveRoot(root))
	if err != nil {
		return nil, err
	}
	for _, project := range projects.Projects {
		if project.Key != projectKey {
			continue
		}
		choices := make([]workapp.ChoiceOption, 0, len(project.Project.Repositories))
		for _, repository := range project.Project.Repositories {
			choices = append(choices, workapp.ChoiceOption{Value: repository.Key, Label: l10n.M("prompt.choice.value", l10n.A("value", repository.Key))})
		}
		return choices, nil
	}
	return []workapp.ChoiceOption{}, nil
}

func openWorkspace(ctx context.Context, workspacePath, repository, selected string, useLatest bool) (agent.Launch, error) {
	root := workspaceRoot(workspacePath)
	target := workspacePath
	if strings.TrimSpace(repository) != "" {
		manifest, err := workspace.ReadManifest(filepath.Join(workspacePath, workspace.ManifestFile))
		if err != nil {
			return agent.Launch{}, err
		}
		project, found, err := (workspace.FileConfigPort{}).Project(ctx, root, manifest.Project)
		if err != nil {
			return agent.Launch{}, err
		}
		if !found {
			return agent.Launch{}, fmt.Errorf("workspace project is not configured: %s", manifest.Project)
		}
		target, err = workspace.ResolveOpenTarget(workspacePath, manifest, project, repository)
		if err != nil {
			return agent.Launch{}, err
		}
	}
	var choice *agent.Agent
	if strings.TrimSpace(selected) != "" {
		parsed, err := agent.Parse(selected)
		if err != nil {
			return agent.Launch{}, err
		}
		choice = &parsed
	}
	return agent.BuildOpenLaunch(choice, agent.OpenRequest{Root: root, Workspace: target, Continue: useLatest}), nil
}

func workspaceRoot(path string) string {
	clean := filepath.Clean(path)
	for current := clean; ; current = filepath.Dir(current) {
		if filepath.Base(current) == "projects" {
			return filepath.Dir(current)
		}
		parent := filepath.Dir(current)
		if parent == current {
			return config.ResolveRoot("")
		}
	}
}
