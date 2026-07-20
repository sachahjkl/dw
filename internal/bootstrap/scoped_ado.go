package bootstrap

import (
	"context"
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

type scopedADOProvider struct {
	cache sync.Map
	base  *ado.Provider
}

func newScopedADOProvider() *scopedADOProvider {
	return &scopedADOProvider{base: ado.New(ado.Options{}, nil)}
}

func (*scopedADOProvider) Name() work.ProviderName { return ado.ProviderName }

func (provider *scopedADOProvider) resolve(ctx context.Context, reference work.ProjectRef) (*ado.Provider, work.ProjectRef) {
	root := contextRoot(ctx)
	if reference.Root != "" {
		root = config.ResolveRoot(reference.Root)
	}
	workflow := config.LoadWorkflowConfig(root)
	options := ado.Options{}
	if workflow.AzureDevOps != nil {
		options = ado.Options{Organization: workflow.AzureDevOps.OrganizationURL, Project: workflow.AzureDevOps.Project, APIVersion: workflow.AzureDevOps.APIVersion}
	}
	if project, found := config.ResolveProject(config.LoadProjectsConfig(root), string(reference.Key)); found && project.AzureDevOps != nil {
		if project.AzureDevOps.OrganizationURL != "" {
			options.Organization = project.AzureDevOps.OrganizationURL
		}
		if project.AzureDevOps.Project != "" {
			options.Project = project.AzureDevOps.Project
		}
		if project.AzureDevOps.APIVersion != "" {
			options.APIVersion = project.AzureDevOps.APIVersion
		}
	}
	if reference.Organization == "" {
		reference.Organization = options.Organization
	}
	if reference.Project == "" {
		reference.Project = options.Project
	}
	var auth *ado.AuthOptions
	if workflow.Auth != nil {
		auth = &ado.AuthOptions{TenantID: workflow.Auth.TenantID, ClientID: workflow.Auth.ClientID, Scopes: append([]string(nil), workflow.Auth.Scopes...)}
	}
	key := root + "\x00" + reference.Organization + "\x00" + reference.Project + "\x00" + options.APIVersion
	if cached, found := provider.cache.Load(key); found {
		return cached.(*ado.Provider), reference
	}
	created := ado.New(options, auth)
	actual, _ := provider.cache.LoadOrStore(key, created)
	return actual.(*ado.Provider), reference
}

func (provider *scopedADOProvider) AuthStatus(ctx context.Context, project work.ProjectRef) (work.AuthStatus, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.AuthStatus(ctx, project)
}
func (provider *scopedADOProvider) Login(ctx context.Context, project work.ProjectRef, mode work.AuthMode, emit func(work.DeviceLogin) error) (work.AuthStatus, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.Login(ctx, project, mode, emit)
}
func (provider *scopedADOProvider) Logout(ctx context.Context, project work.ProjectRef) (bool, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.Logout(ctx, project)
}
func (provider *scopedADOProvider) ReadItems(ctx context.Context, project work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.Item, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.ReadItems(ctx, project, ids, options)
}
func (provider *scopedADOProvider) QueryAssigned(ctx context.Context, project work.ProjectRef, query work.AssignedQuery) ([]work.Item, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.QueryAssigned(ctx, project, query)
}
func (provider *scopedADOProvider) ReadRelations(ctx context.Context, project work.ProjectRef, ids []work.ItemID) ([]work.Relation, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.ReadRelations(ctx, project, ids)
}
func (provider *scopedADOProvider) UpdateStates(ctx context.Context, project work.ProjectRef, changes []work.StateChange) ([]work.StateChangeResult, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.UpdateStates(ctx, project, changes)
}
func (provider *scopedADOProvider) IsFinalState(kind work.ItemType, state work.State) bool {
	return provider.base.IsFinalState(kind, state)
}
func (provider *scopedADOProvider) CreateChild(ctx context.Context, project work.ProjectRef, request work.ChildCreate) (work.ChildCreateResult, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.CreateChild(ctx, project, request)
}
func (provider *scopedADOProvider) ListPullRequests(ctx context.Context, project work.ProjectRef, query work.PullRequestQuery) ([]work.PullRequest, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.ListPullRequests(ctx, project, query)
}
func (provider *scopedADOProvider) ActivePullRequest(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, source string) (*work.PullRequest, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.ActivePullRequest(ctx, project, repository, source)
}
func (provider *scopedADOProvider) PullRequestWorkItemIDs(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, id work.PullRequestID) ([]work.ItemID, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.PullRequestWorkItemIDs(ctx, project, repository, id)
}
func (provider *scopedADOProvider) CreatePullRequest(ctx context.Context, project work.ProjectRef, request work.PullRequestCreate) (work.PullRequestCreateResult, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.CreatePullRequest(ctx, project, request)
}
func (provider *scopedADOProvider) LinkPullRequestWorkItem(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, pullRequest work.PullRequestID, item work.ItemID) error {
	delegate, project := provider.resolve(ctx, project)
	return delegate.LinkPullRequestWorkItem(ctx, project, repository, pullRequest, item)
}
func (provider *scopedADOProvider) ReadRichContext(ctx context.Context, project work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.RichContext, error) {
	delegate, project := provider.resolve(ctx, project)
	return delegate.ReadRichContext(ctx, project, ids, options)
}
func (provider *scopedADOProvider) ReadRawItem(ctx context.Context, project work.ProjectRef, id work.ItemID) (wirejson.Value, error) {
	delegate, project := provider.resolve(ctx, project)
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
