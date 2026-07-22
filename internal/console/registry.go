package console

import (
	"errors"
	"sort"
	"strings"
	"sync"

	"github.com/sachahjkl/dw/internal/action"
)

type ResultKind = action.ID

const (
	ResultVersion                  ResultKind = "version"
	ResultGuide                    ResultKind = "guide"
	ResultDoctor                   ResultKind = "doctor"
	ResultInit                     ResultKind = "init"
	ResultRefresh                  ResultKind = "refresh"
	ResultTUI                      ResultKind = "tui"
	ResultAgentContext             ResultKind = "agent.context"
	ResultAgentConfig              ResultKind = "agent.config"
	ResultAgentShow                ResultKind = "agent.show"
	ResultAgentDefaultSet          ResultKind = "agent.default.set"
	ResultAgentDoctor              ResultKind = "agent.doctor"
	ResultProviderList             ResultKind = "provider.list"
	ResultProviderShow             ResultKind = "provider.show"
	ResultProviderCapabilities     ResultKind = "provider.capabilities"
	ResultProviderAuthLogin        ResultKind = "provider.auth.login"
	ResultProviderAuthStatus       ResultKind = "provider.auth.status"
	ResultProviderAuthLogout       ResultKind = "provider.auth.logout"
	ResultCompletionShow           ResultKind = "completion.show"
	ResultCompletionGenerate       ResultKind = "completion.generate"
	ResultCompletionInstall        ResultKind = "completion.install"
	ResultCompletionComplete       ResultKind = "completion.complete"
	ResultConfigShow               ResultKind = "config.show"
	ResultConfigDoctor             ResultKind = "config.doctor"
	ResultConfigRootSet            ResultKind = "config.root.set"
	ResultConfigColorSet           ResultKind = "config.color.set"
	ResultWorkItemList             ResultKind = "work.item.list"
	ResultWorkPullRequestList      ResultKind = "work.pr.list"
	ResultWorkChangelog            ResultKind = "work.changelog"
	ResultWorkItemShow             ResultKind = "work.item.show"
	ResultWorkItemStateSet         ResultKind = "work.item.state.set"
	ResultWorkContextShow          ResultKind = "work.context.show"
	ResultWorkContextAI            ResultKind = "work.context.ai"
	ResultDataSourceList           ResultKind = "data.source.list"
	ResultDataSourceCollect        ResultKind = "data.source.collect"
	ResultDataGuard                ResultKind = "data.guard"
	ResultDataCatalog              ResultKind = "data.catalog"
	ResultDataDescribe             ResultKind = "data.describe"
	ResultDataQuery                ResultKind = "data.query"
	ResultDataRead                 ResultKind = "data.read"
	ResultSecretList               ResultKind = "secret.list"
	ResultSecretSet                ResultKind = "secret.set"
	ResultSecretGet                ResultKind = "secret.get"
	ResultSecretDelete             ResultKind = "secret.delete"
	ResultUpgrade                  ResultKind = "upgrade"
	ResultWorkspaceStatus          ResultKind = "workspace.status"
	ResultWorkspaceList            ResultKind = "workspace.list"
	ResultWorkspaceCurrent         ResultKind = "workspace.current"
	ResultWorkItemDoing            ResultKind = "work.item.doing"
	ResultWorkspaceItemAdd         ResultKind = "workspace.item.add"
	ResultWorkspaceItemRemove      ResultKind = "workspace.item.remove"
	ResultWorkItemChildCreate      ResultKind = "work.item.child.create"
	ResultWorkspaceOpen            ResultKind = "workspace.open"
	ResultWorkspaceStart           ResultKind = "workspace.start"
	ResultWorkspaceStartPR         ResultKind = "workspace.pr.start"
	ResultWorkspacePreflight       ResultKind = "workspace.preflight"
	ResultWorkspaceSync            ResultKind = "workspace.sync"
	ResultWorkspaceRename          ResultKind = "workspace.rename"
	ResultWorkspaceAddRepo         ResultKind = "workspace.repo.add"
	ResultWorkspaceRepoLatest      ResultKind = "workspace.repo.latest"
	ResultWorkspaceCommit          ResultKind = "workspace.commit"
	ResultWorkspaceFinish          ResultKind = "workspace.finish"
	ResultWorkspaceHandoffValidate ResultKind = "workspace.handoff.validate"
	ResultWorkspaceTeardown        ResultKind = "workspace.teardown"
	ResultWorkspacePrune           ResultKind = "workspace.prune"
)

var RequiredResultKinds = []ResultKind{
	"init", "refresh", "config.show", "config.doctor", "config.root.set", "config.color.set",
	"agent.config", "agent.show", "agent.default.set", "agent.context", "doctor", "agent.doctor",
	"secret.list", "secret.set", "secret.get", "secret.delete", "upgrade",
	"work.item.list", "work.item.show", "work.item.doing", "work.item.state.set", "work.item.child.create",
	"work.pr.list", "work.context.show", "work.context.ai", "work.changelog",
	"workspace.status", "workspace.list", "workspace.current", "workspace.open", "workspace.start", "workspace.pr.start",
	"workspace.preflight", "workspace.sync", "workspace.rename", "workspace.repo.add", "workspace.repo.latest",
	"workspace.item.add", "workspace.item.remove", "workspace.commit", "workspace.finish", "workspace.handoff.validate", "workspace.teardown", "workspace.prune",
	"data.source.list", "data.source.collect", "data.guard", "data.catalog", "data.describe", "data.query", "data.read",
	"provider.list", "provider.show", "provider.capabilities", "provider.auth.login", "provider.auth.status", "provider.auth.logout",
}

type RenderContext struct {
	Localizer  Localizer
	Theme      Theme
	EventTheme Theme
	Policy     Policy
}

func NewRenderContext(policy Policy, localizer Localizer) RenderContext {
	return RenderContext{
		Localizer:  WithConsoleMessages(localizer),
		Theme:      NewTheme(policy.StdoutColor()),
		EventTheme: NewTheme(policy.StderrColor()),
		Policy:     policy,
	}
}

func NewRenderContextForFormat(policy Policy, localizer Localizer, format OutputFormat) RenderContext {
	policy.Machine = format == FormatJSON
	return NewRenderContext(policy, localizer)
}

type ResultRenderer func(RenderContext, any) (Output, error)

type Registry struct {
	mu        sync.RWMutex
	renderers map[ResultKind]ResultRenderer
}

func NewRegistry() *Registry { return &Registry{renderers: make(map[ResultKind]ResultRenderer)} }

func (r *Registry) Register(kind ResultKind, renderer ResultRenderer) error {
	if kind == "" || renderer == nil {
		return errors.New("console.invalid-renderer-registration")
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, exists := r.renderers[kind]; exists {
		return errors.New("console.duplicate-renderer:" + string(kind))
	}
	r.renderers[kind] = renderer
	return nil
}

func RegisterResult[T any](registry *Registry, kind ResultKind, renderer func(RenderContext, T) (Output, error)) error {
	return registry.Register(kind, func(context RenderContext, payload any) (Output, error) {
		value, ok := payload.(T)
		if !ok {
			return Output{}, PayloadTypeError{Kind: string(kind)}
		}
		return renderer(context, value)
	})
}

func RegisterPageResult[T any](registry *Registry, kind ResultKind, project func(T) Page) error {
	return RegisterResult(registry, kind, func(context RenderContext, value T) (Output, error) {
		page := project(value)
		return TextOutput(FormatHuman, RenderPage(page, context.Localizer, context.Theme)), nil
	})
}

func (r *Registry) Render(context RenderContext, kind ResultKind, payload any) (Output, error) {
	r.mu.RLock()
	renderer, ok := r.renderers[kind]
	r.mu.RUnlock()
	if !ok {
		return Output{}, RendererNotFoundError{Kind: string(kind)}
	}
	return renderer(context, payload)
}

func (r *Registry) RenderEnvelope(context RenderContext, envelope action.ResultEnvelope) (Output, error) {
	return r.Render(context, envelope.Action, envelope.Result)
}

func (r *Registry) Missing(required []ResultKind) []ResultKind {
	r.mu.RLock()
	defer r.mu.RUnlock()
	missing := make([]ResultKind, 0)
	for _, kind := range required {
		if _, ok := r.renderers[kind]; !ok {
			missing = append(missing, kind)
		}
	}
	sort.Slice(missing, func(i, j int) bool { return missing[i] < missing[j] })
	return missing
}

func (r *Registry) ValidateComplete(required []ResultKind) error {
	missing := r.Missing(required)
	if len(missing) == 0 {
		return nil
	}
	return MissingRenderersError{Kinds: missing}
}

type RendererNotFoundError struct{ Kind string }

func (e RendererNotFoundError) Error() string { return "console.renderer-not-found:" + e.Kind }

type PayloadTypeError struct{ Kind string }

func (e PayloadTypeError) Error() string { return "console.invalid-renderer-payload:" + e.Kind }

type MissingRenderersError struct{ Kinds []ResultKind }

func (e MissingRenderersError) Error() string {
	kinds := make([]string, len(e.Kinds))
	for i := range e.Kinds {
		kinds[i] = string(e.Kinds[i])
	}
	return "console.missing-renderers:" + strings.Join(kinds, ",")
}
