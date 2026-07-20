package ado

import (
	"context"

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
	token, err := auth.RequireToken(ctx)
	return options, token, err
}

var (
	_ work.Provider          = (*Provider)(nil)
	_ work.Authenticator     = (*Provider)(nil)
	_ work.ItemReader        = (*Provider)(nil)
	_ work.AssignedQuerier   = (*Provider)(nil)
	_ work.RelationReader    = (*Provider)(nil)
	_ work.StateWriter       = (*Provider)(nil)
	_ work.StateClassifier   = (*Provider)(nil)
	_ work.ChildCreator      = (*Provider)(nil)
	_ work.PullRequestReader = (*Provider)(nil)
	_ work.PullRequestWriter = (*Provider)(nil)
	_ work.RichContextReader = (*Provider)(nil)
	_ work.RawItemReader     = (*Provider)(nil)
)
