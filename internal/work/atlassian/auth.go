package atlassian

import (
	"context"
	"fmt"
	"net/http"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/work"
)

func (provider *Provider) AuthStatus(ctx context.Context, reference work.ProjectRef) (work.AuthStatus, error) {
	options, err := provider.options(reference)
	if err != nil {
		return work.AuthStatus{}, err
	}
	jiraToken, jiraSource, err := provider.credential(ctx, options.Jira.TokenEnvironmentVariable, options.Jira.CredentialKey)
	if err != nil {
		return work.AuthStatus{}, err
	}
	bitbucketToken, bitbucketSource, err := provider.credential(ctx, options.Bitbucket.TokenEnvironmentVariable, options.Bitbucket.CredentialKey)
	if err != nil {
		return work.AuthStatus{}, err
	}
	status := work.AuthStatus{}
	var principals []string
	var sources []string
	if options.Jira.URL != "" {
		if jiraToken == "" {
			return status, nil
		}
		var user struct {
			DisplayName  string `json:"displayName"`
			EmailAddress string `json:"emailAddress"`
		}
		if _, err := provider.jiraRequest(ctx, options, http.MethodGet, "/rest/api/3/myself", nil, nil, &user); err != nil {
			return work.AuthStatus{}, err
		}
		principal := user.DisplayName
		if principal == "" {
			principal = user.EmailAddress
		}
		principals = append(principals, principal)
		sources = append(sources, "jira="+jiraSource)
	}
	if options.Bitbucket.Workspace != "" || options.Bitbucket.Repository != "" {
		if bitbucketToken == "" {
			return status, nil
		}
		var user struct {
			DisplayName string `json:"display_name"`
			Username    string `json:"username"`
		}
		if _, err := provider.bitbucketRequest(ctx, options, http.MethodGet, "/user", nil, nil, &user); err != nil {
			return work.AuthStatus{}, err
		}
		principal := user.DisplayName
		if principal == "" {
			principal = user.Username
		}
		principals = append(principals, principal)
		sources = append(sources, "bitbucket="+bitbucketSource)
	}
	if len(principals) == 0 {
		return status, nil
	}
	status.Authenticated = true
	status.Principal = strings.Join(principals, " / ")
	status.Source = strings.Join(sources, ",")
	return status, nil
}

func (provider *Provider) Login(ctx context.Context, reference work.ProjectRef, mode work.AuthMode, _ func(work.DeviceLogin) error) (work.AuthStatus, error) {
	if mode != work.AuthEnvironment {
		return work.AuthStatus{}, fmt.Errorf("atlassian.auth-mode-unsupported:%s:configure environment credentials or credential keys", mode)
	}
	status, err := provider.AuthStatus(ctx, reference)
	if err != nil {
		return work.AuthStatus{}, err
	}
	if !status.Authenticated {
		return work.AuthStatus{}, fmt.Errorf("atlassian.credentials-required")
	}
	return status, nil
}

func (provider *Provider) Logout(ctx context.Context, reference work.ProjectRef) (bool, error) {
	options, err := provider.options(reference)
	if err != nil {
		return false, err
	}
	if provider.secrets == nil {
		return false, nil
	}
	deleted := false
	for _, key := range []string{options.Jira.CredentialKey, options.Bitbucket.CredentialKey} {
		if key == "" {
			continue
		}
		removed, deleteErr := provider.secrets.Delete(ctx, contract.SecretKey(key))
		if deleteErr != nil {
			return deleted, fmt.Errorf("atlassian.secret-store: %w", deleteErr)
		}
		deleted = deleted || removed
	}
	return deleted, nil
}

var _ work.Authenticator = (*Provider)(nil)
