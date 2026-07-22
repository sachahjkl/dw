package atlassian

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/work"
)

const ProviderName work.ProviderName = "atlassian"

type JiraOptions struct {
	URL                      string `json:"url"`
	Project                  string `json:"project"`
	Email                    string `json:"email"`
	TokenEnvironmentVariable string `json:"tokenEnvironmentVariable"`
	CredentialKey            string `json:"credentialKey"`
}

type BitbucketOptions struct {
	URL                      string `json:"url"`
	Workspace                string `json:"workspace"`
	Repository               string `json:"repository"`
	Username                 string `json:"username"`
	TokenEnvironmentVariable string `json:"tokenEnvironmentVariable"`
	CredentialKey            string `json:"credentialKey"`
}

type Options struct {
	Jira      JiraOptions      `json:"jira"`
	Bitbucket BitbucketOptions `json:"bitbucket"`
	Client    *http.Client     `json:"-"`
}

type Provider struct {
	base    Options
	secrets contract.SecretStore
}

func New(options Options, secrets contract.SecretStore) *Provider {
	return &Provider{base: options, secrets: secrets}
}
func (*Provider) Name() work.ProviderName { return ProviderName }

func (provider *Provider) options(reference work.ProjectRef) (Options, error) {
	resolved := Options{}
	if reference.Root != "" || reference.Key != "" {
		root := config.ResolveRoot(reference.Root)
		workflow := config.LoadWorkflowConfig(root)
		project, _ := config.ResolveProject(config.LoadProjectsConfig(root), string(reference.Key))
		configured, found, err := config.ResolveProviderOptions[Options](workflow, project, string(ProviderName))
		if err != nil {
			return Options{}, err
		}
		if found {
			resolved = configured
		}
	}
	overlay(&resolved, provider.base)
	if resolved.Bitbucket.URL == "" {
		resolved.Bitbucket.URL = "https://api.bitbucket.org/2.0"
	}
	if resolved.Jira.TokenEnvironmentVariable == "" {
		resolved.Jira.TokenEnvironmentVariable = "JIRA_API_TOKEN"
	}
	if resolved.Bitbucket.TokenEnvironmentVariable == "" {
		resolved.Bitbucket.TokenEnvironmentVariable = "BITBUCKET_TOKEN"
	}
	if resolved.Jira.CredentialKey == "" {
		resolved.Jira.CredentialKey = "atlassian/jira-token"
	}
	if resolved.Bitbucket.CredentialKey == "" {
		resolved.Bitbucket.CredentialKey = "atlassian/bitbucket-token"
	}
	if resolved.Client == nil {
		resolved.Client = http.DefaultClient
	}
	if resolved.Jira.Project == "" {
		if reference.Project != "" {
			resolved.Jira.Project = reference.Project
		} else {
			resolved.Jira.Project = string(reference.Key)
		}
	}
	return resolved, nil
}

func overlay(target *Options, override Options) {
	if override.Jira.URL != "" {
		target.Jira.URL = override.Jira.URL
	}
	if override.Jira.Project != "" {
		target.Jira.Project = override.Jira.Project
	}
	if override.Jira.Email != "" {
		target.Jira.Email = override.Jira.Email
	}
	if override.Jira.TokenEnvironmentVariable != "" {
		target.Jira.TokenEnvironmentVariable = override.Jira.TokenEnvironmentVariable
	}
	if override.Jira.CredentialKey != "" {
		target.Jira.CredentialKey = override.Jira.CredentialKey
	}
	if override.Bitbucket.URL != "" {
		target.Bitbucket.URL = override.Bitbucket.URL
	}
	if override.Bitbucket.Workspace != "" {
		target.Bitbucket.Workspace = override.Bitbucket.Workspace
	}
	if override.Bitbucket.Repository != "" {
		target.Bitbucket.Repository = override.Bitbucket.Repository
	}
	if override.Bitbucket.Username != "" {
		target.Bitbucket.Username = override.Bitbucket.Username
	}
	if override.Bitbucket.TokenEnvironmentVariable != "" {
		target.Bitbucket.TokenEnvironmentVariable = override.Bitbucket.TokenEnvironmentVariable
	}
	if override.Bitbucket.CredentialKey != "" {
		target.Bitbucket.CredentialKey = override.Bitbucket.CredentialKey
	}
	if override.Client != nil {
		target.Client = override.Client
	}
}

func (provider *Provider) credential(ctx context.Context, environment, key string) (string, string, error) {
	if environment != "" {
		if value, found := os.LookupEnv(environment); found && strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value), "environment:" + environment, nil
		}
	}
	if provider.secrets != nil && key != "" {
		secret, found, err := provider.secrets.Get(ctx, contract.SecretKey(key))
		if err != nil {
			return "", "", fmt.Errorf("atlassian.secret-store: %w", err)
		}
		if found && !secret.Empty() {
			return secret.Reveal(), "keyring:" + key, nil
		}
	}
	return "", "", nil
}

func (provider *Provider) jiraRequest(ctx context.Context, options Options, method, path string, query url.Values, body, result any) ([]byte, error) {
	if strings.TrimSpace(options.Jira.URL) == "" {
		return nil, fmt.Errorf("atlassian.jira-url-required")
	}
	token, _, err := provider.credential(ctx, options.Jira.TokenEnvironmentVariable, options.Jira.CredentialKey)
	if err != nil {
		return nil, err
	}
	return provider.request(ctx, options.Client, strings.TrimRight(options.Jira.URL, "/")+path, method, query, body, result, func(request *http.Request) {
		if token != "" {
			request.SetBasicAuth(options.Jira.Email, token)
		}
	})
}

func (provider *Provider) bitbucketRequest(ctx context.Context, options Options, method, path string, query url.Values, body, result any) ([]byte, error) {
	token, _, err := provider.credential(ctx, options.Bitbucket.TokenEnvironmentVariable, options.Bitbucket.CredentialKey)
	if err != nil {
		return nil, err
	}
	return provider.request(ctx, options.Client, strings.TrimRight(options.Bitbucket.URL, "/")+path, method, query, body, result, func(request *http.Request) {
		if token == "" {
			return
		}
		if options.Bitbucket.Username != "" {
			request.SetBasicAuth(options.Bitbucket.Username, token)
		} else {
			request.Header.Set("Authorization", "Bearer "+token)
		}
	})
}

func (*Provider) request(ctx context.Context, client *http.Client, endpoint, method string, query url.Values, body, result any, authorize func(*http.Request)) ([]byte, error) {
	if len(query) != 0 {
		endpoint += "?" + query.Encode()
	}
	var payload io.Reader
	if body != nil {
		encoded, err := json.Marshal(body)
		if err != nil {
			return nil, err
		}
		payload = bytes.NewReader(encoded)
	}
	request, err := http.NewRequestWithContext(ctx, method, endpoint, payload)
	if err != nil {
		return nil, err
	}
	request.Header.Set("Accept", "application/json")
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	authorize(request)
	response, err := client.Do(request)
	if err != nil {
		return nil, fmt.Errorf("atlassian.request: %w", err)
	}
	defer response.Body.Close()
	content, err := io.ReadAll(io.LimitReader(response.Body, 8<<20))
	if err != nil {
		return nil, err
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, fmt.Errorf("atlassian.http:%d:%s", response.StatusCode, atlassianMessage(content))
	}
	if result != nil && len(content) != 0 {
		if err := json.Unmarshal(content, result); err != nil {
			return nil, fmt.Errorf("atlassian.decode: %w", err)
		}
	}
	return content, nil
}

func atlassianMessage(content []byte) string {
	var response struct {
		ErrorMessages []string `json:"errorMessages"`
		Message       string   `json:"message"`
		Error         struct {
			Message string `json:"message"`
		} `json:"error"`
	}
	if json.Unmarshal(content, &response) == nil {
		if len(response.ErrorMessages) != 0 {
			return strings.Join(response.ErrorMessages, "; ")
		}
		if response.Message != "" {
			return response.Message
		}
		if response.Error.Message != "" {
			return response.Error.Message
		}
	}
	return strings.TrimSpace(string(content))
}

func bitbucketRepository(options Options, repository work.RepositoryName) (string, string, error) {
	value := strings.TrimSpace(string(repository))
	if value == "" {
		value = options.Bitbucket.Repository
	}
	workspace := options.Bitbucket.Workspace
	if before, after, found := strings.Cut(value, "/"); found {
		workspace, value = before, after
	}
	if workspace == "" || value == "" {
		return "", "", fmt.Errorf("atlassian.bitbucket-repository-required")
	}
	return workspace, value, nil
}

func bitbucketRepositoryPath(options Options, repository work.RepositoryName) (string, error) {
	workspace, name, err := bitbucketRepository(options, repository)
	if err != nil {
		return "", err
	}
	return "/repositories/" + url.PathEscape(workspace) + "/" + url.PathEscape(name), nil
}
