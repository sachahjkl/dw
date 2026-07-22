package github

import (
	"context"
	"fmt"
	"net/http"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/work"
)

func (provider *Provider) AuthStatus(ctx context.Context, reference work.ProjectRef) (work.AuthStatus, error) {
	options, err := provider.options(reference)
	if err != nil {
		return work.AuthStatus{}, err
	}
	token, source, err := provider.token(ctx, options)
	if err != nil {
		return work.AuthStatus{}, err
	}
	if token == "" {
		return work.AuthStatus{}, nil
	}
	var user struct {
		Login string `json:"login"`
	}
	if _, err := provider.request(ctx, options, http.MethodGet, "/user", nil, nil, &user); err != nil {
		return work.AuthStatus{}, err
	}
	return work.AuthStatus{Authenticated: true, Source: source, Principal: user.Login}, nil
}

func (provider *Provider) Login(ctx context.Context, reference work.ProjectRef, mode work.AuthMode, _ func(work.DeviceLogin) error) (work.AuthStatus, error) {
	if mode != work.AuthEnvironment {
		return work.AuthStatus{}, fmt.Errorf("github.auth-mode-unsupported:%s:configure GITHUB_TOKEN or a credentialKey", mode)
	}
	status, err := provider.AuthStatus(ctx, reference)
	if err != nil {
		return work.AuthStatus{}, err
	}
	if !status.Authenticated {
		return work.AuthStatus{}, fmt.Errorf("github.token-required")
	}
	return status, nil
}

func (provider *Provider) Logout(ctx context.Context, reference work.ProjectRef) (bool, error) {
	options, err := provider.options(reference)
	if err != nil {
		return false, err
	}
	if provider.secrets == nil || options.CredentialKey == "" {
		return false, nil
	}
	deleted, err := provider.secrets.Delete(ctx, contract.SecretKey(options.CredentialKey))
	if err != nil {
		return false, fmt.Errorf("github.secret-store: %w", err)
	}
	return deleted, nil
}

var _ work.Authenticator = (*Provider)(nil)
