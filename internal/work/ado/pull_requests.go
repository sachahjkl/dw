package ado

import (
	"context"
	"sort"
	"strconv"
	"strings"
	"sync"

	"github.com/sachahjkl/dw/internal/work"
)

func (p *Provider) TryGetPullRequestWorkItemIDs(ctx context.Context, options Options, repository string, pullRequestID int64, token Token) ([]string, bool, error) {
	body, found, err := p.transport().GetOptional404(ctx, PullRequestWorkItemsURL(options, repository, pullRequestID), token)
	if err != nil || !found {
		return nil, found, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, false, err
	}
	ids := make([]string, 0)
	for _, value := range array(root["value"]) {
		if id := elementText(object(value)["id"]); id != nil {
			ids = append(ids, *id)
		}
	}
	return ids, true, nil
}

func (p *Provider) FindActivePullRequest(ctx context.Context, options Options, repository, sourceRef string, token Token) (*PullRequestSummary, error) {
	body, err := p.transport().Get(ctx, ActivePullRequestsURL(options, repository, sourceRef), token)
	if err != nil {
		return nil, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, err
	}
	for _, value := range array(root["value"]) {
		item := object(value)
		id, ok := int64Value(item["pullRequestId"])
		if !ok {
			continue
		}
		source, _ := item["sourceRefName"].(string)
		if strings.EqualFold(source, sourceRef) {
			return &PullRequestSummary{PullRequestID: id, URL: fieldText(item, "url")}, nil
		}
	}
	return nil, nil
}

func (p *Provider) ListActivePullRequests(ctx context.Context, options Options, repository string, token Token) ([]PullRequestListItem, error) {
	body, err := p.transport().Get(ctx, ActivePullRequestsForRepositoryURL(options, repository), token)
	if err != nil {
		return nil, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, err
	}
	result := make([]PullRequestListItem, 0)
	for _, value := range array(root["value"]) {
		item := object(value)
		id, ok := int64Value(item["pullRequestId"])
		if !ok {
			continue
		}
		workItemIDs, _, err := p.TryGetPullRequestWorkItemIDs(ctx, options, repository, id, token)
		if err != nil {
			return nil, err
		}
		webURL := nestedString(item, "_links", "web", "href")
		if webURL == nil {
			webURL = stringPointer(PullRequestWebURL(options, repository, id))
		}
		result = append(result, PullRequestListItem{
			Repository: repository, PullRequestID: id, Title: fieldText(item, "title"), Status: fieldText(item, "status"), SourceRefName: fieldText(item, "sourceRefName"), TargetRefName: fieldText(item, "targetRefName"), IsDraft: boolValue(item["isDraft"]), CreatedBy: identityText(item["createdBy"]), URL: fieldText(item, "url"), WebURL: webURL, WorkItemIDs: workItemIDs,
		})
	}
	return result, nil
}

func (p *Provider) CreateADOPullRequest(ctx context.Context, options Options, input CreatePullRequestInput, token Token) (PullRequestCreateResult, error) {
	refs := make([]struct {
		ID string `json:"id"`
	}, len(input.WorkItemIDs))
	for index, id := range input.WorkItemIDs {
		refs[index].ID = id
	}
	body, err := p.transport().Post(ctx, PullRequestsURL(options, input.Repository), token, struct {
		SourceRefName string `json:"sourceRefName"`
		TargetRefName string `json:"targetRefName"`
		Title         string `json:"title"`
		Description   string `json:"description"`
		IsDraft       bool   `json:"isDraft"`
		WorkItemRefs  any    `json:"workItemRefs"`
	}{input.SourceRefName, input.TargetRefName, input.Title, input.Description, input.IsDraft, refs})
	if err != nil {
		return PullRequestCreateResult{}, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return PullRequestCreateResult{}, err
	}
	result := PullRequestCreateResult{URL: fieldText(root, "url")}
	if id, ok := int64Value(root["pullRequestId"]); ok {
		result.PullRequestID = &id
	}
	return result, nil
}

func (p *Provider) LinkWorkItemToPullRequest(ctx context.Context, options Options, repository string, pullRequestID int64, workItemID string, token Token) error {
	_, err := p.transport().Patch(ctx, PullRequestWorkItemsURL(options, repository, pullRequestID), token, []struct {
		ID string `json:"id"`
	}{{ID: workItemID}}, "application/json")
	return err
}

func (p *Provider) ListPullRequests(ctx context.Context, project work.ProjectRef, query work.PullRequestQuery) ([]work.PullRequest, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	options := adoOptions
	type response struct {
		items []PullRequestListItem
		err   error
	}
	responses := make(chan response, len(query.Repositories))
	var group sync.WaitGroup
	for _, repository := range query.Repositories {
		group.Add(1)
		go func(repository work.RepositoryName) {
			defer group.Done()
			items, err := p.ListActivePullRequests(ctx, options, string(repository), token)
			responses <- response{items: items, err: err}
		}(repository)
	}
	group.Wait()
	close(responses)
	items := make([]PullRequestListItem, 0)
	for response := range responses {
		if response.err != nil {
			return nil, response.err
		}
		items = append(items, response.items...)
	}
	sort.Slice(items, func(i, j int) bool {
		if items[i].Repository == items[j].Repository {
			return items[i].PullRequestID < items[j].PullRequestID
		}
		return items[i].Repository < items[j].Repository
	})
	result := make([]work.PullRequest, len(items))
	for index, item := range items {
		result[index] = genericPullRequest(item)
	}
	return result, nil
}

func (p *Provider) ActivePullRequest(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, sourceRef string) (*work.PullRequest, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	options := adoOptions
	summary, err := p.FindActivePullRequest(ctx, options, string(repository), sourceRef, token)
	if err != nil || summary == nil {
		return nil, err
	}
	result := work.PullRequest{ID: work.PullRequestID(strconv.FormatInt(summary.PullRequestID, 10)), Repository: repository, SourceRef: sourceRef}
	if summary.URL != nil {
		result.URL = *summary.URL
	}
	result.WebURL = PullRequestWebURL(options, string(repository), summary.PullRequestID)
	return &result, nil
}

func (p *Provider) PullRequestWorkItemIDs(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, pullRequestID work.PullRequestID) ([]work.ItemID, error) {
	id, err := strconv.ParseInt(string(pullRequestID), 10, 64)
	if err != nil {
		return nil, &Error{Kind: ErrorInvalidInput, Detail: "Invalid pull request ID: " + string(pullRequestID)}
	}
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	ids, found, err := p.TryGetPullRequestWorkItemIDs(ctx, adoOptions, string(repository), id, token)
	if err != nil {
		return nil, err
	}
	if !found {
		return nil, &Error{Kind: ErrorRequest, Detail: "Pull request #" + string(pullRequestID) + " was not found in Azure DevOps repository " + string(repository)}
	}
	result := make([]work.ItemID, len(ids))
	for index, value := range ids {
		result[index] = work.ItemID(value)
	}
	return result, nil
}

func (p *Provider) CreatePullRequest(ctx context.Context, project work.ProjectRef, create work.PullRequestCreate) (work.PullRequestCreateResult, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	ids := make([]string, len(create.WorkItemIDs))
	for index, id := range create.WorkItemIDs {
		ids[index] = string(id)
	}
	result, err := p.CreateADOPullRequest(ctx, adoOptions, CreatePullRequestInput{Repository: string(create.Repository), SourceRefName: create.SourceRef, TargetRefName: create.TargetRef, Title: create.Title, Description: create.Description, IsDraft: create.Draft, WorkItemIDs: ids}, token)
	if err != nil {
		return work.PullRequestCreateResult{}, err
	}
	mapped := work.PullRequestCreateResult{URL: valueOrEmpty(result.URL)}
	if result.PullRequestID != nil {
		mapped.ID = work.PullRequestID(strconv.FormatInt(*result.PullRequestID, 10))
		mapped.WebURL = PullRequestWebURL(adoOptions, string(create.Repository), *result.PullRequestID)
	}
	return mapped, nil
}

func (p *Provider) LinkPullRequestWorkItem(ctx context.Context, project work.ProjectRef, repository work.RepositoryName, pullRequestID work.PullRequestID, itemID work.ItemID) error {
	id, err := strconv.ParseInt(string(pullRequestID), 10, 64)
	if err != nil {
		return &Error{Kind: ErrorInvalidInput, Detail: "Invalid pull request ID: " + string(pullRequestID)}
	}
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return err
	}
	return p.LinkWorkItemToPullRequest(ctx, adoOptions, string(repository), id, string(itemID), token)
}

func genericPullRequest(item PullRequestListItem) work.PullRequest {
	result := work.PullRequest{ID: work.PullRequestID(strconv.FormatInt(item.PullRequestID, 10)), Repository: work.RepositoryName(item.Repository), Draft: item.IsDraft}
	if item.Title != nil {
		result.Title = *item.Title
	}
	if item.Status != nil {
		result.Status = *item.Status
	}
	if item.SourceRefName != nil {
		result.SourceRef = *item.SourceRefName
	}
	if item.TargetRefName != nil {
		result.TargetRef = *item.TargetRefName
	}
	if item.CreatedBy != nil {
		result.CreatedBy = *item.CreatedBy
	}
	if item.URL != nil {
		result.URL = *item.URL
	}
	if item.WebURL != nil {
		result.WebURL = *item.WebURL
	}
	result.WorkItemIDs = make([]work.ItemID, len(item.WorkItemIDs))
	for index, id := range item.WorkItemIDs {
		result.WorkItemIDs[index] = work.ItemID(id)
	}
	return result
}

func nestedString(root map[string]any, path ...string) *string {
	var value any = root
	for _, name := range path {
		value = object(value)[name]
	}
	return elementText(value)
}
