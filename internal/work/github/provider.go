package github

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/work"
)

const ProviderName work.ProviderName = "github"

type Options struct {
	Owner                    string       `json:"owner"`
	Repository               string       `json:"repository"`
	APIURL                   string       `json:"apiUrl"`
	WebURL                   string       `json:"webUrl"`
	TokenEnvironmentVariable string       `json:"tokenEnvironmentVariable"`
	CredentialKey            string       `json:"credentialKey"`
	OAuthClientID            string       `json:"oauthClientId"`
	Client                   *http.Client `json:"-"`
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
	overlayOptions(&resolved, provider.base)
	if resolved.APIURL == "" {
		resolved.APIURL = "https://api.github.com"
	}
	if resolved.WebURL == "" {
		resolved.WebURL = "https://github.com"
	}
	if resolved.TokenEnvironmentVariable == "" {
		resolved.TokenEnvironmentVariable = "GITHUB_TOKEN"
	}
	if resolved.CredentialKey == "" {
		resolved.CredentialKey = "github/token"
	}
	if resolved.Client == nil {
		resolved.Client = http.DefaultClient
	}
	if resolved.Repository == "" && reference.Project != "" {
		resolved.Repository = reference.Project
	}
	return resolved, nil
}

func overlayOptions(target *Options, override Options) {
	if override.Owner != "" {
		target.Owner = override.Owner
	}
	if override.Repository != "" {
		target.Repository = override.Repository
	}
	if override.APIURL != "" {
		target.APIURL = override.APIURL
	}
	if override.WebURL != "" {
		target.WebURL = override.WebURL
	}
	if override.TokenEnvironmentVariable != "" {
		target.TokenEnvironmentVariable = override.TokenEnvironmentVariable
	}
	if override.CredentialKey != "" {
		target.CredentialKey = override.CredentialKey
	}
	if override.OAuthClientID != "" {
		target.OAuthClientID = override.OAuthClientID
	}
	if override.Client != nil {
		target.Client = override.Client
	}
}

func (provider *Provider) token(ctx context.Context, options Options) (string, string, error) {
	if variable := strings.TrimSpace(options.TokenEnvironmentVariable); variable != "" {
		if value, found := os.LookupEnv(variable); found && strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value), "environment:" + variable, nil
		}
	}
	if provider.secrets != nil && strings.TrimSpace(options.CredentialKey) != "" {
		secret, found, err := provider.secrets.Get(ctx, contract.SecretKey(options.CredentialKey))
		if err != nil {
			return "", "", fmt.Errorf("github.secret-store: %w", err)
		}
		if found && !secret.Empty() {
			return secret.Reveal(), "keyring:" + options.CredentialKey, nil
		}
	}
	return "", "", nil
}

func (provider *Provider) request(ctx context.Context, options Options, method, path string, query url.Values, body any, result any) ([]byte, error) {
	endpoint := strings.TrimRight(options.APIURL, "/") + path
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
	request.Header.Set("Accept", "application/vnd.github+json")
	request.Header.Set("X-GitHub-Api-Version", "2022-11-28")
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	if token, _, tokenErr := provider.token(ctx, options); tokenErr != nil {
		return nil, tokenErr
	} else if token != "" {
		request.Header.Set("Authorization", "Bearer "+token)
	}
	response, err := options.Client.Do(request)
	if err != nil {
		return nil, fmt.Errorf("github.request: %w", err)
	}
	defer response.Body.Close()
	content, err := io.ReadAll(io.LimitReader(response.Body, 8<<20))
	if err != nil {
		return nil, err
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, fmt.Errorf("github.http:%d:%s", response.StatusCode, responseMessage(content))
	}
	if result != nil && len(content) != 0 {
		if err := json.Unmarshal(content, result); err != nil {
			return nil, fmt.Errorf("github.decode: %w", err)
		}
	}
	return content, nil
}

func responseMessage(content []byte) string {
	var response struct {
		Message string `json:"message"`
	}
	if json.Unmarshal(content, &response) == nil && response.Message != "" {
		return response.Message
	}
	return strings.TrimSpace(string(content))
}

func repositoryParts(options Options, repository work.RepositoryName) (string, string, error) {
	value := strings.TrimSpace(string(repository))
	if value == "" {
		value = strings.TrimSpace(options.Repository)
	}
	owner := strings.TrimSpace(options.Owner)
	if before, after, found := strings.Cut(value, "/"); found {
		owner, value = before, after
	}
	if owner == "" || value == "" {
		return "", "", fmt.Errorf("github.repository-required")
	}
	return owner, value, nil
}

func repositoryPath(options Options, repository work.RepositoryName) (string, error) {
	owner, name, err := repositoryParts(options, repository)
	if err != nil {
		return "", err
	}
	return "/repos/" + url.PathEscape(owner) + "/" + url.PathEscape(name), nil
}

func parseID(id work.ItemID) (int64, error) {
	value, err := strconv.ParseInt(string(id), 10, 64)
	if err != nil || value < 1 {
		return 0, fmt.Errorf("github.invalid-item-id:%s", id)
	}
	return value, nil
}

func timestamp(value string) contract.Timestamp {
	if parsed, err := time.Parse(time.RFC3339, value); err == nil {
		return contract.Timestamp(parsed.UTC().Format(time.RFC3339))
	}
	return contract.Timestamp(value)
}
