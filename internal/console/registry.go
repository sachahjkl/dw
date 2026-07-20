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
	ResultVersion             ResultKind = "version"
	ResultGuide               ResultKind = "guide"
	ResultDoctor              ResultKind = "doctor"
	ResultInit                ResultKind = "init"
	ResultRefresh             ResultKind = "refresh"
	ResultTUI                 ResultKind = "tui"
	ResultAgentContext        ResultKind = "agent.context"
	ResultAgentOpen           ResultKind = "agent.open"
	ResultAgentConfig         ResultKind = "agent.config"
	ResultAgentShow           ResultKind = "agent.show"
	ResultAgentDefaultSet     ResultKind = "agent.default.set"
	ResultAgentDoctor         ResultKind = "agent.doctor"
	ResultAuthLogin           ResultKind = "auth.login"
	ResultAuthStatus          ResultKind = "auth.status"
	ResultAuthLogout          ResultKind = "auth.logout"
	ResultCompletionShow      ResultKind = "completion.show"
	ResultCompletionGenerate  ResultKind = "completion.generate"
	ResultCompletionInstall   ResultKind = "completion.install"
	ResultCompletionComplete  ResultKind = "completion.complete"
	ResultConfigShow          ResultKind = "config.show"
	ResultConfigDoctor        ResultKind = "config.doctor"
	ResultConfigRootSet       ResultKind = "config.root.set"
	ResultConfigColorSet      ResultKind = "config.color.set"
	ResultADOAssigned         ResultKind = "ado.assigned"
	ResultADOPullRequests     ResultKind = "ado.prs"
	ResultADOChangelog        ResultKind = "ado.changelog"
	ResultADOWorkItem         ResultKind = "ado.item.show"
	ResultADOSetState         ResultKind = "ado.state.set"
	ResultADOContext          ResultKind = "ado.context.show"
	ResultADOAIContext        ResultKind = "ado.context.ai"
	ResultDBList              ResultKind = "db.list"
	ResultDBCollect           ResultKind = "db.collect"
	ResultDBGuard             ResultKind = "db.guard"
	ResultDBSchema            ResultKind = "db.schema"
	ResultDBDescribe          ResultKind = "db.describe"
	ResultDBQuery             ResultKind = "db.query"
	ResultSecretList          ResultKind = "secret.list"
	ResultSecretSet           ResultKind = "secret.set"
	ResultSecretGet           ResultKind = "secret.get"
	ResultSecretDelete        ResultKind = "secret.delete"
	ResultUpgrade             ResultKind = "upgrade.run"
	ResultWorkStatus          ResultKind = "work.status"
	ResultWorkList            ResultKind = "work.list"
	ResultWorkCurrent         ResultKind = "work.current"
	ResultWorkDoing           ResultKind = "work.item.doing"
	ResultWorkItemAdd         ResultKind = "work.item.add"
	ResultWorkItemRemove      ResultKind = "work.item.remove"
	ResultWorkCreateChild     ResultKind = "work.task.child.create"
	ResultWorkOpen            ResultKind = "work.open"
	ResultWorkStart           ResultKind = "work.start"
	ResultWorkStartPR         ResultKind = "work.pr.start"
	ResultWorkPreflight       ResultKind = "work.preflight"
	ResultWorkSync            ResultKind = "work.sync"
	ResultWorkRename          ResultKind = "work.rename"
	ResultWorkAddRepo         ResultKind = "work.repo.add"
	ResultWorkRepoLatest      ResultKind = "work.repo.latest"
	ResultWorkCommit          ResultKind = "work.commit"
	ResultWorkFinish          ResultKind = "work.finish"
	ResultWorkHandoffValidate ResultKind = "work.handoff.validate"
	ResultWorkTeardown        ResultKind = "work.teardown"
	ResultWorkPrune           ResultKind = "work.prune"
)

var RequiredResultKinds = []ResultKind{
	"init", "refresh", "config.show", "config.doctor", "config.root.set", "config.color.set",
	"agent.config", "agent.show", "agent.default.set", "agent.context", "agent.open", "doctor", "agent.doctor",
	"db.list", "db.collect", "db.guard", "db.schema", "db.describe", "db.query",
	"secret.list", "secret.set", "secret.get", "secret.delete", "upgrade",
	"auth.login", "auth.status", "auth.logout", "ado.assigned", "ado.prs", "ado.changelog",
	"ado.context.show", "ado.context.ai", "ado.item.show", "ado.state.set", "work.item.doing",
	"work.status", "work.list", "work.current", "work.item.add", "work.item.remove", "work.preflight",
	"work.rename", "work.repo.add", "work.repo.latest", "work.commit", "work.handoff.validate", "work.teardown",
	"work.start", "work.pr.start", "work.open", "work.sync", "work.task.child.create", "work.prune", "work.finish",
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

func (r *Registry) Alias(alias, target ResultKind) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if alias == "" || target == "" {
		return errors.New("console.invalid-renderer-alias")
	}
	if _, exists := r.renderers[alias]; exists {
		return errors.New("console.duplicate-renderer:" + string(alias))
	}
	renderer, exists := r.renderers[target]
	if !exists {
		return RendererNotFoundError{Kind: string(target)}
	}
	r.renderers[alias] = renderer
	return nil
}

func (r *Registry) Union(kind ResultKind, targets ...ResultKind) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if kind == "" || len(targets) == 0 {
		return errors.New("console.invalid-renderer-union")
	}
	if _, exists := r.renderers[kind]; exists {
		return errors.New("console.duplicate-renderer:" + string(kind))
	}
	renderers := make([]ResultRenderer, len(targets))
	for i, target := range targets {
		renderer, exists := r.renderers[target]
		if !exists {
			return RendererNotFoundError{Kind: string(target)}
		}
		renderers[i] = renderer
	}
	r.renderers[kind] = func(context RenderContext, payload any) (Output, error) {
		for _, renderer := range renderers {
			output, err := renderer(context, payload)
			var mismatch PayloadTypeError
			if errors.As(err, &mismatch) {
				continue
			}
			return output, err
		}
		return Output{}, PayloadTypeError{Kind: string(kind)}
	}
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
