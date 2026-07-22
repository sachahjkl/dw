package github

import (
	"context"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work"
)

type issue struct {
	ID        int64  `json:"id"`
	Number    int64  `json:"number"`
	Title     string `json:"title"`
	Body      string `json:"body"`
	State     string `json:"state"`
	HTMLURL   string `json:"html_url"`
	CreatedAt string `json:"created_at"`
	UpdatedAt string `json:"updated_at"`
	User      struct {
		Login string `json:"login"`
	} `json:"user"`
	Assignee *struct {
		Login string `json:"login"`
	} `json:"assignee"`
	Labels []struct {
		Name string `json:"name"`
	} `json:"labels"`
	PullRequest *struct{} `json:"pull_request"`
	Parent      *struct {
		Number int64 `json:"number"`
	} `json:"parent"`
}

func (provider *Provider) ReadItems(ctx context.Context, reference work.ProjectRef, ids []work.ItemID, _ work.ReadOptions) ([]work.Item, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(options, "")
	if err != nil {
		return nil, err
	}
	items := make([]work.Item, 0, len(ids))
	for _, id := range ids {
		number, parseErr := parseID(id)
		if parseErr != nil {
			return nil, parseErr
		}
		var source issue
		if _, err := provider.request(ctx, options, http.MethodGet, base+"/issues/"+strconv.FormatInt(number, 10), nil, nil, &source); err != nil {
			return nil, err
		}
		items = append(items, issueItem(source))
	}
	return items, nil
}

func (provider *Provider) QueryAssigned(ctx context.Context, reference work.ProjectRef, query work.AssignedQuery) ([]work.Item, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(options, "")
	if err != nil {
		return nil, err
	}
	top := query.Top
	if top < 1 || top > 100 {
		top = 100
	}
	parameters := url.Values{"assignee": {"@me"}, "per_page": {strconv.Itoa(top)}}
	if query.ExcludeFinalStates {
		parameters.Set("state", "open")
	} else {
		parameters.Set("state", "all")
	}
	var source []issue
	if _, err := provider.request(ctx, options, http.MethodGet, base+"/issues", parameters, nil, &source); err != nil {
		return nil, err
	}
	items := make([]work.Item, 0, len(source))
	for _, candidate := range source {
		if candidate.PullRequest == nil {
			items = append(items, issueItem(candidate))
		}
	}
	return items, nil
}

func (provider *Provider) ReadRelations(ctx context.Context, reference work.ProjectRef, ids []work.ItemID) ([]work.Relation, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(options, "")
	if err != nil {
		return nil, err
	}
	relations := make([]work.Relation, 0)
	for _, id := range ids {
		number, parseErr := parseID(id)
		if parseErr != nil {
			return nil, parseErr
		}
		var children []issue
		if _, err := provider.request(ctx, options, http.MethodGet, base+"/issues/"+strconv.FormatInt(number, 10)+"/sub_issues", nil, nil, &children); err != nil {
			return nil, err
		}
		for _, child := range children {
			relations = append(relations, work.Relation{SourceID: id, Kind: work.RelationChild, TargetID: contract.Some(work.ItemID(strconv.FormatInt(child.Number, 10))), Name: child.Title, URL: child.HTMLURL})
		}
	}
	return relations, nil
}

func (provider *Provider) UpdateStates(ctx context.Context, reference work.ProjectRef, changes []work.StateChange) ([]work.StateChangeResult, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(options, "")
	if err != nil {
		return nil, err
	}
	results := make([]work.StateChangeResult, 0, len(changes))
	for _, change := range changes {
		items, readErr := provider.ReadItems(ctx, reference, []work.ItemID{change.ID}, work.ReadOptions{})
		if readErr != nil {
			return nil, readErr
		}
		current := items[0].State
		target := "open"
		if provider.IsFinalState("", change.State) {
			target = "closed"
		}
		if strings.EqualFold(string(current), target) {
			results = append(results, work.StateChangeResult{ID: change.ID, Previous: current, Current: current})
			continue
		}
		number, _ := parseID(change.ID)
		path := base + "/issues/" + strconv.FormatInt(number, 10)
		var updated issue
		if _, err := provider.request(ctx, options, http.MethodPatch, path, nil, map[string]any{"state": target}, &updated); err != nil {
			return nil, err
		}
		if strings.TrimSpace(change.Comment) != "" {
			if _, err := provider.request(ctx, options, http.MethodPost, path+"/comments", nil, map[string]any{"body": change.Comment}, nil); err != nil {
				return nil, err
			}
		}
		results = append(results, work.StateChangeResult{ID: change.ID, Previous: current, Current: work.State(updated.State), Changed: true})
	}
	return results, nil
}

func (*Provider) IsFinalState(_ work.ItemType, state work.State) bool {
	switch strings.ToLower(strings.TrimSpace(string(state))) {
	case "closed", "done", "completed", "resolved":
		return true
	default:
		return false
	}
}

func (provider *Provider) CreateChild(ctx context.Context, reference work.ProjectRef, request work.ChildCreate) (work.ChildCreateResult, error) {
	options, err := provider.options(reference)
	if err != nil {
		return work.ChildCreateResult{}, err
	}
	base, err := repositoryPath(options, "")
	if err != nil {
		return work.ChildCreateResult{}, err
	}
	body := map[string]any{"title": request.Title}
	if strings.TrimSpace(request.History) != "" {
		body["body"] = request.History
	}
	var created issue
	if _, err := provider.request(ctx, options, http.MethodPost, base+"/issues", nil, body, &created); err != nil {
		return work.ChildCreateResult{}, err
	}
	parent, err := parseID(request.ParentID)
	if err != nil {
		return work.ChildCreateResult{}, err
	}
	if _, err := provider.request(ctx, options, http.MethodPost, base+"/issues/"+strconv.FormatInt(parent, 10)+"/sub_issues", nil, map[string]any{"sub_issue_id": created.ID}, nil); err != nil {
		return work.ChildCreateResult{}, err
	}
	return work.ChildCreateResult{ID: work.ItemID(strconv.FormatInt(created.Number, 10)), Title: created.Title, URL: created.HTMLURL}, nil
}

func (provider *Provider) ReadRichContext(ctx context.Context, reference work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.RichContext, error) {
	providerOptions, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	base, err := repositoryPath(providerOptions, "")
	if err != nil {
		return nil, err
	}
	result := make([]work.RichContext, 0, len(ids))
	for _, id := range ids {
		number, parseErr := parseID(id)
		if parseErr != nil {
			return nil, parseErr
		}
		var source issue
		if _, err := provider.request(ctx, providerOptions, http.MethodGet, base+"/issues/"+strconv.FormatInt(number, 10), nil, nil, &source); err != nil {
			return nil, err
		}
		rich := work.RichContext{Item: issueItem(source), Description: source.Body, CreatedBy: source.User.Login, CreatedDate: timestamp(source.CreatedAt), ChangedDate: timestamp(source.UpdatedAt)}
		if options.IncludeRelations {
			rich.Relations, err = provider.ReadRelations(ctx, reference, []work.ItemID{id})
			if err != nil {
				return nil, err
			}
		}
		if options.IncludeComments {
			parameters := url.Values{}
			if options.CommentLimit > 0 {
				parameters.Set("per_page", strconv.Itoa(options.CommentLimit))
			}
			var comments []struct {
				User struct {
					Login string `json:"login"`
				} `json:"user"`
				CreatedAt string `json:"created_at"`
				Body      string `json:"body"`
			}
			if _, err := provider.request(ctx, providerOptions, http.MethodGet, base+"/issues/"+strconv.FormatInt(number, 10)+"/comments", parameters, nil, &comments); err != nil {
				return nil, err
			}
			for _, comment := range comments {
				rich.Comments = append(rich.Comments, work.Comment{Author: comment.User.Login, CreatedAt: timestamp(comment.CreatedAt), Text: comment.Body})
			}
		}
		result = append(result, rich)
	}
	return result, nil
}

func (provider *Provider) ReadRawItem(ctx context.Context, reference work.ProjectRef, id work.ItemID) (wirejson.Value, error) {
	options, err := provider.options(reference)
	if err != nil {
		return wirejson.Value{}, err
	}
	base, err := repositoryPath(options, "")
	if err != nil {
		return wirejson.Value{}, err
	}
	number, err := parseID(id)
	if err != nil {
		return wirejson.Value{}, err
	}
	content, err := provider.request(ctx, options, http.MethodGet, base+"/issues/"+strconv.FormatInt(number, 10), nil, nil, nil)
	if err != nil {
		return wirejson.Value{}, err
	}
	return wirejson.Parse(content)
}

var commitReferencePattern = regexp.MustCompile(`(?:^|[^[:alnum:]_])#([1-9][0-9]*)`)

func (*Provider) ExtractCommitReferences(log string) []work.ItemID {
	return extractCommitReferences(log)
}

func extractCommitReferences(log string) []work.ItemID {
	matches := commitReferencePattern.FindAllStringSubmatch(log, -1)
	result := make([]work.ItemID, 0, len(matches))
	seen := make(map[work.ItemID]struct{}, len(matches))
	for _, match := range matches {
		id := work.ItemID(match[1])
		if _, found := seen[id]; !found {
			seen[id] = struct{}{}
			result = append(result, id)
		}
	}
	return result
}

func issueItem(source issue) work.Item {
	kind := work.ItemType("issue")
	if source.PullRequest != nil {
		kind = "pull-request"
	}
	item := work.Item{ID: work.ItemID(strconv.FormatInt(source.Number, 10)), Type: kind, State: work.State(source.State), Title: source.Title, URL: source.HTMLURL}
	if source.Assignee != nil {
		item.AssignedTo = source.Assignee.Login
	}
	for _, label := range source.Labels {
		item.Tags = append(item.Tags, label.Name)
	}
	if source.Parent != nil {
		item.ParentID = contract.Some(work.ItemID(strconv.FormatInt(source.Parent.Number, 10)))
	}
	return item
}

var (
	_ work.ItemReader               = (*Provider)(nil)
	_ work.AssignedQuerier          = (*Provider)(nil)
	_ work.RelationReader           = (*Provider)(nil)
	_ work.StateWriter              = (*Provider)(nil)
	_ work.StateClassifier          = (*Provider)(nil)
	_ work.ChildCreator             = (*Provider)(nil)
	_ work.RichContextReader        = (*Provider)(nil)
	_ work.RawItemReader            = (*Provider)(nil)
	_ work.CommitReferenceExtractor = (*Provider)(nil)
)
