package github

import (
	"context"
	"fmt"
	"net/http"
	"net/url"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
)

type pullRequest struct {
	Number  int64  `json:"number"`
	Title   string `json:"title"`
	Body    string `json:"body"`
	State   string `json:"state"`
	Draft   bool   `json:"draft"`
	URL     string `json:"url"`
	HTMLURL string `json:"html_url"`
	User    struct {
		Login string `json:"login"`
	} `json:"user"`
	Head struct {
		Ref   string `json:"ref"`
		Label string `json:"label"`
	} `json:"head"`
	Base struct {
		Ref string `json:"ref"`
	} `json:"base"`
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
		base, pathErr := repositoryPath(options, repository)
		if pathErr != nil {
			return nil, pathErr
		}
		state := strings.ToLower(strings.TrimSpace(query.Status))
		if state == "active" {
			state = "open"
		}
		if state == "completed" {
			state = "closed"
		}
		if state == "" {
			state = "open"
		}
		parameters := url.Values{"state": {state}, "per_page": {"100"}}
		var pulls []pullRequest
		if _, err := provider.request(ctx, options, http.MethodGet, base+"/pulls", parameters, nil, &pulls); err != nil {
			return nil, err
		}
		_, repositoryName, _ := repositoryParts(options, repository)
		for _, source := range pulls {
			result = append(result, projectPullRequest(source, repositoryName))
		}
	}
	return result, nil
}

func (provider *Provider) ActivePullRequest(ctx context.Context, reference work.ProjectRef, repository work.RepositoryName, sourceRef string) (*work.PullRequest, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(options, repository)
	if err != nil {
		return nil, err
	}
	owner, _, _ := repositoryParts(options, repository)
	branch := trimRef(sourceRef)
	parameters := url.Values{"state": {"open"}, "head": {owner + ":" + branch}, "per_page": {"1"}}
	var pulls []pullRequest
	if _, err := provider.request(ctx, options, http.MethodGet, base+"/pulls", parameters, nil, &pulls); err != nil {
		return nil, err
	}
	if len(pulls) == 0 {
		return nil, nil
	}
	_, repositoryName, _ := repositoryParts(options, repository)
	projected := projectPullRequest(pulls[0], repositoryName)
	return &projected, nil
}

func (provider *Provider) PullRequestWorkItemIDs(ctx context.Context, reference work.ProjectRef, repository work.RepositoryName, id work.PullRequestID) ([]work.ItemID, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(options, repository)
	if err != nil {
		return nil, err
	}
	number, err := strconv.ParseInt(string(id), 10, 64)
	if err != nil || number < 1 {
		return nil, fmt.Errorf("github.invalid-pull-request-id:%s", id)
	}
	var source pullRequest
	if _, err := provider.request(ctx, options, http.MethodGet, base+"/pulls/"+strconv.FormatInt(number, 10), nil, nil, &source); err != nil {
		return nil, err
	}
	return provider.ExtractCommitReferences(source.Title + "\n" + source.Body), nil
}

func (provider *Provider) CreatePullRequest(ctx context.Context, reference work.ProjectRef, request work.PullRequestCreate) (work.PullRequestCreateResult, error) {
	options, err := provider.options(reference)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	base, err := repositoryPath(options, request.Repository)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	body := map[string]any{"title": request.Title, "body": request.Description, "head": trimRef(request.SourceRef), "base": trimRef(request.TargetRef), "draft": request.Draft}
	var created pullRequest
	if _, err := provider.request(ctx, options, http.MethodPost, base+"/pulls", nil, body, &created); err != nil {
		return work.PullRequestCreateResult{}, err
	}
	for _, id := range request.WorkItemIDs {
		if err := provider.linkPullRequestWorkItem(ctx, options, base, work.PullRequestID(strconv.FormatInt(created.Number, 10)), id); err != nil {
			return work.PullRequestCreateResult{}, err
		}
	}
	return work.PullRequestCreateResult{ID: work.PullRequestID(strconv.FormatInt(created.Number, 10)), URL: created.URL, WebURL: created.HTMLURL}, nil
}

func (provider *Provider) LinkPullRequestWorkItem(ctx context.Context, reference work.ProjectRef, repository work.RepositoryName, pullRequestID work.PullRequestID, itemID work.ItemID) error {
	options, err := provider.options(reference)
	if err != nil {
		return err
	}
	base, err := repositoryPath(options, repository)
	if err != nil {
		return err
	}
	return provider.linkPullRequestWorkItem(ctx, options, base, pullRequestID, itemID)
}

func (provider *Provider) linkPullRequestWorkItem(ctx context.Context, options Options, base string, pullRequestID work.PullRequestID, itemID work.ItemID) error {
	number, err := strconv.ParseInt(string(pullRequestID), 10, 64)
	if err != nil || number < 1 {
		return fmt.Errorf("github.invalid-pull-request-id:%s", pullRequestID)
	}
	if _, err := parseID(itemID); err != nil {
		return err
	}
	var source pullRequest
	path := base + "/pulls/" + strconv.FormatInt(number, 10)
	if _, err := provider.request(ctx, options, http.MethodGet, path, nil, nil, &source); err != nil {
		return err
	}
	reference := "Closes #" + string(itemID)
	if strings.Contains(source.Body, reference) {
		return nil
	}
	body := strings.TrimSpace(source.Body)
	if body != "" {
		body += "\n\n"
	}
	body += reference
	_, err = provider.request(ctx, options, http.MethodPatch, path, nil, map[string]any{"body": body}, nil)
	return err
}

func projectPullRequest(source pullRequest, repository string) work.PullRequest {
	return work.PullRequest{ID: work.PullRequestID(strconv.FormatInt(source.Number, 10)), Repository: work.RepositoryName(repository), Title: source.Title, Status: source.State, SourceRef: "refs/heads/" + trimRef(source.Head.Ref), TargetRef: "refs/heads/" + trimRef(source.Base.Ref), Draft: source.Draft, CreatedBy: source.User.Login, URL: source.URL, WebURL: source.HTMLURL, WorkItemIDs: extractCommitReferences(source.Title + "\n" + source.Body)}
}
func trimRef(value string) string {
	value = strings.TrimSpace(value)
	value = strings.TrimPrefix(value, "refs/heads/")
	return value
}

var (
	_ work.PullRequestReader = (*Provider)(nil)
	_ work.PullRequestWriter = (*Provider)(nil)
)
