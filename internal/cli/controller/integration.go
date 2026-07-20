package controller

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/complete"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/dbcompat"
	"github.com/sachahjkl/dw/internal/doctor"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/update"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
)

// Integration contains the direct-only CLI dependencies. Domain operations use
// the shared action dispatcher; version, guide, completion, and TUI deliberately
// bypass it because they either produce protocol text or own the terminal.
type Integration struct {
	Root                 *spec.Command
	InformationalVersion string
	PackageVersion       string
	Completion           complete.Resolver
	RunTUI               func(context.Context, string, Execution) error
}

// RegisterRoutes registers every leaf in spec.Root in grammar order. It fails
// immediately when a direct dependency is absent or grammar and routes drift.
func RegisterRoutes(registry *Registry, integration Integration) error {
	if registry == nil || integration.Root == nil || integration.Completion == nil || integration.RunTUI == nil {
		return fmt.Errorf("cli.invalid-route-integration")
	}
	register := func(route Route) error { return registry.Register(route) }
	direct := directRoutes(integration)
	for _, route := range []Route{
		direct["version"], direct["guide"],
		buildRoute("doctor", buildDoctor, humanProject),
		buildRoute("init", buildInit, humanProject),
		buildRoute("refresh", buildRefresh, humanProject),
		direct["tui"],
		buildRoute("agent.context", buildAgentContext, humanProject),
		buildRoute("agent.open", buildAgentOpen, jsonOptionProject),
		buildRoute("agent.config", buildAgentConfig, humanProject),
		buildRoute("agent.show", buildAgentShow, humanProject),
		buildRoute("agent.default.set", buildAgentDefaultSet, humanProject),
		buildRoute("agent.doctor", buildAgentDoctor, humanProject),
		authLoginRoute(),
		buildRoute("auth.status", buildAuthStatus, humanProject),
		buildRoute("auth.logout", buildAuthLogout, humanProject),
		direct["completion.show"], direct["completion.generate"], direct["completion.install"], direct["completion.complete"],
		buildRoute("config.show", buildConfigShow, jsonOptionProject),
		buildRoute("config.doctor", buildConfigDoctor, jsonOptionProject),
		buildRoute("config.root.set", buildConfigRootSet, humanProject),
		buildRoute("config.color.set", buildConfigColorSet, humanProject),
		assignedRoute(),
		buildRoute("ado.prs", buildADOPRs, pullRequestsProject),
		buildRoute("ado.changelog", buildADOChangelog, changelogProject),
		buildRoute("ado.item.show", buildADOItemShow, workItemsProject),
		stateSetRoute(),
		buildRoute("ado.context.show", buildADOContext, contextProject),
		buildRoute("ado.context.ai", buildADOAIContext, aiContextProject),
		buildRoute("db.list", buildDBList, jsonOptionProject),
		buildRoute("db.collect", buildDBCollect, jsonOptionProject),
		buildRoute("db.guard", buildDBGuard, humanProject),
		buildRoute("db.schema", buildDBSchema, jsonOptionProject),
		buildRoute("db.describe", buildDBDescribe, jsonOptionProject),
		buildRoute("db.query", buildDBQuery, jsonOptionProject),
		buildRoute("secret.list", buildSecretList, jsonOptionProject),
		buildRoute("secret.set", buildSecretSet, humanProject),
		buildRoute("secret.get", buildSecretGet, humanProject),
		buildRoute("secret.delete", buildSecretDelete, humanProject),
		buildRoute("upgrade", buildUpgrade, upgradeProject),
		buildRoute("work.status", buildWorkStatus, humanProject),
		buildRoute("work.list", buildWorkList, workListProject),
		buildRoute("work.current", buildWorkCurrent, jsonOptionProject),
		doingRoute(),
		buildRoute("work.item.add", buildWorkItemAdd, workspacePhaseProject),
		buildRoute("work.item.remove", buildWorkItemRemove, workspacePhaseProject),
		buildRoute("work.task.child.create", buildWorkChild, jsonOptionProject),
		buildRoute("work.open", buildWorkOpen, jsonOptionProject),
		startRoute(),
		buildRoute("work.pr.start", buildWorkPRStart, workspacePhaseProject),
		buildRoute("work.preflight", buildWorkPreflight, jsonOptionProject),
		buildRoute("work.sync", buildWorkSync, jsonOptionProject),
		buildRoute("work.rename", buildWorkRename, workspacePhaseProject),
		buildRoute("work.repo.add", buildWorkRepoAdd, workspacePhaseProject),
		buildRoute("work.repo.latest", buildWorkRepoLatest, repoLatestProject),
		buildRoute("work.commit", buildWorkCommit, workspacePhaseProject),
		finishRoute(),
		buildRoute("work.handoff.validate", buildWorkHandoff, jsonOptionProject),
		teardownRoute(),
		pruneRoute(),
	} {
		if err := register(route); err != nil {
			return err
		}
	}
	return registry.ValidateComplete(integration.Root)
}

func buildRoute(key string, build Builder, project Projector) Route {
	route := Route{Key: key, Build: build, Project: project}
	if routeUsesJSONOption(key) {
		route.Machine = jsonMachine
	}
	if key == "ado.context.ai" {
		route.Machine = func(parse.Values) bool { return true }
	}
	if key == "ado.changelog" {
		route.Machine = func(values parse.Values) bool { return values.Bool("ids_only") || values.String("format") != "" }
	}
	switch key {
	case "work.finish":
		route.Grant = GrantWorkFinish
	case "work.teardown":
		route.Grant = GrantWorkTeardown
	case "work.prune":
		route.Grant = GrantWorkPrune
	}
	route.Status = statusForKey(key)
	return route
}

func routeUsesJSONOption(key string) bool {
	switch key {
	case "agent.open", "config.show", "config.doctor", "ado.assigned", "ado.prs", "ado.item.show", "ado.state.set", "ado.context.show", "db.list", "db.collect", "db.schema", "db.describe", "db.query", "secret.list", "work.list", "work.current", "work.item.add", "work.item.remove", "work.task.child.create", "work.open", "work.start", "work.pr.start", "work.preflight", "work.sync", "work.rename", "work.repo.add", "work.repo.latest", "work.commit", "work.finish", "work.handoff.validate", "work.teardown", "work.prune":
		return true
	default:
		return false
	}
}

func statusForKey(key string) Status {
	switch key {
	case "doctor":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(doctor.Report)
			if ok && !report.Passed() {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	case "agent.doctor":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(doctor.AgentReport)
			if ok && !report.Passed() {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	case "auth.status":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(workapp.AuthStatusReport)
			if ok && !report.Connected {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	case "config.doctor":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(config.ConfigDoctorReport)
			if ok && !report.Passed {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	case "db.guard":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(dbcompat.GuardResult)
			if ok && !report.IsAllowed {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	case "work.preflight":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(WorkspacePreflightResult)
			if ok && report.HasBlockingIssues {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	case "work.handoff.validate":
		return func(result action.ResultEnvelope) console.ExitCode {
			report, ok := result.Result.(WorkspaceHandoffResult)
			if ok && !report.IsValid {
				return console.ExitFailure
			}
			return console.ExitSuccess
		}
	default:
		return nil
	}
}

func directRoutes(integration Integration) map[string]Route {
	root := integration.Root
	version := integration.PackageVersion
	return map[string]Route{
		"version": {Key: "version", Direct: func(_ context.Context, execution Execution, _ *parse.Result) (Outcome, error) {
			return success(console.RenderVersion(version, execution.Localizer, console.NewTheme(execution.Policy.StdoutColor()))), nil
		}},
		"guide": {Key: "guide", Direct: func(_ context.Context, execution Execution, _ *parse.Result) (Outcome, error) {
			text := console.RenderGuide(console.GuideResult{Version: integration.InformationalVersion}, execution.Localizer, console.NewTheme(execution.Policy.StdoutColor()))
			return success(console.TextOutput(console.FormatHuman, text)), nil
		}},
		"tui": {Key: "tui", Direct: func(ctx context.Context, execution Execution, invocation *parse.Result) (Outcome, error) {
			if !execution.Policy.Streams.StdinTTY || !execution.Policy.Streams.StdoutTTY {
				return Outcome{}, console.WithExitCode(fmt.Errorf("cli.tui-requires-terminal"), console.ExitUsage)
			}
			if err := integration.RunTUI(ctx, invocation.Values.String("root"), execution); err != nil {
				return Outcome{}, err
			}
			return success(console.Output{}), nil
		}},
		"completion.show": completionTextRoute("completion.show", func(_ *parse.Result) (string, error) { return complete.Show(root), nil }),
		"completion.generate": completionTextRoute("completion.generate", func(invocation *parse.Result) (string, error) {
			shell, err := complete.ParseShell(invocation.Values.String("shell"))
			if err != nil {
				return "", usage(err)
			}
			return complete.Generate(root, shell)
		}),
		"completion.install": completionTextRoute("completion.install", func(invocation *parse.Result) (string, error) {
			shell, err := complete.ParseShell(invocation.Values.String("shell"))
			if err != nil {
				return "", usage(err)
			}
			return complete.Install(shell)
		}),
		"completion.complete": {Key: "completion.complete", Machine: func(parse.Values) bool { return true }, Direct: func(_ context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
			format, err := complete.ParseFormat(invocation.Values.String("format"))
			if err != nil {
				return Outcome{}, usage(err)
			}
			items, err := complete.CompleteInstalled(root, invocation.Values.Strings("words"), integration.Completion)
			if err != nil {
				return Outcome{}, err
			}
			body, err := complete.Render(format, items)
			if err != nil {
				return Outcome{}, err
			}
			outputFormat := console.FormatRaw
			if format == complete.FormatJSON || format == complete.FormatPowerShell {
				outputFormat = console.FormatJSON
			}
			return success(console.Output{Format: outputFormat, Body: body}), nil
		}},
	}
}

func completionTextRoute(key string, render func(*parse.Result) (string, error)) Route {
	return Route{Key: key, Machine: func(parse.Values) bool { return true }, Direct: func(_ context.Context, _ Execution, invocation *parse.Result) (Outcome, error) {
		text, err := render(invocation)
		if err != nil {
			return Outcome{}, err
		}
		return success(console.TextOutput(console.FormatRaw, text)), nil
	}}
}

func success(output console.Output) Outcome {
	return Outcome{Output: output, Code: console.ExitSuccess}
}
func usage(err error) error                { return console.WithExitCode(err, console.ExitUsage) }
func jsonMachine(values parse.Values) bool { return values.Bool("json") }

func humanProject(_ action.ResultEnvelope, _ *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	return console.FormatHuman, nil, nil
}
func jsonOptionProject(result action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	return marshalProjection(result.Result)
}
func repoLatestProject(result action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if !invocation.Values.Bool("json") {
		return console.FormatHuman, nil, nil
	}
	report, ok := result.Result.(WorkspaceRepoLatestResult)
	if !ok {
		return 0, nil, fmt.Errorf("cli.invalid-result:work.repo.latest:%T", result.Result)
	}
	return marshalProjection(report.Plan)
}
func jsonProject(result action.ResultEnvelope, _ *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	return marshalProjection(result.Result)
}
func upgradeProject(result action.ResultEnvelope, _ *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	report, ok := result.Result.(update.Report)
	if !ok {
		return 0, nil, fmt.Errorf("cli.invalid-result:upgrade:%T", result.Result)
	}
	projection, err := console.UpdateJSONProjection(report)
	if err != nil {
		return 0, nil, err
	}
	return console.FormatHuman, &projection, nil
}
func changelogProject(result action.ResultEnvelope, invocation *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
	if invocation.Values.Bool("ids_only") {
		return console.FormatRaw, nil, nil
	}
	switch invocation.Values.String("format") {
	case "markdown":
		return console.FormatMarkdown, nil, nil
	case "html":
		return console.FormatHTML, nil, nil
	default:
		return console.FormatRaw, nil, nil
	}
}
func marshalProjection(value any) (console.OutputFormat, *console.JSONProjection, error) {
	projection, err := console.JSONProjectionOf(value)
	if err != nil {
		return 0, nil, err
	}
	return console.FormatJSON, &projection, nil
}

func buildDoctor(inv *parse.Result) (action.Request, error) {
	return doctor.Request{Fix: inv.Values.Bool("fix")}, nil
}
func buildInit(inv *parse.Result) (action.Request, error) {
	return config.InitRequest{Root: resolvedRoot(inv.Values), Profile: inv.Values.String("profile"), DryRun: inv.Values.Bool("dry_run"), NoSave: inv.Values.Bool("no_save")}, nil
}
func buildRefresh(inv *parse.Result) (action.Request, error) {
	profile := optional(inv.Values, "profile")
	return config.RefreshRequest{Root: resolvedRoot(inv.Values), Profile: profile}, nil
}
func buildAgentContext(inv *parse.Result) (action.Request, error) {
	return AgentContextRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildAgentOpen(inv *parse.Result) (action.Request, error) { return openRequest(inv, false) }
func buildAgentConfig(inv *parse.Result) (action.Request, error) {
	return config.AgentConfigRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildAgentShow(inv *parse.Result) (action.Request, error) {
	return config.AgentShowRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildAgentDefaultSet(inv *parse.Result) (action.Request, error) {
	return config.AgentDefaultSetRequest{Root: resolvedRoot(inv.Values), Agent: config.Agent(inv.Values.String("agent"))}, nil
}
func buildAgentDoctor(inv *parse.Result) (action.Request, error) {
	var selected *contract.Agent
	if inv.Values.Has("agent") {
		value := contract.Agent(inv.Values.String("agent"))
		if !value.Valid() {
			return nil, usage(fmt.Errorf("cli.invalid-agent:%s", value))
		}
		selected = &value
	}
	return doctor.AgentRequest{Agent: selected}, nil
}
func buildAuthLogin(inv *parse.Result) (action.Request, error) {
	return workapp.AuthLoginRequest{Root: resolvedRoot(inv.Values), Mode: workapp.AuthLoginBrowser}, nil
}
func buildAuthStatus(inv *parse.Result) (action.Request, error) {
	return workapp.AuthStatusRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildAuthLogout(inv *parse.Result) (action.Request, error) {
	return workapp.AuthLogoutRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildConfigShow(inv *parse.Result) (action.Request, error) {
	return config.ShowRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildConfigDoctor(inv *parse.Result) (action.Request, error) {
	return config.DoctorRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildConfigRootSet(inv *parse.Result) (action.Request, error) {
	return config.RootSetRequest{Path: inv.Values.String("path")}, nil
}
func buildConfigColorSet(inv *parse.Result) (action.Request, error) {
	mode, err := config.ParseColorMode(inv.Values.String("mode"))
	if err != nil {
		return nil, usage(err)
	}
	return config.ColorSetRequest{Mode: mode}, nil
}
func buildADOAssigned(inv *parse.Result) (action.Request, error) {
	project := inv.Values.String("project")
	if inv.Values.Bool("json") && project == "" {
		return nil, usage(fmt.Errorf("cli.project-required"))
	}
	return workapp.AssignedRequest{Root: resolvedRoot(inv.Values), Project: project, Top: int(inv.Values.Int("top")), IncludeFinalStates: inv.Values.Bool("all"), GroupByParent: inv.Values.Bool("group_by_parent")}, nil
}
func buildADOPRs(inv *parse.Result) (action.Request, error) {
	root, project := resolvedRoot(inv.Values), inv.Values.String("project")
	repositories := split(inv.Values.String("repo"))
	if len(repositories) == 0 {
		repositories = configuredRepositories(root, project)
	}
	return workapp.PullRequestsRequest{Root: root, Project: project, Repositories: repositories}, nil
}
func buildADOItemShow(inv *parse.Result) (action.Request, error) {
	return workapp.ItemShowRequest{Root: resolvedRoot(inv.Values), Project: inv.Values.String("project"), IDs: split(inv.Values.String("id"))}, nil
}
func buildADOStateSet(inv *parse.Result) (action.Request, error) {
	history := inv.Values.String("history")
	if history == "" {
		history = "dw ado state set"
	}
	request := workapp.StatePlanRequest{Root: resolvedRoot(inv.Values), Project: inv.Values.String("project"), IDs: split(inv.Values.String("id")), State: inv.Values.String("state"), History: history}
	if inv.Values.Bool("yes") {
		return workapp.StateSetRequest{Request: request}, nil
	}
	return request, nil
}
func buildADOContext(inv *parse.Result) (action.Request, error) {
	return workapp.ContextRequest{Root: resolvedRoot(inv.Values), Project: inv.Values.String("project"), IDs: split(inv.Values.String("id")), Summary: inv.Values.Bool("summary"), Comments: int(inv.Values.Int("comments")), IncludeComments: inv.Values.Int("comments") > 0, Mode: workapp.ContextRaw}, nil
}
func buildADOAIContext(inv *parse.Result) (action.Request, error) {
	request := workapp.ContextRequest{Root: resolvedRoot(inv.Values), Organization: inv.Values.String("organization"), Project: inv.Values.String("project"), IDs: split(inv.Values.String("id")), Summary: inv.Values.Bool("summary"), Comments: int(inv.Values.Int("comments")), IncludeComments: inv.Values.Bool("include_comments"), Mode: workapp.ContextRich}
	return workapp.AIContextRequest{ContextRequest: request}, nil
}
func buildADOChangelog(inv *parse.Result) (action.Request, error) {
	request := workapp.ChangelogRequest{Root: resolvedRoot(inv.Values), Project: inv.Values.String("project"), GroupByParent: inv.Values.Bool("group_by_parent"), Table: inv.Values.Bool("table"), IDsOnly: inv.Values.Bool("ids_only"), Repositories: split(inv.Values.String("repo"))}
	switch inv.Values.String("format") {
	case "markdown":
		request.Format = workapp.ChangelogMarkdown
	case "html":
		request.Format = workapp.ChangelogHTML
	default:
		request.Format = workapp.ChangelogRaw
	}
	ids := inv.Values.String("ids")
	if inv.Values.Bool("from_pr") {
		request.Source = workapp.ChangelogPullRequests
		for _, value := range split(ids) {
			id, err := strconv.ParseInt(value, 10, 64)
			if err != nil {
				return nil, usage(fmt.Errorf("cli.invalid-pull-request:%s", value))
			}
			request.PullRequestIDs = append(request.PullRequestIDs, id)
		}
	} else if inv.Values.Bool("from_git") {
		request.Source = workapp.ChangelogGitRange
		request.GitFrom, request.GitTo = ids, inv.Values.String("git_to")
	} else {
		request.Source = workapp.ChangelogWorkItems
		request.WorkItemIDs = split(ids)
	}
	if request.Source == workapp.ChangelogPullRequests && len(request.Repositories) == 0 {
		request.Repositories = configuredRepositories(request.Root, request.Project)
	}
	return request, nil
}

func dbSelection(values parse.Values) dbcompat.Selection {
	return dbcompat.Selection{Root: resolvedRoot(values), Project: values.String("project"), Database: values.String("database"), Env: values.String("env")}
}
func buildDBList(inv *parse.Result) (action.Request, error) {
	return dbcompat.ListRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildDBCollect(inv *parse.Result) (action.Request, error) {
	return dbcompat.CollectRequest{Root: resolvedRoot(inv.Values), Save: inv.Values.Bool("save")}, nil
}
func buildDBGuard(inv *parse.Result) (action.Request, error) {
	return dbcompat.GuardRequest{SQL: inv.Values.String("sql")}, nil
}
func buildDBSchema(inv *parse.Result) (action.Request, error) {
	return dbcompat.SchemaRequest{Selection: dbSelection(inv.Values)}, nil
}
func buildDBDescribe(inv *parse.Result) (action.Request, error) {
	return dbcompat.DescribeRequest{Selection: dbSelection(inv.Values), Table: inv.Values.String("table")}, nil
}
func buildDBQuery(inv *parse.Result) (action.Request, error) {
	sql := strings.TrimSpace(inv.Values.String("sql"))
	parts := inv.Values.Strings("sql_parts")
	if sql != "" && len(parts) != 0 {
		return nil, usage(fmt.Errorf("cli.db-query-conflicting-sql"))
	}
	if sql == "" {
		sql = strings.Join(parts, " ")
	}
	if sql == "" {
		return nil, usage(fmt.Errorf("cli.db-query-missing-sql"))
	}
	var maximum *int
	if inv.Values.Has("max_rows") {
		value := int(inv.Values.Int("max_rows"))
		maximum = &value
	}
	return dbcompat.QueryRequest{Selection: dbSelection(inv.Values), SQL: sql, MaxRows: maximum}, nil
}
func buildSecretList(inv *parse.Result) (action.Request, error) {
	return secret.ListRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildSecretSet(inv *parse.Result) (action.Request, error) {
	request := secret.SetRequest{Key: contract.SecretKey(inv.Values.String("key"))}
	if inv.Values.Has("value") {
		value := contract.NewSecretValue(inv.Values.String("value"))
		request.Value = &value
	}
	if inv.Values.Has("from_env") {
		value := contract.EnvironmentVariable(inv.Values.String("from_env"))
		request.Environment = &value
	}
	return request, nil
}
func buildSecretGet(inv *parse.Result) (action.Request, error) {
	return secret.GetRequest{Key: contract.SecretKey(inv.Values.String("key"))}, nil
}
func buildSecretDelete(inv *parse.Result) (action.Request, error) {
	return secret.DeleteRequest{Key: contract.SecretKey(inv.Values.String("key")), Confirmed: inv.Values.Bool("yes")}, nil
}
func buildUpgrade(inv *parse.Result) (action.Request, error) {
	executable, err := os.Executable()
	if err != nil {
		return nil, err
	}
	settings := update.DefaultConfig()
	if configured := config.LoadWorkflowConfig(config.ResolveRoot("")).Updates; configured != nil {
		settings = *configured
	}
	return update.Request{Check: inv.Values.Bool("check"), RID: inv.Values.String("rid"), Config: settings, ExecutablePath: executable}, nil
}

func buildWorkStatus(inv *parse.Result) (action.Request, error) {
	return WorkspaceStatusRequest{Root: resolvedRoot(inv.Values)}, nil
}
func buildWorkList(inv *parse.Result) (action.Request, error) {
	return WorkspaceListRequest{Root: resolvedRoot(inv.Values), Project: optional(inv.Values, "project"), WorkItemIDs: split(inv.Values.String("work_item"))}, nil
}
func buildWorkCurrent(_ *parse.Result) (action.Request, error) { return WorkspaceCurrentRequest{}, nil }
func buildWorkItemAdd(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	ids := split(inv.Values.String("work_item_ids"))
	if inv.Values.Bool("json") && len(ids) == 0 {
		return nil, usage(fmt.Errorf("cli.work-item-ids-required"))
	}
	return WorkspaceItemAddRequest{Selection: selection, IDs: ids, SkipWork: inv.Values.Bool("skip_ado"), Type: inv.Values.String("type"), Title: inv.Values.String("title"), State: inv.Values.String("state"), Execute: inv.Values.Bool("execute")}, nil
}
func buildWorkItemRemove(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	ids := split(inv.Values.String("work_item_ids"))
	if inv.Values.Bool("json") && len(ids) == 0 {
		return nil, usage(fmt.Errorf("cli.work-item-ids-required"))
	}
	return WorkspaceItemRemoveRequest{Selection: selection, IDs: ids, Execute: inv.Values.Bool("execute")}, nil
}
func buildWorkChild(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	return workapp.ChildRequest{Root: selection.Root, Project: selection.Project, Workspace: selection.Workspace, WorkItemIDs: selection.IDs, Continue: selection.Continue, Repository: inv.Values.String("repo"), Title: inv.Values.String("title")}, nil
}
func buildWorkOpen(inv *parse.Result) (action.Request, error) {
	return openRequest(inv, inv.Values.Bool("json"))
}
func openRequest(inv *parse.Result, resolveOnly bool) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	var pullRequest *int64
	if value := strings.TrimSpace(inv.Values.String("pr")); value != "" {
		parsed, err := strconv.ParseInt(value, 10, 64)
		if err != nil || parsed <= 0 {
			return nil, usage(fmt.Errorf("cli.invalid-pull-request:%s", value))
		}
		pullRequest = &parsed
	}
	return workapp.OpenRequest{Root: selection.Root, Project: selection.Project, Workspace: selection.Workspace, WorkItemIDs: selection.IDs, Continue: selection.Continue, PullRequestID: pullRequest, Repository: inv.Values.String("repo"), Agent: inv.Values.String("agent"), ResolveOnly: resolveOnly}, nil
}
func buildWorkStart(inv *parse.Result) (action.Request, error) {
	root := resolvedRoot(inv.Values)
	ids := split(inv.Values.String("work_item_id"))
	if inv.Values.Bool("json") && len(ids) == 0 {
		return nil, usage(fmt.Errorf("cli.work-item-ids-required"))
	}
	states, createChildren, updateState := taskStartSettings(root)
	if !updateState {
		states = nil
	}
	return workapp.StartRequest{Root: root, Project: inv.Values.String("project"), WorkItemIDs: ids, TaskID: optional(inv.Values, "task"), Type: inv.Values.String("type"), Repositories: split(inv.Values.String("only")), Slug: inv.Values.String("slug"), SkipWork: inv.Values.Bool("skip_ado"), WithActiveChildren: inv.Values.Bool("with_active_children"), CreateChildTasks: inv.Values.Bool("create_child_tasks") || createChildren, Execute: inv.Values.Bool("execute"), States: states}, nil
}
func buildWorkPRStart(inv *parse.Result) (action.Request, error) {
	id, err := strconv.ParseInt(inv.Values.String("pull_request_id"), 10, 64)
	if err != nil {
		return nil, usage(fmt.Errorf("cli.invalid-pull-request:%s", inv.Values.String("pull_request_id")))
	}
	root, project := resolvedRoot(inv.Values), inv.Values.String("project")
	local, provider := configuredRepositoryPairs(root, project)
	requested := split(inv.Values.String("repo"))
	if len(requested) != 0 {
		local, provider = requested, requested
	}
	states, _, updateState := taskStartSettings(root)
	if !updateState {
		states = nil
	}
	return workapp.StartPullRequestRequest{Root: root, Project: project, PullRequestID: id, Repositories: local, ProviderRepositories: provider, Type: inv.Values.String("type"), Slug: inv.Values.String("slug"), Execute: inv.Values.Bool("execute"), States: states}, nil
}
func buildWorkPreflight(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	return WorkspacePreflightRequest{Selection: selection, Files: inv.Values.Strings("ai_context_file")}, nil
}
func buildWorkSync(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	return workapp.SyncRequest{Root: selection.Root, Project: selection.Project, Workspace: selection.Workspace, WorkItemIDs: selection.IDs, Continue: selection.Continue}, nil
}
func buildWorkRename(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	return WorkspaceRenameRequest{Selection: selection, Slug: inv.Values.String("slug"), Execute: inv.Values.Bool("execute")}, nil
}
func buildWorkRepoAdd(inv *parse.Result) (action.Request, error) {
	repository := inv.Values.String("repo")
	if inv.Values.Bool("json") && strings.TrimSpace(repository) == "" {
		return nil, usage(fmt.Errorf("cli.work-repository-required"))
	}
	return WorkspaceRepoAddRequest{Selection: WorkspaceSelection{Root: resolvedRoot(inv.Values), Workspace: optional(inv.Values, "workspace")}, Repository: repository, Execute: inv.Values.Bool("execute")}, nil
}
func buildWorkRepoLatest(inv *parse.Result) (action.Request, error) {
	return WorkspaceRepoLatestRequest{Selection: WorkspaceSelection{Root: resolvedRoot(inv.Values), Workspace: optional(inv.Values, "workspace"), Continue: inv.Values.Bool("continue")}, Repositories: split(inv.Values.String("only")), Execute: !inv.Values.Bool("json")}, nil
}
func buildWorkCommit(inv *parse.Result) (action.Request, error) {
	return WorkspaceCommitRequest{Selection: WorkspaceSelection{Root: resolvedRoot(inv.Values), Workspace: optional(inv.Values, "workspace"), Continue: inv.Values.Bool("continue")}, Message: inv.Values.String("message"), Execute: inv.Values.Bool("execute")}, nil
}
func buildWorkFinish(inv *parse.Result) (action.Request, error) {
	root := resolvedRoot(inv.Values)
	return workapp.FinishRequest{Root: root, Workspace: optional(inv.Values, "workspace"), Continue: inv.Values.Bool("continue"), Execute: inv.Values.Bool("execute"), CreatePR: inv.Values.Bool("create_pr"), Ready: inv.Values.Bool("ready"), SkipVerify: inv.Values.Bool("skip_verify"), SkipWork: inv.Values.Bool("skip_ado"), ForceWithLease: inv.Values.Bool("force_with_lease"), Message: optional(inv.Values, "message"), FinishStates: taskFinishStates(root)}, nil
}
func buildWorkHandoff(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	return WorkspaceHandoffRequest{Selection: selection}, nil
}
func buildWorkTeardown(inv *parse.Result) (action.Request, error) {
	selection, err := workspaceSelection(inv.Values)
	if err != nil {
		return nil, usage(err)
	}
	execute := inv.Values.Bool("execute")
	return WorkspaceTeardownRequest{Selection: selection, Execute: execute, Approved: execute}, nil
}
func buildWorkPrune(inv *parse.Result) (action.Request, error) {
	return workapp.PruneRequest{Root: resolvedRoot(inv.Values), Project: optional(inv.Values, "project"), WorkItemIDs: split(inv.Values.String("work_item")), Execute: inv.Values.Bool("execute"), NoSync: inv.Values.Bool("no_sync")}, nil
}

func taskStartSettings(root string) (map[string]string, bool, bool) {
	states := map[string]string{"user story": "En réalisation", "anomalie": "En réalisation", "bug": "En développement", "activite": "En développement", "task": "En développement", "tache": "En développement"}
	options := config.LoadWorkflowConfig(config.ResolveRoot(root)).TaskStart
	createChildren, updateState := false, true
	if options == nil {
		return states, createChildren, updateState
	}
	if options.CreateChildTasks != nil {
		createChildren = *options.CreateChildTasks
	}
	if options.UpdateWorkItemState != nil {
		updateState = *options.UpdateWorkItemState
	}
	for _, configured := range []struct {
		keys  []string
		value *string
	}{{[]string{"user story"}, options.UserStoryState}, {[]string{"anomalie"}, options.AnomalyState}, {[]string{"bug", "activite"}, options.BugState}, {[]string{"task", "tache"}, options.TaskState}} {
		if configured.value != nil && strings.TrimSpace(*configured.value) != "" {
			for _, key := range configured.keys {
				states[key] = *configured.value
			}
		}
	}
	return states, createChildren, updateState
}

func taskFinishStates(root string) map[string]string {
	states := map[string]string{"bug": "PR en attente", "activite": "PR en attente", "task": "PR en attente", "tache": "PR en attente"}
	options := config.LoadWorkflowConfig(config.ResolveRoot(root)).TaskFinish
	if options == nil {
		return states
	}
	for _, configured := range []struct {
		keys  []string
		value *string
	}{{[]string{"bug", "activite"}, options.BugState}, {[]string{"task", "tache"}, options.TaskState}} {
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

func configuredRepositories(root, project string) []string {
	_, provider := configuredRepositoryPairs(root, project)
	return provider
}

func configuredRepositoryPairs(root, project string) ([]string, []string) {
	if strings.TrimSpace(project) == "" {
		return nil, nil
	}
	configured, found := config.ResolveProject(config.LoadProjectsConfig(config.ResolveRoot(root)), project)
	if !found {
		return nil, nil
	}
	local := make([]string, 0, len(configured.Repositories))
	provider := make([]string, 0, len(configured.Repositories))
	for _, entry := range configured.Repositories {
		if entry.Key == "" {
			continue
		}
		local = append(local, entry.Key)
		value := entry.Key
		if entry.Repository.AzureDevOpsRepository != nil && strings.TrimSpace(*entry.Repository.AzureDevOpsRepository) != "" {
			value = *entry.Repository.AzureDevOpsRepository
		}
		provider = append(provider, value)
	}
	return local, provider
}

func resolvedRoot(values parse.Values) string { return config.ResolveRoot(values.String("root")) }

func optional(values parse.Values, name string) *string {
	if !values.Has(name) {
		return nil
	}
	value := values.String(name)
	return &value
}
func split(value string) []string {
	fields := strings.FieldsFunc(value, func(r rune) bool { return r == ',' || r == ';' })
	result := make([]string, 0, len(fields))
	for _, field := range fields {
		if value := strings.TrimSpace(field); value != "" {
			result = append(result, value)
		}
	}
	return result
}

func workspaceSelection(values parse.Values) (WorkspaceSelection, error) {
	ids, err := workspace.ResolveWorkItemSelection(values.String("work_item"), values.String("positional_work_item"))
	if err != nil {
		return WorkspaceSelection{}, err
	}
	project := values.String("project")
	return WorkspaceSelection{Root: resolvedRoot(values), Workspace: optional(values, "workspace"), Project: project, IDs: ids, Continue: values.Bool("continue") || project != "" || len(ids) != 0}, nil
}
