package ado

import (
	"context"
	"strings"
	"sync"
	"time"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/secret"
	"github.com/sachahjkl/dw/internal/work"
)

const ProviderName work.ProviderName = "azure-devops"

type Resolution struct {
	Options     Options
	AuthOptions *AuthOptions
}

type Resolver interface {
	ResolveADO(context.Context, work.ProjectRef) (Resolution, error)
}

type ResolverFunc func(context.Context, work.ProjectRef) (Resolution, error)

func (resolve ResolverFunc) ResolveADO(ctx context.Context, project work.ProjectRef) (Resolution, error) {
	return resolve(ctx, project)
}

type Provider struct {
	Options   Options
	Transport *Transport
	Auth      *Authenticator
	Resolver  Resolver
	Store     contract.SecretStore
}

func New(options Options, authOptions *AuthOptions) *Provider {
	store := secret.NewKeyringStore(KeyringService, "")
	return NewWithStore(options, authOptions, store)
}

func NewWithStore(options Options, authOptions *AuthOptions, store contract.SecretStore) *Provider {
	return &Provider{Options: options, Transport: NewTransport(), Auth: NewAuthenticator(authOptions, store)}
}

func NewDynamic(resolver Resolver) *Provider {
	return NewDynamicWithStore(resolver, secret.NewKeyringStore(KeyringService, ""))
}

func NewDynamicWithStore(resolver Resolver, store contract.SecretStore) *Provider {
	return &Provider{Transport: NewTransport(), Resolver: resolver, Store: store}
}

func (p *Provider) Name() work.ProviderName { return ProviderName }

func (p *Provider) transport() *Transport {
	if p.Transport == nil {
		p.Transport = NewTransport()
	}
	return p.Transport
}

func (p *Provider) resolve(ctx context.Context, project work.ProjectRef) (Options, *Authenticator, error) {
	if p.Resolver != nil {
		resolution, err := p.Resolver.ResolveADO(ctx, project)
		if err != nil {
			return Options{}, nil, err
		}
		options := normalizedOptions(resolution.Options)
		if project.Organization != "" {
			options.Organization = project.Organization
		}
		if project.Project != "" {
			options.Project = project.Project
		}
		return options, NewAuthenticator(resolution.AuthOptions, p.Store), nil
	}
	options := p.Options
	if project.Organization != "" {
		options.Organization = project.Organization
	}
	if project.Project != "" {
		options.Project = project.Project
	}
	return normalizedOptions(options), p.Auth, nil
}

func (p *Provider) session(ctx context.Context, project work.ProjectRef) (Options, Token, error) {
	options, auth, err := p.resolve(ctx, project)
	if err != nil {
		return Options{}, Token{}, err
	}
	if auth == nil {
		return Options{}, Token{}, &Error{Kind: ErrorMissingAuth}
	}
	token, err := p.accessToken(ctx, options, auth)
	return options, token, err
}

const accessTokenExpirySkew = time.Minute

type accessTokenKey struct {
	organization string
	tenant       string
	client       string
	scopes       string
}

type accessTokenCache struct {
	mu     sync.Mutex
	tokens map[accessTokenKey]cachedAccessToken
}

type cachedAccessToken struct {
	token   Token
	expires time.Time
}

var accessTokens accessTokenCache

func (p *Provider) accessToken(ctx context.Context, options Options, auth *Authenticator) (Token, error) {
	if token := EnvironmentToken(); token != nil || auth.Options == nil {
		return auth.RequireToken(ctx)
	}
	tenant, client, _ := auth.tenantAndClient()
	key := accessTokenKey{organization: strings.ToLower(strings.TrimRight(options.Organization, "/")), tenant: strings.ToLower(tenant), client: client, scopes: strings.Join(auth.scopes(false), " ")}
	return accessTokens.get(ctx, key, auth)
}

func (cache *accessTokenCache) get(ctx context.Context, key accessTokenKey, auth *Authenticator) (Token, error) {
	cache.mu.Lock()
	defer cache.mu.Unlock()
	now := auth.now()
	if cached, found := cache.tokens[key]; found && now.Before(cached.expires.Add(-accessTokenExpirySkew)) {
		return cached.token, nil
	}
	token, err := auth.RequireToken(ctx)
	if err != nil {
		return Token{}, err
	}
	if token.ExpiresOn != nil {
		if expires, parseErr := time.Parse(time.RFC3339, *token.ExpiresOn); parseErr == nil {
			if cache.tokens == nil {
				cache.tokens = make(map[accessTokenKey]cachedAccessToken)
			}
			cache.tokens[key] = cachedAccessToken{token: token, expires: expires}
		}
	}
	return token, nil
}

func (cache *accessTokenCache) clear() {
	cache.mu.Lock()
	defer cache.mu.Unlock()
	cache.tokens = nil
}

var (
	_ work.Provider            = (*Provider)(nil)
	_ work.Authenticator       = (*Provider)(nil)
	_ work.ItemReader          = (*Provider)(nil)
	_ work.AssignedQuerier     = (*Provider)(nil)
	_ work.CurrentUserAssigner = (*Provider)(nil)
	_ work.RelationReader      = (*Provider)(nil)
	_ work.StateWriter         = (*Provider)(nil)
	_ work.StateClassifier     = (*Provider)(nil)
	_ work.ChildCreator        = (*Provider)(nil)
	_ work.PullRequestReader   = (*Provider)(nil)
	_ work.PullRequestWriter   = (*Provider)(nil)
	_ work.RichContextReader   = (*Provider)(nil)
	_ work.RawItemReader       = (*Provider)(nil)
)
