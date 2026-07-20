package bootstrap

import (
	"context"
	"encoding/json"
	"sync"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/work/ado"
	"github.com/sachahjkl/dw/internal/workapp"
)

type rootContextKey struct{}

func withRoot(ctx context.Context, root string) context.Context {
	return context.WithValue(ctx, rootContextKey{}, config.ResolveRoot(root))
}

func contextRoot(ctx context.Context) string {
	if root, ok := ctx.Value(rootContextKey{}).(string); ok && root != "" {
		return root
	}
	return config.ResolveRoot("")
}

type scopedHandler struct{ action.Handler }

func (handler scopedHandler) Execute(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
	return handler.Handler.Execute(withRoot(ctx, requestRoot(request)), request, runtime)
}

type scopedADOConfiguration struct {
	Organization string           `json:"organization"`
	Project      string           `json:"project"`
	APIVersion   string           `json:"apiVersion"`
	Auth         *ado.AuthOptions `json:"auth,omitempty"`
}

type scopedADOProvider struct {
	cache sync.Map
	base  *ado.Provider
}

func newScopedADOProvider() *scopedADOProvider {
	return &scopedADOProvider{base: ado.New(ado.Options{}, nil)}
}

func (*scopedADOProvider) Name() work.ProviderName { return ado.ProviderName }

func (provider *scopedADOProvider) ExtractCommitReferences(commitLog string) []work.ItemID {
	return provider.base.ExtractCommitReferences(commitLog)
}

func (provider *scopedADOProvider) resolve(ctx context.Context, reference work.ProjectRef) (*ado.Provider, work.ProjectRef, error) {
	root := contextRoot(ctx)
	if reference.Root != "" {
		root = config.ResolveRoot(reference.Root)
	}
	workflow := config.LoadWorkflowConfig(root)
	project, _ := config.ResolveProject(config.LoadProjectsConfig(root), string(reference.Key))
	raw, found, err := config.ResolveProviderRawOptions(workflow, project, string(ado.ProviderName))
	if err != nil {
		return nil, reference, err
	}
	var configured scopedADOConfiguration
	if found {
		if err := json.Unmarshal(raw, &configured); err != nil {
			return nil, reference, err
		}
	}
	options := ado.Options{}
	var auth *ado.AuthOptions
	if found {
		options = ado.Options{Organization: configured.Organization, Project: configured.Project, APIVersion: configured.APIVersion}
		auth = configured.Auth
	}
	if reference.Organization == "" {
		reference.Organization = options.Organization
	}
	if reference.Project == "" {
		reference.Project = options.Project
	}
	key := root + "\x00" + reference.Organization + "\x00" + reference.Project + "\x00" + options.APIVersion
	if cached, found := provider.cache.Load(key); found {
		return cached.(*ado.Provider), reference, nil
	}
	created := ado.New(options, auth)
	actual, _ := provider.cache.LoadOrStore(key, created)
	return actual.(*ado.Provider), reference, nil
}

func (provider *scopedADOProvider) AuthStatus(ctx context.Context, project work.ProjectRef) (work.AuthStatus, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return work.AuthStatus{}, err
	}
	return delegate.AuthStatus(ctx, project)
}
func (provider *scopedADOProvider) Login(ctx context.Context, project work.ProjectRef, mode work.AuthMode, emit func(work.DeviceLogin) error) (work.AuthStatus, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return work.AuthStatus{}, err
	}
	return delegate.Login(ctx, project, mode, emit)
}
func (provider *scopedADOProvider) Logout(ctx context.Context, project work.ProjectRef) (bool, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return false, err
	}
	return delegate.Logout(ctx, project)
}
func (provider *scopedADOProvider) ReadItems(ctx context.Context, project work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.Item, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.ReadItems(ctx, project, ids, options)
}
func (provider *scopedADOProvider) QueryAssigned(ctx context.Context, project work.ProjectRef, query work.AssignedQuery) ([]work.Item, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.QueryAssigned(ctx, project, query)
}
func (provider *scopedADOProvider) ReadRelations(ctx context.Context, project work.ProjectRef, ids []work.ItemID) ([]work.Relation, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.ReadRelations(ctx, project, ids)
}
func (provider *scopedADOProvider) UpdateStates(ctx context.Context, project work.ProjectRef, changes []work.StateChange) ([]work.StateChangeResult, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.UpdateStates(ctx, project, changes)
}
func (provider *scopedADOProvider) IsFinalState(kind work.ItemType, state work.State) bool {
	return provider.base.IsFinalState(kind, state)
}
func (provider *scopedADOProvider) CreateChild(ctx context.Context, project work.ProjectRef, request work.ChildCreate) (work.ChildCreateResult, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return work.ChildCreateResult{}, err
	}
	return delegate.CreateChild(ctx, project, request)
}
func (provider *scopedADOProvider) ListPullRequests(ctx context.Context, project work.ProjectRef, query work.PullRequestQuery) ([]work.PullRequest, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.ListPullRequests(ctx, project, query)
}
func (provider *scopedADOProvider) ActivePullRequest(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, source string) (*work.PullRequest, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.ActivePullRequest(ctx, project, repository, source)
}
func (provider *scopedADOProvider) PullRequestWorkItemIDs(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, id work.PullRequestID) ([]work.ItemID, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.PullRequestWorkItemIDs(ctx, project, repository, id)
}
func (provider *scopedADOProvider) CreatePullRequest(ctx context.Context, project work.ProjectRef, request work.PullRequestCreate) (work.PullRequestCreateResult, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	return delegate.CreatePullRequest(ctx, project, request)
}
func (provider *scopedADOProvider) LinkPullRequestWorkItem(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, pullRequest work.PullRequestID, item work.ItemID) error {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return err
	}
	return delegate.LinkPullRequestWorkItem(ctx, project, repository, pullRequest, item)
}
func (provider *scopedADOProvider) ReadRichContext(ctx context.Context, project work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.RichContext, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return nil, err
	}
	return delegate.ReadRichContext(ctx, project, ids, options)
}
func (provider *scopedADOProvider) ReadRawItem(ctx context.Context, project work.ProjectRef, id work.ItemID) (wirejson.Value, error) {
	delegate, project, err := provider.resolve(ctx, project)
	if err != nil {
		return wirejson.Value{}, err
	}
	return delegate.ReadRawItem(ctx, project, id)
}

func requestRoot(request action.Request) string {
	switch value := request.(type) {
	case workapp.AuthLoginRequest:
		return value.Root
	case workapp.AuthStatusRequest:
		return value.Root
	case workapp.AuthLogoutRequest:
		return value.Root
	case workapp.AssignedRequest:
		return value.Root
	case workapp.PullRequestsRequest:
		return value.Root
	case workapp.ChangelogRequest:
		return value.Root
	case workapp.ContextRequest:
		return value.Root
	case workapp.AIContextRequest:
		return value.Root
	case workapp.ItemShowRequest:
		return value.Root
	case workapp.StatePlanRequest:
		return value.Root
	case workapp.StateExecuteRequest:
		return value.Plan.Root
	case workapp.StateSetRequest:
		return value.Request.Root
	case workapp.DoingRequest:
		return value.Root
	case workapp.DoingExecuteRequest:
		return value.Plan.Root
	case workapp.StartRequest:
		return value.Root
	case workapp.StartPullRequestRequest:
		return value.Root
	case workapp.OpenRequest:
		return value.Root
	case workapp.SyncRequest:
		return value.Root
	case workapp.ChildRequest:
		return value.Root
	case workapp.PruneRequest:
		return value.Root
	case workapp.FinishRequest:
		return value.Root
	case controller.WorkspaceStatusRequest:
		return value.Root
	case controller.WorkspaceListRequest:
		return value.Root
	case controller.WorkspaceItemAddRequest:
		return value.Selection.Root
	case controller.WorkspaceItemRemoveRequest:
		return value.Selection.Root
	case controller.WorkspacePreflightRequest:
		return value.Selection.Root
	case controller.WorkspaceRenameRequest:
		return value.Selection.Root
	case controller.WorkspaceRepoAddRequest:
		return value.Selection.Root
	case controller.WorkspaceRepoLatestRequest:
		return value.Selection.Root
	case controller.WorkspaceCommitRequest:
		return value.Selection.Root
	case controller.WorkspaceHandoffRequest:
		return value.Selection.Root
	case controller.WorkspaceTeardownRequest:
		return value.Selection.Root
	default:
		return ""
	}
}

var (
	_ work.Authenticator     = (*scopedADOProvider)(nil)
	_ work.ItemReader        = (*scopedADOProvider)(nil)
	_ work.AssignedQuerier   = (*scopedADOProvider)(nil)
	_ work.RelationReader    = (*scopedADOProvider)(nil)
	_ work.StateWriter       = (*scopedADOProvider)(nil)
	_ work.StateClassifier   = (*scopedADOProvider)(nil)
	_ work.ChildCreator      = (*scopedADOProvider)(nil)
	_ work.PullRequestReader = (*scopedADOProvider)(nil)
	_ work.PullRequestWriter = (*scopedADOProvider)(nil)
	_ work.RichContextReader = (*scopedADOProvider)(nil)
	_ work.RawItemReader     = (*scopedADOProvider)(nil)
)
