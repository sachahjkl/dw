package bootstrap

import (
	"context"
	"fmt"
	"net/url"
	"os"
	"reflect"
	"runtime"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/buildinfo"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/cockpit"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/dataapp"
	"github.com/sachahjkl/dw/internal/doctor"
	executionapp "github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/tui"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
	"github.com/sachahjkl/dw/internal/workspaceapp"
)

var bootstrapTUIEnglishEntries = []l10n.Entry{
	{ID: "bootstrap.tui.initialize", Text: "Initialize"},
	{ID: "bootstrap.tui.open", Text: "Open"},
	{ID: "bootstrap.tui.preflight", Text: "Preflight"},
	{ID: "bootstrap.tui.sync", Text: "Sync"},
	{ID: "bootstrap.tui.latest", Text: "Update repositories"},
	{ID: "bootstrap.tui.handoff", Text: "Validate handoff"},
	{ID: "bootstrap.tui.commit", Text: "Commit preview"},
	{ID: "bootstrap.tui.finish-preview", Text: "Finish preview"},
	{ID: "bootstrap.tui.finish", Text: "Finish"},
	{ID: "bootstrap.tui.teardown-preview", Text: "Teardown preview"},
	{ID: "bootstrap.tui.teardown", Text: "Teardown"},
	{ID: "bootstrap.tui.start-preview", Text: "Start preview"},
	{ID: "bootstrap.tui.start", Text: "Start"},
	{ID: "bootstrap.tui.show", Text: "Show"},
	{ID: "bootstrap.tui.context", Text: "Context"},
	{ID: "bootstrap.tui.set-state", Text: "Set state"},
	{ID: "bootstrap.tui.open-url", Text: "Open URL"},
	{ID: "bootstrap.tui.catalog", Text: "Catalog"},
	{ID: "bootstrap.tui.changelog", Text: "Changelog"},
	{ID: "bootstrap.tui.diff", Text: "Diff"},
	{ID: "bootstrap.tui.doctor", Text: "Doctor"},
	{ID: "bootstrap.tui.doctor-fix", Text: "Fix issues"},
	{ID: "bootstrap.tui.refresh", Text: "Refresh"},
	{ID: "bootstrap.tui.refresh-work", Text: "Refresh all work"},
	{ID: "bootstrap.tui.workspaces", Text: "Workspaces"},
	{ID: "bootstrap.tui.prune", Text: "Prune candidates"},
	{ID: "bootstrap.tui.sign-in", Text: "Sign in"},
	{ID: "bootstrap.tui.config-show", Text: "Show configuration"},
	{ID: "bootstrap.tui.config-doctor", Text: "Diagnose configuration"},
	{ID: "bootstrap.tui.guide", Text: "Getting-started guide"},
	{ID: "bootstrap.tui.agent-doctor", Text: "Diagnose agents"},
	{ID: "bootstrap.tui.agent-opencode", Text: "Use OpenCode"},
	{ID: "bootstrap.tui.agent-cursor", Text: "Use Cursor"},
	{ID: "bootstrap.tui.agent-claude", Text: "Use Claude"},
	{ID: "bootstrap.tui.agent-codex", Text: "Use Codex"},
	{ID: "bootstrap.tui.agent-codex-cli", Text: "Use Codex CLI"},
	{ID: "bootstrap.tui.agent-copilot", Text: "Use Copilot"},
	{ID: "bootstrap.tui.color-auto", Text: "Automatic color"},
	{ID: "bootstrap.tui.color-always", Text: "Always use color"},
	{ID: "bootstrap.tui.color-never", Text: "Never use color"},
}

const (
	actionOpenURL action.ID = "bootstrap.open-url"
	actionGuide   action.ID = "bootstrap.guide"
)

type openURLRequest struct {
	URL string `json:"url"`
}

func (openURLRequest) ActionID() action.ID { return actionOpenURL }

type externalResult struct {
	URL string `json:"url"`
}

func (externalResult) ActionID() action.ID { return actionOpenURL }

type guideRequest struct{}

func (guideRequest) ActionID() action.ID { return actionGuide }

type guideResult struct {
	Version string `json:"version"`
}

func (guideResult) ActionID() action.ID { return actionGuide }

func bootstrapHandlers() []action.Handler {
	return []action.Handler{
		action.HandlerFunc{Action: actionOpenURL, ExecuteFunc: func(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
			value, ok := request.(openURLRequest)
			if !ok || strings.TrimSpace(value.URL) == "" {
				return nil, fmt.Errorf("bootstrap.invalid-open-url")
			}
			location, err := url.Parse(value.URL)
			if err != nil || location.Host == "" || location.Scheme != "http" && location.Scheme != "https" {
				return nil, fmt.Errorf("bootstrap.invalid-open-url")
			}
			return externalResult(value), nil
		}},
		action.HandlerFunc{Action: actionGuide, ExecuteFunc: func(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
			if _, ok := request.(guideRequest); !ok {
				return nil, fmt.Errorf("bootstrap.invalid-guide-request")
			}
			return guideResult{Version: buildinfo.Informational()}, nil
		}},
	}
}

func tuiLabel(localizer l10n.Localizer, id l10n.ID) string { return localizer.Text(id) }

func menuAction(relation cockpit.Relation, labelID l10n.ID, _, _ string, request action.Request, localizer l10n.Localizer) cockpit.Operation {
	label := tuiLabel(localizer, labelID)
	return cockpit.Operation{Relation: relation, Label: label, Description: label, Active: true, Request: request}
}

func bindOperations(subject cockpit.ResourceRef, operations []cockpit.Operation) []cockpit.Operation {
	for index := range operations {
		operations[index].Subject = subject
	}
	return operations
}

type tuiRequestBuilder struct {
	routes  *controller.Registry
	grammar *spec.Command
	root    string
}

func (builder tuiRequestBuilder) Build(_ context.Context, request action.Request) (action.Request, error) {
	if form, ok := request.(tui.FormRequest); ok {
		arguments, err := tuiArguments(form, builder.root)
		if err != nil {
			return nil, err
		}
		invocation, parseErr := parse.Parse(builder.grammar, arguments)
		if parseErr != nil {
			return nil, parseErr
		}
		route, found := builder.routes.Route(invocation.Command.Key)
		if !found || route.Build == nil {
			return nil, fmt.Errorf("bootstrap.tui-route-unavailable:%s", invocation.Command.Key)
		}
		request, err = route.Build(invocation)
		if err != nil {
			return nil, err
		}
	}
	return scopeTUIRoot(request, builder.root), nil
}

func scopeTUIRoot(request action.Request, root string) action.Request {
	switch value := request.(type) {
	case doctor.Request:
		value.Root = root
		return value
	case dataapp.CatalogRequest:
		value.Selection.Root = root
		return value
	case dataapp.DescribeRequest:
		value.Selection.Root = root
		return value
	case dataapp.QueryRequest:
		value.Selection.Root = root
		return value
	default:
		return request
	}
}

func runTUI(services *services, routes *controller.Registry, grammar *spec.Command) func(context.Context, string, controller.Execution) error {
	return func(ctx context.Context, explicitRoot string, cliExecution controller.Execution) error {
		root := config.ResolveRoot(explicitRoot)
		contextForRender := console.NewRenderContext(cliExecution.Policy, cliExecution.Localizer)
		actor := cliExecution.Actor
		actor.Origin = executionapp.OriginTUI
		cockpitService, err := newCockpitService(services, cliExecution.Localizer)
		if err != nil {
			return err
		}
		return tui.Run(ctx, tui.Dependencies{
			Root:           root,
			Executor:       services.executor,
			Actor:          actor,
			RequestBuilder: tuiRequestBuilder{routes: routes, grammar: grammar, root: root}.Build,
			Cockpit:        cockpitService,
			ProjectEvent: func(envelope action.EventEnvelope) (tui.LogLevel, string, string) {
				line, _, err := cliExecution.Console.RenderEvent(contextForRender, envelope)
				if err != nil {
					return tui.ErrorLevel, string(envelope.Action), err.Error()
				}
				return tui.InfoLevel, string(envelope.Action), line
			},
			ProjectResult: func(result action.Result) []string {
				if _, ok := result.(externalResult); ok {
					return nil
				}
				output, err := cliExecution.Console.Results.Render(contextForRender, result.ActionID(), result)
				if err != nil {
					return []string{err.Error()}
				}
				return console.Lines(output)
			},
			ProjectPage: func(result action.Result) (console.Page, bool, error) {
				return cliExecution.Console.Results.ProjectPage(result.ActionID(), result)
			},
			ProjectExternal: projectExternal,
			ProjectState:    projectState,
			Localizer:       cliExecution.Localizer,
			Input:           cliExecution.Policy.Streams.Stdin,
			Output:          cliExecution.Policy.Streams.Stdout,
		})
	}
}

func newCockpitService(services *services, localizer l10n.Localizer) (*cockpit.Service, error) {
	return cockpit.New(snapshotLoader(services, localizer), workLoader(services, localizer), pullRequestLoader(services, localizer))
}

func snapshotLoader(services *services, localizer l10n.Localizer) cockpit.SnapshotLoader {
	return func(ctx context.Context, explicitRoot string) (cockpit.Snapshot, error) {
		root := config.ResolveRoot(explicitRoot)
		status := config.Status(root)
		settings := config.LoadUserSettings()
		rootRef := cockpit.ResourceRef{Kind: cockpit.ResourceRoot, Root: root, Key: root}
		snapshot := cockpit.Snapshot{
			Ref:              rootRef,
			Root:             root,
			NeedsInit:        !status.Initialized,
			DefaultAgent:     string(config.DefaultAgent(root)),
			ColorMode:        string(config.NormalizeColorMode(settings.Color)),
			ProjectProviders: make(map[string]string),
			States:           mustCompletionStates(root),
			SecretKeys:       config.SecretKeyValues(root),
			Environment:      environmentNames(),
		}
		for _, provider := range services.work.Providers() {
			snapshot.WorkProviders = append(snapshot.WorkProviders, string(provider.Name()))
		}
		for _, provider := range services.data.Providers() {
			snapshot.DataProviders = append(snapshot.DataProviders, string(provider.Name()))
		}
		projects := config.LoadProjectsConfig(root)
		for _, entry := range projects.Projects {
			snapshot.Projects = append(snapshot.Projects, entry.Key)
			resolved, found := config.ResolveProject(projects, entry.Key)
			if !found {
				continue
			}
			snapshot.ProjectProviders[entry.Key] = config.ResolveWorkProvider(root, entry.Key)
			for _, repository := range resolved.Repositories {
				snapshot.Repositories = appendUnique(snapshot.Repositories, repository.Key)
			}
		}
		snapshot.ProjectCount = len(snapshot.Projects)
		snapshot.RepositoryCount = len(snapshot.Repositories)
		for _, summary := range workspace.Discover(root) {
			item := cockpit.Workspace{Ref: cockpit.ResourceRef{Kind: cockpit.ResourceWorkspace, Root: root, Project: summary.Manifest.Project, Key: summary.Path}, Path: summary.Path, Project: summary.Manifest.Project, Type: summary.Manifest.Type, Slug: summary.Manifest.Slug, Branch: summary.Manifest.BranchName, Repositories: append([]string(nil), summary.Manifest.Repositories...)}
			item.WorkItems = summary.Manifest.AllKnownWorkItemIDs()
			item.Operations = bindOperations(item.Ref, workspaceActions(localizer, root, config.ResolveWorkProvider(root, item.Project), item))
			snapshot.Workspaces = append(snapshot.Workspaces, item)
		}
		snapshot.PruneCandidates = len(workspace.PruneCandidates(root, "", nil))
		if status.Initialized {
			if report, err := services.dataApplication.List(root, ""); err == nil {
				for _, entry := range report.Entries {
					project := ""
					if entry.Project != nil {
						project = *entry.Project
					}
					reference := cockpit.ResourceRef{Kind: cockpit.ResourceDataSource, Root: root, Project: project, Key: entry.Database}
					operations := []cockpit.Operation{{Relation: cockpit.Relation(tui.DataCatalogSlot), Label: tuiLabel(localizer, "bootstrap.tui.catalog"), Active: true, Request: dataapp.CatalogRequest{Selection: dataapp.Selection{Root: root, Project: project, Source: entry.Database, Provider: entry.Provider}}}}
					snapshot.DataSources = append(snapshot.DataSources, cockpit.DataSource{Ref: reference, Project: project, Key: entry.Database, Provider: entry.Provider, Operations: bindOperations(reference, operations)})
				}
			}
		}
		doctorReport, err := services.doctor.RunAtRoot(ctx, root, false)
		if err == nil {
			snapshot.DoctorOK = doctorReport.Passed()
		}
		doctorAction := cockpit.Operation{Relation: cockpit.RelationDoctor, Subject: rootRef, Label: tuiLabel(localizer, "bootstrap.tui.doctor"), Active: true, Request: doctor.Request{Root: root}}
		doctorFixAction := cockpit.Operation{Relation: cockpit.RelationDoctorFix, Subject: rootRef, Label: tuiLabel(localizer, "bootstrap.tui.doctor-fix"), Active: true, Request: doctor.Request{Root: root, Fix: true}}
		initializeAction := cockpit.Operation{Relation: cockpit.RelationInitialize, Subject: rootRef, Label: tuiLabel(localizer, "bootstrap.tui.initialize"), Active: true, Request: config.InitRequest{Root: root, Profile: "default"}}
		snapshot.Operations = []cockpit.Operation{doctorAction, doctorFixAction, initializeAction}
		snapshot.Cockpit = []cockpit.CockpitItem{{Ref: rootRef, Section: "system", Title: doctorAction.Label, Status: strconv.FormatBool(snapshot.DoctorOK), Primary: doctorAction}}
		configurationActions := []cockpit.Operation{
			menuAction(cockpit.RelationViewConfiguration, "bootstrap.tui.config-show", "s", "configuration", config.ShowRequest{Root: root}, localizer),
			menuAction(cockpit.RelationValidateConfig, "bootstrap.tui.config-doctor", "d", "configuration", config.DoctorRequest{Root: root}, localizer),
		}
		if status.Initialized {
			defaultProfile := "default"
			configurationActions = append(configurationActions, menuAction(cockpit.RelationRefreshConfig, "bootstrap.tui.refresh", "r", "configuration", config.RefreshRequest{Root: root, Profile: &defaultProfile}, localizer))
		}
		configurationActions = append(configurationActions,
			menuAction(cockpit.RelationShowGuide, "bootstrap.tui.guide", "g", "configuration", guideRequest{}, localizer),
			menuAction(cockpit.RelationValidateAgent, "bootstrap.tui.agent-doctor", "a", "configuration", doctor.AgentRequest{}, localizer),
		)
		snapshot.Operations = append(snapshot.Operations, configurationActions...)
		agentLabels := []l10n.ID{"bootstrap.tui.agent-opencode", "bootstrap.tui.agent-cursor", "bootstrap.tui.agent-claude", "bootstrap.tui.agent-codex", "bootstrap.tui.agent-codex-cli", "bootstrap.tui.agent-copilot"}
		agentRelations := []cockpit.Relation{cockpit.RelationSetAgentOpenCode, cockpit.RelationSetAgentCursor, cockpit.RelationSetAgentClaude, cockpit.RelationSetAgentCodex, cockpit.RelationSetAgentCodexCLI, cockpit.RelationSetAgentCopilot}
		for index, agentChoice := range config.AgentDefaultChoices {
			snapshot.Operations = append(snapshot.Operations, menuAction(agentRelations[index], agentLabels[index], strconv.Itoa(index+1), "default-agent", config.AgentDefaultSetRequest{Root: root, Agent: agentChoice}, localizer))
		}
		colorLabels := []l10n.ID{"bootstrap.tui.color-auto", "bootstrap.tui.color-always", "bootstrap.tui.color-never"}
		colorRelations := []cockpit.Relation{cockpit.RelationSetColorAuto, cockpit.RelationSetColorAlways, cockpit.RelationSetColorNever}
		for index, colorChoice := range config.ColorModeChoices {
			snapshot.Operations = append(snapshot.Operations, menuAction(colorRelations[index], colorLabels[index], strconv.Itoa(index+7), "terminal-color", config.ColorSetRequest{Mode: colorChoice}, localizer))
		}
		if status.Initialized {
			refreshAction := cockpit.Operation{Relation: cockpit.RelationRefresh, Subject: rootRef, Label: tuiLabel(localizer, "bootstrap.tui.refresh"), Active: true, Request: config.RefreshRequest{Root: root}}
			workspaceAction := cockpit.Operation{Relation: cockpit.RelationInspect, Subject: rootRef, Label: tuiLabel(localizer, "bootstrap.tui.workspaces"), Active: true, Request: workspaceapp.StatusRequest{Root: root}}
			snapshot.Operations = append(snapshot.Operations, refreshAction, workspaceAction)
			snapshot.Cockpit = append(snapshot.Cockpit, cockpit.CockpitItem{Ref: rootRef, Section: "work", Title: workspaceAction.Label, Status: strconv.Itoa(len(snapshot.Workspaces)), Primary: workspaceAction})
		}
		if snapshot.PruneCandidates > 0 {
			pruneAction := cockpit.Operation{Relation: cockpit.RelationReviewPrune, Subject: rootRef, Label: tuiLabel(localizer, "bootstrap.tui.prune"), Active: true, Risk: cockpit.RiskPreview, Request: workapp.PruneRequest{Root: root, NoSync: true}}
			snapshot.Operations = append(snapshot.Operations, pruneAction)
			snapshot.Cockpit = append(snapshot.Cockpit, cockpit.CockpitItem{Ref: rootRef, Section: "work", Title: pruneAction.Label, Status: strconv.Itoa(snapshot.PruneCandidates), Severity: cockpit.RiskPreview, Primary: pruneAction})
		}
		if snapshot.NeedsInit {
			snapshot.InitOperation = &initializeAction
		}
		snapshot.Operations = bindOperations(rootRef, snapshot.Operations)
		return snapshot, nil
	}
}

func workspaceActions(localizer l10n.Localizer, root, provider string, item cockpit.Workspace) []cockpit.Operation {
	selection := workspaceapp.Selection{Root: root, Workspace: stringPointer(item.Path)}
	finishStates := tuiFinishStates(root)
	return []cockpit.Operation{
		{Relation: cockpit.Relation(tui.WorkspaceOpenSlot), Label: tuiLabel(localizer, "bootstrap.tui.open"), Active: true, Risk: cockpit.RiskExternal, Request: workapp.OpenRequest{Provider: provider, Root: root, Workspace: stringPointer(item.Path)}},
		{Relation: cockpit.Relation(tui.WorkspacePreflightSlot), Label: tuiLabel(localizer, "bootstrap.tui.preflight"), Active: true, Request: workspaceapp.PreflightRequest{Selection: selection}},
		{Relation: cockpit.Relation(tui.WorkspaceSyncSlot), Label: tuiLabel(localizer, "bootstrap.tui.sync"), Active: true, Request: workapp.SyncRequest{Provider: provider, Root: root, Workspace: stringPointer(item.Path)}},
		{Relation: cockpit.Relation(tui.WorkspaceLatestSlot), Label: tuiLabel(localizer, "bootstrap.tui.latest"), Active: true, Request: workspaceapp.RepoLatestRequest{Selection: selection, Execute: true}},
		{Relation: cockpit.Relation(tui.WorkspaceHandoffSlot), Label: tuiLabel(localizer, "bootstrap.tui.handoff"), Active: true, Request: workspaceapp.HandoffRequest{Selection: selection}},
		{Relation: cockpit.Relation(tui.WorkspaceCommitSlot), Label: tuiLabel(localizer, "bootstrap.tui.commit"), Active: true, Risk: cockpit.RiskPreview, Request: workspaceapp.CommitRequest{Selection: selection}},
		{Relation: cockpit.Relation(tui.WorkspaceFinishPlanSlot), Label: tuiLabel(localizer, "bootstrap.tui.finish-preview"), Active: true, Risk: cockpit.RiskPreview, Request: workapp.FinishRequest{Provider: provider, Root: root, Workspace: stringPointer(item.Path), FinishStates: finishStates}},
		{Relation: cockpit.Relation(tui.WorkspaceFinishSlot), Label: tuiLabel(localizer, "bootstrap.tui.finish"), Active: true, Risk: cockpit.RiskDestructive, Request: workapp.FinishRequest{Provider: provider, Root: root, Workspace: stringPointer(item.Path), Execute: true, FinishStates: finishStates}},
		{Relation: cockpit.Relation(tui.WorkspaceRemovePlanSlot), Label: tuiLabel(localizer, "bootstrap.tui.teardown-preview"), Active: true, Risk: cockpit.RiskPreview, Request: workspaceapp.TeardownRequest{Selection: selection}},
		{Relation: cockpit.Relation(tui.WorkspaceRemoveSlot), Label: tuiLabel(localizer, "bootstrap.tui.teardown"), Active: true, Risk: cockpit.RiskDestructive, Request: workspaceapp.TeardownRequest{Selection: selection, Execute: true, Approved: true}},
	}
}

func workLoader(services *services, localizer l10n.Localizer) cockpit.WorkLoader {
	return func(ctx context.Context, snapshot cockpit.Snapshot) ([]cockpit.WorkProject, error) {
		result := make([]cockpit.WorkProject, 0, len(snapshot.Projects))
		startStates, createChildren, updateState := tuiStartSettings(snapshot.Root)
		for _, project := range snapshot.Projects {
			provider := config.ResolveWorkProvider(snapshot.Root, project)
			reference := cockpit.ResourceRef{Kind: cockpit.ResourceProject, Root: snapshot.Root, Project: project, Key: project}
			login := cockpit.Operation{
				Relation: cockpit.RelationAuthenticate, Subject: reference, Label: tuiLabel(localizer, "bootstrap.tui.sign-in"),
				Description: "Connect the configured work provider.", Active: true,
				Request: workapp.AuthLoginRequest{Provider: provider, Root: snapshot.Root},
			}
			refresh := cockpit.Operation{
				Relation: cockpit.RelationRefreshWork, Subject: reference, Label: tuiLabel(localizer, "bootstrap.tui.refresh-work"), Active: true,
				Request: workapp.AssignedRequest{Provider: provider, Root: snapshot.Root, Project: project, Top: 20, IncludeFinalStates: true},
			}
			item := cockpit.WorkProject{Ref: reference, Operations: []cockpit.Operation{login, refresh}, Key: project, Label: project, Provider: provider}
			report, err := services.workapp.Assigned(withRoot(ctx, snapshot.Root), workapp.AssignedRequest{Provider: provider, Root: snapshot.Root, Project: project, Top: 20}, nil)
			if err != nil {
				item.Error = console.LocalizedErrorText(localizer, err)
				result = append(result, item)
				continue
			}
			for _, source := range report.Items {
				projected := cockpit.WorkItem{Ref: cockpit.ResourceRef{Kind: cockpit.ResourceWorkItem, Root: snapshot.Root, Project: project, Key: source.ID}, ID: source.ID, Type: stringValue(source.Type), State: stringValue(source.State), Title: stringValue(source.Title), URL: stringValue(source.URL)}
				targetState := ""
				if updateState {
					targetState = startStates[strings.ToLower(strings.TrimSpace(projected.Type))]
				}
				projected.Operations = []cockpit.Operation{
					{Relation: cockpit.Relation(tui.WorkStartPlanSlot), Label: tuiLabel(localizer, "bootstrap.tui.start-preview"), Active: true, Risk: cockpit.RiskPreview, Request: workapp.StartRequest{Provider: provider, Root: snapshot.Root, Project: project, WorkItemIDs: []string{source.ID}, CreateChildTasks: createChildren, States: startStates}},
					{Relation: cockpit.Relation(tui.WorkStartSlot), Label: tuiLabel(localizer, "bootstrap.tui.start"), Active: true, Risk: cockpit.RiskDestructive, Request: workapp.StartRequest{Provider: provider, Root: snapshot.Root, Project: project, WorkItemIDs: []string{source.ID}, CreateChildTasks: createChildren, States: startStates, Execute: true}},
					{Relation: cockpit.Relation(tui.WorkContextSlot), Label: tuiLabel(localizer, "bootstrap.tui.context"), Active: true, Request: workapp.ContextRequest{Provider: provider, Root: snapshot.Root, Project: project, IDs: []string{source.ID}, Mode: workapp.ContextRich}},
					{Relation: cockpit.Relation(tui.WorkItemSlot), Label: tuiLabel(localizer, "bootstrap.tui.show"), Active: true, Request: workapp.ItemShowRequest{Provider: provider, Root: snapshot.Root, Project: project, IDs: []string{source.ID}}},
				}
				if targetState != "" {
					projected.Operations = append(projected.Operations, cockpit.Operation{Relation: cockpit.Relation(tui.WorkSetStateSlot), Label: tuiLabel(localizer, "bootstrap.tui.set-state"), Active: true, Risk: cockpit.RiskDestructive, Request: workapp.StateSetRequest{Request: workapp.StatePlanRequest{Provider: provider, Root: snapshot.Root, Project: project, IDs: []string{source.ID}, State: targetState, History: "tui"}}})
				}
				matches := workspace.WorkspaceValues(snapshot.Root, project, source.ID)
				if len(matches) != 0 {
					projected.Operations = append(projected.Operations, cockpit.Operation{Relation: cockpit.Relation(tui.WorkOpenAgentSlot), Label: tuiLabel(localizer, "bootstrap.tui.open"), Active: true, Risk: cockpit.RiskExternal, Request: workapp.OpenRequest{Provider: provider, Root: snapshot.Root, Project: project, Workspace: stringPointer(matches[0])}})
				}
				if projected.URL != "" {
					projected.Operations = append(projected.Operations, cockpit.Operation{Relation: cockpit.Relation(tui.WorkOpenURLSlot), Label: tuiLabel(localizer, "bootstrap.tui.open-url"), Active: true, Risk: cockpit.RiskExternal, Request: openURLRequest{URL: projected.URL}})
				}
				projected.Operations = bindOperations(projected.Ref, projected.Operations)
				item.Items = append(item.Items, projected)
			}
			result = append(result, item)
		}
		return result, nil
	}
}

func pullRequestLoader(services *services, localizer l10n.Localizer) cockpit.PullRequestLoader {
	return func(ctx context.Context, snapshot cockpit.Snapshot) ([]cockpit.PullRequest, error) {
		projects := config.LoadProjectsConfig(snapshot.Root)
		result := make([]cockpit.PullRequest, 0)
		startStates, _, updateStartState := tuiStartSettings(snapshot.Root)
		if !updateStartState {
			startStates = nil
		}
		finishStates := tuiFinishStates(snapshot.Root)
		for _, project := range snapshot.Projects {
			configured, found := config.ResolveProject(projects, project)
			if !found {
				continue
			}
			provider := config.ResolveWorkProvider(snapshot.Root, project)
			repositories := make([]string, 0, len(configured.Repositories))
			providerRepositories := make([]string, 0, len(configured.Repositories))
			for _, repository := range configured.Repositories {
				repositories = append(repositories, repository.Key)
				providerRepository := repository.Key
				if repository.Repository.ProviderRepository != nil && strings.TrimSpace(*repository.Repository.ProviderRepository) != "" {
					providerRepository = *repository.Repository.ProviderRepository
				}
				providerRepositories = append(providerRepositories, providerRepository)
			}
			report, err := services.workapp.PullRequests(withRoot(ctx, snapshot.Root), workapp.PullRequestsRequest{Provider: provider, Root: snapshot.Root, Project: project, Repositories: providerRepositories}, nil)
			if err != nil {
				result = append(result, cockpit.PullRequest{Provider: provider, Project: project, Error: console.LocalizedErrorText(localizer, err)})
				continue
			}
			for _, source := range report.Items {
				item := cockpit.PullRequest{Ref: cockpit.ResourceRef{Kind: cockpit.ResourcePullRequest, Root: snapshot.Root, Project: project, Key: strconv.FormatInt(source.PullRequestID, 10)}, ID: strconv.FormatInt(source.PullRequestID, 10), Provider: provider, Project: project, Repository: source.Repository, Branch: stringValue(source.SourceRefName), TargetBranch: stringValue(source.TargetRefName), Title: stringValue(source.Title), Draft: source.IsDraft, WorkItems: append([]string(nil), source.WorkItemIDs...), URL: stringValue(source.WebURL)}
				localRepository := source.Repository
				for index, providerRepository := range providerRepositories {
					if providerRepository == source.Repository {
						localRepository = repositories[index]
						break
					}
				}
				matches := workspace.WorkspaceValues(snapshot.Root, project, strings.Join(source.WorkItemIDs, ","))
				if len(matches) != 0 {
					item.Workspace = matches[0]
				}
				if item.Workspace == "" {
					item.Operations = append(item.Operations,
						cockpit.Operation{Relation: cockpit.Relation(tui.PRStartPlanSlot), Label: tuiLabel(localizer, "bootstrap.tui.start-preview"), Active: true, Risk: cockpit.RiskPreview, Request: workapp.StartPullRequestRequest{Provider: provider, Root: snapshot.Root, Project: project, PullRequestID: source.PullRequestID, Repositories: []string{localRepository}, ProviderRepositories: []string{source.Repository}, States: startStates}},
						cockpit.Operation{Relation: cockpit.Relation(tui.PRStartSlot), Label: tuiLabel(localizer, "bootstrap.tui.start"), Active: true, Risk: cockpit.RiskDestructive, Request: workapp.StartPullRequestRequest{Provider: provider, Root: snapshot.Root, Project: project, PullRequestID: source.PullRequestID, Repositories: []string{localRepository}, ProviderRepositories: []string{source.Repository}, States: startStates, Execute: true}},
					)
				} else {
					workspaceValue := stringPointer(item.Workspace)
					selection := workspaceapp.Selection{Root: snapshot.Root, Workspace: workspaceValue}
					item.Operations = append(item.Operations,
						cockpit.Operation{Relation: cockpit.Relation(tui.PROpenAgentSlot), Label: tuiLabel(localizer, "bootstrap.tui.open"), Active: true, Risk: cockpit.RiskExternal, Request: workapp.OpenRequest{Provider: provider, Root: snapshot.Root, Workspace: workspaceValue, Repository: localRepository}},
						cockpit.Operation{Relation: cockpit.Relation(tui.PRFinishPlanSlot), Label: tuiLabel(localizer, "bootstrap.tui.finish-preview"), Active: true, Risk: cockpit.RiskPreview, Request: workapp.FinishRequest{Provider: provider, Root: snapshot.Root, Workspace: workspaceValue, CreatePR: true, FinishStates: finishStates}},
						cockpit.Operation{Relation: cockpit.Relation(tui.PRFinishSlot), Label: tuiLabel(localizer, "bootstrap.tui.finish"), Active: true, Risk: cockpit.RiskDestructive, Request: workapp.FinishRequest{Provider: provider, Root: snapshot.Root, Workspace: workspaceValue, Execute: true, CreatePR: true, FinishStates: finishStates}},
						cockpit.Operation{Relation: cockpit.Relation(tui.PRDiffSlot), Label: tuiLabel(localizer, "bootstrap.tui.diff"), Active: true, Risk: cockpit.RiskPreview, Request: workspaceapp.CommitRequest{Selection: selection}},
					)
				}
				item.Operations = append(item.Operations, cockpit.Operation{Relation: cockpit.Relation(tui.PRChangelogSlot), Label: tuiLabel(localizer, "bootstrap.tui.changelog"), Active: true, Request: workapp.ChangelogRequest{Provider: provider, Root: snapshot.Root, Project: project, Source: workapp.ChangelogPullRequests, PullRequestIDs: []int64{source.PullRequestID}, Repositories: []string{source.Repository}}})
				if item.URL != "" {
					item.Operations = append(item.Operations, cockpit.Operation{Relation: cockpit.Relation(tui.PROpenURLSlot), Label: tuiLabel(localizer, "bootstrap.tui.open-url"), Active: true, Risk: cockpit.RiskExternal, Request: openURLRequest{URL: item.URL}})
				}
				item.Operations = bindOperations(item.Ref, item.Operations)
				result = append(result, item)
			}
		}
		return result, nil
	}
}

func tuiArguments(request tui.FormRequest, root string) ([]string, error) {
	values := parameterMap(request.Parameters)
	if request.Action == "secret.get" || request.Action == "secret.delete" || request.Action == "secret.set-from-env" {
		key := textParameter(values, "key")
		if request.Action == "secret.delete" {
			return []string{"secret", "delete", key, "--yes"}, nil
		}
		if request.Action == "secret.set-from-env" {
			return []string{"secret", "set", key, "--from-env", textParameter(values, "fromEnv")}, nil
		}
		if boolParameter(values, "delete") {
			return []string{"secret", "delete", key, "--yes"}, nil
		}
		if boolParameter(values, "setFromEnv") {
			return []string{"secret", "set", key, "--from-env", textParameter(values, "fromEnv")}, nil
		}
		return []string{"secret", "get", key}, nil
	}
	if request.Action == "config.set-root" {
		return []string{"config", "root", "set", textParameter(values, "root")}, nil
	}
	path, found := tuiCommandPath(request.Action)
	if !found {
		return nil, fmt.Errorf("bootstrap.unknown-tui-action:%s", request.Action)
	}
	arguments := append([]string(nil), path...)
	if root != "" {
		arguments = append(arguments, "--root", root)
	}
	positional := tuiPositional(request.Action)
	if positional != "" {
		if value := textParameter(values, positional); value != "" {
			arguments = append(arguments, value)
		}
	}
	for _, parameter := range request.Parameters {
		if parameter.Name == positional {
			continue
		}
		option := tuiOption(request.Action, parameter.Name)
		if option == "" {
			continue
		}
		if flag, ok := parameter.Value.(bool); ok {
			if flag {
				arguments = append(arguments, "--"+option)
			}
			continue
		}
		value := parameterText(parameter.Value)
		if strings.TrimSpace(value) != "" {
			arguments = append(arguments, "--"+option, value)
		}
	}
	if boolParameter(values, "execute") && (request.Action == "workspace.finish" || request.Action == "workspace.teardown" || request.Action == "workspace.prune") {
		arguments = append(arguments, "--yes")
	}
	if request.Action == "work.item.state.set" {
		arguments = append(arguments, "--yes")
	}
	return arguments, nil
}

func tuiCommandPath(id action.ID) ([]string, bool) {
	paths := map[action.ID][]string{
		"workspace.start":       {"workspace", "start"},
		"workspace.pr.start":    {"workspace", "pr", "start"},
		"workspace.finish":      {"workspace", "finish"},
		"workspace.teardown":    {"workspace", "teardown"},
		"workspace.prune":       {"workspace", "prune"},
		"workspace.item.add":    {"workspace", "item", "add"},
		"workspace.item.remove": {"workspace", "item", "remove"},
		"workspace.repo.add":    {"workspace", "repo", "add"},
		"workspace.rename":      {"workspace", "rename"},
		"workspace.open":        {"workspace", "open"},
		"work.item.list":        {"work", "item", "list"},
		"work.item.state.set":   {"work", "item", "state", "set"},
		"data.catalog":          {"data", "catalog"},
		"data.describe":         {"data", "describe"},
		"data.query":            {"data", "query"},
	}
	path, found := paths[id]
	return path, found
}

func tuiPositional(id action.ID) string {
	switch id {
	case "workspace.start", "workspace.item.add", "workspace.item.remove", "work.item.state.set":
		return "workItemIds"
	case "workspace.pr.start":
		return "pullRequest"
	case "workspace.repo.add":
		return "repository"
	case "workspace.rename":
		return "slug"
	case "data.describe":
		return "object"
	default:
		return ""
	}
}

func tuiOption(id action.ID, name string) string {
	if name == "workspaceWorkItemIds" || name == "workItemIds" {
		return "work-item"
	}
	if name == "repositories" {
		if id == "workspace.start" {
			return "only"
		}
		return "repo"
	}
	if name == "repository" {
		return "repo"
	}
	options := map[string]string{"createPr": "create-pr", "skipVerify": "skip-verify", "skipProvider": "skip-provider", "groupByParent": "group-by-parent", "noSync": "no-sync", "maxRows": "max-rows"}
	if value, found := options[name]; found {
		return value
	}
	return strings.ReplaceAll(name, "_", "-")
}

func parameterMap(parameters []tui.Parameter) map[string]any {
	result := make(map[string]any, len(parameters))
	for _, parameter := range parameters {
		result[parameter.Name] = parameter.Value
	}
	return result
}

func textParameter(values map[string]any, name string) string {
	if value, found := values[name]; found {
		return parameterText(value)
	}
	return ""
}

func parameterText(value any) string {
	if value == nil {
		return ""
	}
	reflected := reflect.ValueOf(value)
	if reflected.Kind() != reflect.Slice && reflected.Kind() != reflect.Array {
		return fmt.Sprint(value)
	}
	items := make([]string, 0, reflected.Len())
	for index := 0; index < reflected.Len(); index++ {
		item := strings.TrimSpace(parameterText(reflected.Index(index).Interface()))
		if item != "" {
			items = append(items, item)
		}
	}
	return strings.Join(items, ",")
}

func boolParameter(values map[string]any, name string) bool {
	value, _ := values[name].(bool)
	return value
}

func projectExternal(result action.Result) (tui.ExternalProcess, bool) {
	if external, ok := result.(externalResult); ok {
		switch runtime.GOOS {
		case "windows":
			return tui.ExternalProcess{Program: "rundll32", Arguments: []string{"url.dll,FileProtocolHandler", external.URL}}, true
		default:
			return tui.ExternalProcess{Program: "xdg-open", Arguments: []string{external.URL}}, true
		}
	}
	report, ok := result.(workapp.OpenReport)
	if !ok {
		return tui.ExternalProcess{}, false
	}
	if report.Launch == nil {
		return tui.ExternalProcess{}, false
	}
	launch := *report.Launch
	environment := make([]string, 0, len(launch.Environment))
	for _, variable := range launch.Environment {
		environment = append(environment, variable.Name+"="+variable.Value)
	}
	return tui.ExternalProcess{Program: launch.FileName, Arguments: launch.Arguments, Directory: launch.WorkingDirectory, Env: environment}, true
}

func projectState(result action.Result) *tui.StateEffect {
	switch value := result.(type) {
	case config.InitReport:
		return &tui.StateEffect{Root: &value.Root, Initialized: true}
	case config.RootSetReport:
		return &tui.StateEffect{Root: &value.Root}
	case config.ColorSetReport:
		mode := string(value.Mode)
		return &tui.StateEffect{ColorMode: &mode}
	case config.AgentDefaultSetReport:
		agentName := string(value.Agent)
		return &tui.StateEffect{DefaultAgent: &agentName}
	default:
		return nil
	}
}

func tuiStartSettings(root string) (map[string]string, bool, bool) {
	states := map[string]string{"user story": "En réalisation", "anomalie": "En réalisation", "bug": "En développement", "activite": "En développement", "task": "En développement", "tache": "En développement"}
	options := config.LoadWorkflowConfig(config.ResolveRoot(root)).TaskStart
	if options == nil {
		return states, false, true
	}
	createChildren := options.CreateChildTasks != nil && *options.CreateChildTasks
	updateState := options.UpdateWorkItemState == nil || *options.UpdateWorkItemState
	for _, configured := range []struct {
		keys  []string
		value *string
	}{
		{[]string{"user story"}, options.UserStoryState},
		{[]string{"anomalie"}, options.AnomalyState},
		{[]string{"bug", "activite"}, options.BugState},
		{[]string{"task", "tache"}, options.TaskState},
	} {
		if configured.value != nil && strings.TrimSpace(*configured.value) != "" {
			for _, key := range configured.keys {
				states[key] = *configured.value
			}
		}
	}
	if !updateState {
		return nil, createChildren, false
	}
	return states, createChildren, true
}

func tuiFinishStates(root string) map[string]string {
	states := map[string]string{"bug": "PR en attente", "activite": "PR en attente", "task": "PR en attente", "tache": "PR en attente"}
	options := config.LoadWorkflowConfig(config.ResolveRoot(root)).TaskFinish
	if options == nil {
		return states
	}
	for _, configured := range []struct {
		keys  []string
		value *string
	}{
		{[]string{"bug", "activite"}, options.BugState},
		{[]string{"task", "tache"}, options.TaskState},
	} {
		if configured.value != nil && strings.TrimSpace(*configured.value) != "" {
			for _, key := range configured.keys {
				states[key] = *configured.value
			}
		}
	}
	if options.UpdateWorkItemState != nil && !*options.UpdateWorkItemState {
		return nil
	}
	return states
}

func mustCompletionStates(root string) []string {
	values, _ := completionStates(root)
	return values
}

func environmentNames() []string {
	result := make([]string, 0)
	for _, entry := range os.Environ() {
		name, _, _ := strings.Cut(entry, "=")
		result = appendUnique(result, name)
	}
	return result
}

func appendUnique(values []string, value string) []string {
	for _, existing := range values {
		if existing == value {
			return values
		}
	}
	return append(values, value)
}

func stringPointer(value string) *string { return &value }
func stringValue(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}
