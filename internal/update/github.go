package update

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
)

func (service *Service) DiscoverRelease(ctx context.Context, config Config) (GitHubRelease, error) {
	config, err := normalizeConfig(config)
	if err != nil {
		return GitHubRelease{}, err
	}
	base := strings.TrimRight(service.APIBaseURL, "/")
	if base == "" {
		base = DefaultGitHubAPI
	}
	owner := url.PathEscape(config.Owner)
	repository := url.PathEscape(config.Repository)
	endpoint := base + "/repos/" + owner + "/" + repository + "/releases/latest"
	if config.IncludePrerelease {
		endpoint = base + "/repos/" + owner + "/" + repository + "/releases"
	}
	response, err := service.doGET(ctx, endpoint)
	if err != nil {
		return GitHubRelease{}, err
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		body, _ := io.ReadAll(response.Body)
		return GitHubRelease{}, fmt.Errorf("update: github-releases-http-%d: %s", response.StatusCode, body)
	}
	if config.IncludePrerelease {
		var releases []githubReleaseWire
		if err := decodeJSON(response.Body, &releases); err != nil {
			return GitHubRelease{}, fmt.Errorf("update: decode-github-releases: %w", err)
		}
		if len(releases) == 0 {
			return GitHubRelease{}, fmt.Errorf("update: no-github-release")
		}
		return releases[0].release()
	}
	var release githubReleaseWire
	if err := decodeJSON(response.Body, &release); err != nil {
		return GitHubRelease{}, fmt.Errorf("update: decode-github-release: %w", err)
	}
	return release.release()
}

type githubReleaseWire struct {
	TagName *string `json:"tag_name"`
	Assets  *[]struct {
		Name *string `json:"name"`
		URL  *string `json:"browser_download_url"`
	} `json:"assets"`
}

func (wire githubReleaseWire) release() (GitHubRelease, error) {
	if wire.TagName == nil || wire.Assets == nil {
		return GitHubRelease{}, fmt.Errorf("update: github-release-missing-required-field")
	}
	release := GitHubRelease{TagName: *wire.TagName, Assets: make([]GitHubAsset, 0, len(*wire.Assets))}
	for index, asset := range *wire.Assets {
		if asset.Name == nil || asset.URL == nil {
			return GitHubRelease{}, fmt.Errorf("update: github-asset-%d-missing-required-field", index)
		}
		release.Assets = append(release.Assets, GitHubAsset{Name: *asset.Name, URL: *asset.URL})
	}
	return release, nil
}

func (service *Service) FetchManifest(ctx context.Context, release GitHubRelease, assetName string) (Manifest, error) {
	var manifestURL string
	for _, asset := range release.Assets {
		if strings.EqualFold(asset.Name, assetName) {
			manifestURL = asset.URL
			break
		}
	}
	if manifestURL == "" {
		return Manifest{}, fmt.Errorf("update: release-asset-not-found %q", assetName)
	}
	response, err := service.doGET(ctx, manifestURL)
	if err != nil {
		return Manifest{}, err
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		body, _ := io.ReadAll(response.Body)
		return Manifest{}, fmt.Errorf("update: manifest-http-%d: %s", response.StatusCode, body)
	}
	return ParseManifest(response.Body)
}

func (service *Service) doGET(ctx context.Context, endpoint string) (*http.Response, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return nil, fmt.Errorf("update: create-http-request: %w", err)
	}
	userAgent := service.UserAgent
	if userAgent == "" {
		userAgent = "dw/1.0"
	}
	request.Header.Set("User-Agent", userAgent)
	client := service.HTTPClient
	if client == nil {
		client = http.DefaultClient
	}
	response, err := client.Do(request)
	if err != nil {
		return nil, fmt.Errorf("update: http-request: %w", err)
	}
	return response, nil
}
