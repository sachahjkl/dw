package atlassian

import (
	"context"
	"fmt"
	"net/http"
	"net/url"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
)

type bitbucketPullRequest struct {
	ID          int64  `json:"id"`
	Title       string `json:"title"`
	Description string `json:"description"`
	State       string `json:"state"`
	Draft       bool   `json:"draft"`
	Author      struct {
		DisplayName string `json:"display_name"`
	} `json:"author"`
	Source struct {
		Branch struct {
			Name string `json:"name"`
		} `json:"branch"`
	} `json:"source"`
	Destination struct {
		Branch struct {
			Name string `json:"name"`
		} `json:"branch"`
	} `json:"destination"`
	Links struct {
		HTML struct {
			Href string `json:"href"`
		} `json:"html"`
		Self struct {
			Href string `json:"href"`
		} `json:"self"`
	} `json:"links"`
}

type bitbucketPullResponse struct {
	Values []bitbucketPullRequest `json:"values"`
	Next   string                 `json:"next"`
}

func (provider *Provider) ListPullRequests(ctx context.Context, reference work.ProjectRef, query work.PullRequestQuery) ([]work.PullRequest, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	repositories := query.Repositories
	if len(repositories) == 0 {
		repositories = []work.RepositoryName{""}
	}
	result := make([]work.PullRequest, 0)
	for _, repository := range repositories {
		base, err := bitbucketRepositoryPath(options, repository)
		if err != nil {
			return nil, err
		}
		state := bitbucketState(query.Status)
		parameters := url.Values{"pagelen": {"50"}}
		if state != "" {
			parameters.Set("state", state)
		}
		var response bitbucketPullResponse
		if _, err := provider.bitbucketRequest(ctx, options, http.MethodGet, base+"/pullrequests", parameters, nil, &response); err != nil {
			return nil, err
		}
		_, repositoryName, _ := bitbucketRepository(options, repository)
		for _, source := range response.Values {
			result = append(result, projectBitbucketPull(source, repositoryName))
		}
		for response.Next != "" {
			next, parseErr := url.Parse(response.Next)
			if parseErr != nil {
				return nil, parseErr
			}
			response = bitbucketPullResponse{}
			nextPath := next.Path
			if baseURL, baseErr := url.Parse(options.Bitbucket.URL); baseErr == nil {
				nextPath = strings.TrimPrefix(nextPath, strings.TrimRight(baseURL.Path, "/"))
			}
			if !strings.HasPrefix(nextPath, "/") {
				nextPath = "/" + nextPath
			}
			if _, err := provider.bitbucketRequest(ctx, options, http.MethodGet, nextPath, next.Query(), nil, &response); err != nil {
				return nil, err
			}
			for _, source := range response.Values {
				result = append(result, projectBitbucketPull(source, repositoryName))
			}
		}
	}
	return result, nil
}

func (provider *Provider) ActivePullRequest(ctx context.Context, reference work.ProjectRef, repository work.RepositoryName, sourceRef string) (*work.PullRequest, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := bitbucketRepositoryPath(options, repository)
	if err != nil {
		return nil, err
	}
	branch := trimBranch(sourceRef)
	parameters := url.Values{"state": {"OPEN"}, "q": {"source.branch.name=\"" + strings.ReplaceAll(branch, "\"", "\\\"") + "\""}, "pagelen": {"1"}}
	var response bitbucketPullResponse
	if _, err := provider.bitbucketRequest(ctx, options, http.MethodGet, base+"/pullrequests", parameters, nil, &response); err != nil {
		return nil, err
	}
	if len(response.Values) == 0 {
		return nil, nil
	}
	_, repositoryName, _ := bitbucketRepository(options, repository)
	projected := projectBitbucketPull(response.Values[0], repositoryName)
	return &projected, nil
}

func (provider *Provider) PullRequestWorkItemIDs(ctx context.Context, reference work.ProjectRef, repository work.RepositoryName, id work.PullRequestID) ([]work.ItemID, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := bitbucketRepositoryPath(options, repository)
	if err != nil {
		return nil, err
	}
	number, err := pullRequestNumber(id)
	if err != nil {
		return nil, err
	}
	var source bitbucketPullRequest
	if _, err := provider.bitbucketRequest(ctx, options, http.MethodGet, base+"/pullrequests/"+strconv.FormatInt(number, 10), nil, nil, &source); err != nil {
		return nil, err
	}
	return provider.ExtractCommitReferences(source.Title + "\n" + source.Description), nil
}

func (provider *Provider) CreatePullRequest(ctx context.Context, reference work.ProjectRef, request work.PullRequestCreate) (work.PullRequestCreateResult, error) {
	options, err := provider.options(reference)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	base, err := bitbucketRepositoryPath(options, request.Repository)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	body := map[string]any{"title": request.Title, "description": request.Description, "source": map[string]any{"branch": map[string]string{"name": trimBranch(request.SourceRef)}}, "destination": map[string]any{"branch": map[string]string{"name": trimBranch(request.TargetRef)}}, "draft": request.Draft, "close_source_branch": false}
	var created bitbucketPullRequest
	if _, err := provider.bitbucketRequest(ctx, options, http.MethodPost, base+"/pullrequests", nil, body, &created); err != nil {
		return work.PullRequestCreateResult{}, err
	}
	for _, id := range request.WorkItemIDs {
		if err := provider.linkPullRequestWorkItem(ctx, options, base, work.PullRequestID(strconv.FormatInt(created.ID, 10)), id); err != nil {
			return work.PullRequestCreateResult{}, err
		}
	}
	return work.PullRequestCreateResult{ID: work.PullRequestID(strconv.FormatInt(created.ID, 10)), URL: created.Links.Self.Href, WebURL: created.Links.HTML.Href}, nil
}

func (provider *Provider) LinkPullRequestWorkItem(ctx context.Context, reference work.ProjectRef, repository work.RepositoryName, pullRequestID work.PullRequestID, itemID work.ItemID) error {
	options, err := provider.options(reference)
	if err != nil {
		return err
	}
	base, err := bitbucketRepositoryPath(options, repository)
	if err != nil {
		return err
	}
	return provider.linkPullRequestWorkItem(ctx, options, base, pullRequestID, itemID)
}

func (provider *Provider) linkPullRequestWorkItem(ctx context.Context, options Options, base string, pullRequestID work.PullRequestID, itemID work.ItemID) error {
	number, err := pullRequestNumber(pullRequestID)
	if err != nil {
		return err
	}
	path := base + "/pullrequests/" + strconv.FormatInt(number, 10)
	var source bitbucketPullRequest
	if _, err := provider.bitbucketRequest(ctx, options, http.MethodGet, path, nil, nil, &source); err != nil {
		return err
	}
	reference := string(itemID)
	if containsJiraReference(source.Description, reference) {
		return nil
	}
	description := strings.TrimSpace(source.Description)
	if description != "" {
		description += "\n\n"
	}
	description += reference
	_, err = provider.bitbucketRequest(ctx, options, http.MethodPut, path, nil, map[string]any{"title": source.Title, "description": description}, nil)
	return err
}

func projectBitbucketPull(source bitbucketPullRequest, repository string) work.PullRequest {
	return work.PullRequest{ID: work.PullRequestID(strconv.FormatInt(source.ID, 10)), Repository: work.RepositoryName(repository), Title: source.Title, Status: source.State, SourceRef: "refs/heads/" + trimBranch(source.Source.Branch.Name), TargetRef: "refs/heads/" + trimBranch(source.Destination.Branch.Name), Draft: source.Draft, CreatedBy: source.Author.DisplayName, URL: source.Links.Self.Href, WebURL: source.Links.HTML.Href, WorkItemIDs: extractCommitReferences(source.Title + "\n" + source.Description)}
}
func bitbucketState(state string) string {
	switch strings.ToLower(strings.TrimSpace(state)) {
	case "active", "open":
		return "OPEN"
	case "completed", "merged":
		return "MERGED"
	case "abandoned", "declined":
		return "DECLINED"
	default:
		return strings.ToUpper(strings.TrimSpace(state))
	}
}
func trimBranch(value string) string {
	return strings.TrimPrefix(strings.TrimSpace(value), "refs/heads/")
}
func pullRequestNumber(id work.PullRequestID) (int64, error) {
	value, err := strconv.ParseInt(string(id), 10, 64)
	if err != nil || value < 1 {
		return 0, fmt.Errorf("atlassian.invalid-pull-request-id:%s", id)
	}
	return value, nil
}
func containsJiraReference(description, reference string) bool {
	for _, id := range extractCommitReferences(description) {
		if strings.EqualFold(string(id), reference) {
			return true
		}
	}
	return false
}

var (
	_ work.PullRequestReader = (*Provider)(nil)
	_ work.PullRequestWriter = (*Provider)(nil)
)
