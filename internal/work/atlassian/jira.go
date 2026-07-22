package atlassian

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"regexp"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work"
)

type jiraIssue struct {
	Key    string `json:"key"`
	Fields struct {
		Summary     string          `json:"summary"`
		Description json.RawMessage `json:"description"`
		Status      struct {
			Name           string `json:"name"`
			StatusCategory struct {
				Key string `json:"key"`
			} `json:"statusCategory"`
		} `json:"status"`
		IssueType struct {
			Name string `json:"name"`
		} `json:"issuetype"`
		Assignee *struct {
			DisplayName  string `json:"displayName"`
			EmailAddress string `json:"emailAddress"`
		} `json:"assignee"`
		Creator struct {
			DisplayName string `json:"displayName"`
		} `json:"creator"`
		Reporter struct {
			DisplayName string `json:"displayName"`
		} `json:"reporter"`
		Parent *struct {
			Key    string `json:"key"`
			Fields struct {
				Summary string `json:"summary"`
			} `json:"fields"`
		} `json:"parent"`
		Labels   []string `json:"labels"`
		Created  string   `json:"created"`
		Updated  string   `json:"updated"`
		Subtasks []struct {
			Key    string `json:"key"`
			Fields struct {
				Summary string `json:"summary"`
			} `json:"fields"`
		} `json:"subtasks"`
		IssueLinks []struct {
			Type struct {
				Inward  string `json:"inward"`
				Outward string `json:"outward"`
			} `json:"type"`
			InwardIssue *struct {
				Key    string `json:"key"`
				Fields struct {
					Summary string `json:"summary"`
				} `json:"fields"`
			} `json:"inwardIssue"`
			OutwardIssue *struct {
				Key    string `json:"key"`
				Fields struct {
					Summary string `json:"summary"`
				} `json:"fields"`
			} `json:"outwardIssue"`
		} `json:"issuelinks"`
		Comment struct {
			Comments []struct {
				Author struct {
					DisplayName string `json:"displayName"`
				} `json:"author"`
				Created string          `json:"created"`
				Body    json.RawMessage `json:"body"`
			} `json:"comments"`
		} `json:"comment"`
	} `json:"fields"`
}

func (provider *Provider) ReadItems(ctx context.Context, reference work.ProjectRef, ids []work.ItemID, _ work.ReadOptions) ([]work.Item, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	items := make([]work.Item, 0, len(ids))
	for _, id := range ids {
		var source jiraIssue
		if _, err := provider.jiraRequest(ctx, options, http.MethodGet, "/rest/api/3/issue/"+url.PathEscape(string(id)), nil, nil, &source); err != nil {
			return nil, err
		}
		items = append(items, jiraItem(options, source))
	}
	return items, nil
}

func (provider *Provider) QueryAssigned(ctx context.Context, reference work.ProjectRef, query work.AssignedQuery) ([]work.Item, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	jql := "project = \"" + strings.ReplaceAll(options.Jira.Project, "\"", "\\\"") + "\" AND assignee = currentUser()"
	if query.ExcludeFinalStates {
		jql += " AND statusCategory != Done"
	}
	jql += " ORDER BY updated DESC"
	maximum := query.Top
	if maximum < 1 || maximum > 100 {
		maximum = 100
	}
	parameters := url.Values{"jql": {jql}, "maxResults": {strconv.Itoa(maximum)}}
	var response struct {
		Issues []jiraIssue `json:"issues"`
	}
	if _, err := provider.jiraRequest(ctx, options, http.MethodGet, "/rest/api/3/search/jql", parameters, nil, &response); err != nil {
		return nil, err
	}
	items := make([]work.Item, len(response.Issues))
	for index, source := range response.Issues {
		items[index] = jiraItem(options, source)
	}
	return items, nil
}

func (provider *Provider) ReadRelations(ctx context.Context, reference work.ProjectRef, ids []work.ItemID) ([]work.Relation, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	result := make([]work.Relation, 0)
	for _, id := range ids {
		var source jiraIssue
		parameters := url.Values{"fields": {"parent,subtasks,issuelinks"}}
		if _, err := provider.jiraRequest(ctx, options, http.MethodGet, "/rest/api/3/issue/"+url.PathEscape(string(id)), parameters, nil, &source); err != nil {
			return nil, err
		}
		if source.Fields.Parent != nil {
			result = append(result, jiraRelation(options, id, work.RelationParent, source.Fields.Parent.Key, source.Fields.Parent.Fields.Summary, "parent"))
		}
		for _, child := range source.Fields.Subtasks {
			result = append(result, jiraRelation(options, id, work.RelationChild, child.Key, child.Fields.Summary, "child"))
		}
		for _, link := range source.Fields.IssueLinks {
			if link.InwardIssue != nil {
				result = append(result, jiraRelation(options, id, work.RelationOther, link.InwardIssue.Key, link.InwardIssue.Fields.Summary, link.Type.Inward))
			}
			if link.OutwardIssue != nil {
				result = append(result, jiraRelation(options, id, work.RelationOther, link.OutwardIssue.Key, link.OutwardIssue.Fields.Summary, link.Type.Outward))
			}
		}
	}
	return result, nil
}

func (provider *Provider) UpdateStates(ctx context.Context, reference work.ProjectRef, changes []work.StateChange) ([]work.StateChangeResult, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	results := make([]work.StateChangeResult, 0, len(changes))
	for _, change := range changes {
		items, err := provider.ReadItems(ctx, reference, []work.ItemID{change.ID}, work.ReadOptions{})
		if err != nil {
			return nil, err
		}
		previous := items[0].State
		if strings.EqualFold(string(previous), string(change.State)) {
			results = append(results, work.StateChangeResult{ID: change.ID, Previous: previous, Current: previous})
			continue
		}
		var response struct {
			Transitions []struct {
				ID   string `json:"id"`
				Name string `json:"name"`
				To   struct {
					Name string `json:"name"`
				} `json:"to"`
			} `json:"transitions"`
		}
		path := "/rest/api/3/issue/" + url.PathEscape(string(change.ID)) + "/transitions"
		if _, err := provider.jiraRequest(ctx, options, http.MethodGet, path, nil, nil, &response); err != nil {
			return nil, err
		}
		transitionID := ""
		for _, transition := range response.Transitions {
			if strings.EqualFold(transition.Name, string(change.State)) || strings.EqualFold(transition.To.Name, string(change.State)) || transition.ID == string(change.State) {
				transitionID = transition.ID
				break
			}
		}
		if transitionID == "" {
			return nil, fmt.Errorf("atlassian.jira-transition-not-found:%s:%s", change.ID, change.State)
		}
		if _, err := provider.jiraRequest(ctx, options, http.MethodPost, path, nil, map[string]any{"transition": map[string]string{"id": transitionID}}, nil); err != nil {
			return nil, err
		}
		if strings.TrimSpace(change.Comment) != "" {
			if _, err := provider.jiraRequest(ctx, options, http.MethodPost, "/rest/api/3/issue/"+url.PathEscape(string(change.ID))+"/comment", nil, map[string]any{"body": adfDocument(change.Comment)}, nil); err != nil {
				return nil, err
			}
		}
		results = append(results, work.StateChangeResult{ID: change.ID, Previous: previous, Current: change.State, Changed: true})
	}
	return results, nil
}

func (*Provider) IsFinalState(_ work.ItemType, state work.State) bool {
	switch strings.ToLower(strings.TrimSpace(string(state))) {
	case "done", "closed", "resolved", "complete", "completed":
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
	fields := map[string]any{"project": map[string]string{"key": options.Jira.Project}, "summary": request.Title, "issuetype": map[string]string{"name": string(request.Type)}, "parent": map[string]string{"key": string(request.ParentID)}}
	if strings.TrimSpace(request.History) != "" {
		fields["description"] = adfDocument(request.History)
	}
	if strings.TrimSpace(request.Area) != "" {
		fields["components"] = []map[string]string{{"name": request.Area}}
	}
	var created struct {
		Key  string `json:"key"`
		Self string `json:"self"`
	}
	if _, err := provider.jiraRequest(ctx, options, http.MethodPost, "/rest/api/3/issue", nil, map[string]any{"fields": fields}, &created); err != nil {
		return work.ChildCreateResult{}, err
	}
	return work.ChildCreateResult{ID: work.ItemID(created.Key), Title: request.Title, URL: strings.TrimRight(options.Jira.URL, "/") + "/browse/" + created.Key}, nil
}

func (provider *Provider) ReadRichContext(ctx context.Context, reference work.ProjectRef, ids []work.ItemID, readOptions work.ReadOptions) ([]work.RichContext, error) {
	options, err := provider.options(reference)
	if err != nil {
		return nil, err
	}
	result := make([]work.RichContext, 0, len(ids))
	for _, id := range ids {
		var source jiraIssue
		parameters := url.Values{"expand": {"renderedFields"}}
		if _, err := provider.jiraRequest(ctx, options, http.MethodGet, "/rest/api/3/issue/"+url.PathEscape(string(id)), parameters, nil, &source); err != nil {
			return nil, err
		}
		rich := work.RichContext{Item: jiraItem(options, source), Description: documentText(source.Fields.Description), CreatedBy: source.Fields.Creator.DisplayName, CreatedDate: contract.Timestamp(source.Fields.Created), ChangedBy: source.Fields.Reporter.DisplayName, ChangedDate: contract.Timestamp(source.Fields.Updated)}
		if readOptions.IncludeRelations {
			rich.Relations, err = provider.ReadRelations(ctx, reference, []work.ItemID{id})
			if err != nil {
				return nil, err
			}
		}
		if readOptions.IncludeComments {
			comments := source.Fields.Comment.Comments
			if readOptions.CommentLimit > 0 && len(comments) > readOptions.CommentLimit {
				comments = comments[len(comments)-readOptions.CommentLimit:]
			}
			for _, comment := range comments {
				rich.Comments = append(rich.Comments, work.Comment{Author: comment.Author.DisplayName, CreatedAt: contract.Timestamp(comment.Created), Text: documentText(comment.Body)})
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
	content, err := provider.jiraRequest(ctx, options, http.MethodGet, "/rest/api/3/issue/"+url.PathEscape(string(id)), nil, nil, nil)
	if err != nil {
		return wirejson.Value{}, err
	}
	return wirejson.Parse(content)
}

var jiraReferencePattern = regexp.MustCompile(`(?i)(?:^|[^A-Z0-9])([A-Z][A-Z0-9]+-[1-9][0-9]*)`)

func (*Provider) ExtractCommitReferences(log string) []work.ItemID {
	return extractCommitReferences(log)
}
func extractCommitReferences(log string) []work.ItemID {
	matches := jiraReferencePattern.FindAllStringSubmatch(log, -1)
	result := make([]work.ItemID, 0, len(matches))
	seen := map[work.ItemID]struct{}{}
	for _, match := range matches {
		id := work.ItemID(strings.ToUpper(match[1]))
		if _, ok := seen[id]; !ok {
			seen[id] = struct{}{}
			result = append(result, id)
		}
	}
	return result
}

func jiraItem(options Options, source jiraIssue) work.Item {
	item := work.Item{ID: work.ItemID(source.Key), Type: work.ItemType(source.Fields.IssueType.Name), State: work.State(source.Fields.Status.Name), Title: source.Fields.Summary, URL: strings.TrimRight(options.Jira.URL, "/") + "/browse/" + source.Key, Tags: append([]string(nil), source.Fields.Labels...)}
	if source.Fields.Assignee != nil {
		item.AssignedTo = source.Fields.Assignee.DisplayName
	}
	if source.Fields.Parent != nil {
		item.ParentID = contract.Some(work.ItemID(source.Fields.Parent.Key))
	}
	return item
}
func jiraRelation(options Options, source work.ItemID, kind work.RelationKind, target, title, relationName string) work.Relation {
	return work.Relation{SourceID: source, Kind: kind, TargetID: contract.Some(work.ItemID(target)), Name: relationName + ": " + title, URL: strings.TrimRight(options.Jira.URL, "/") + "/browse/" + target}
}
func adfDocument(text string) map[string]any {
	return map[string]any{
		"type":    "doc",
		"version": 1,
		"content": []any{map[string]any{
			"type":    "paragraph",
			"content": []any{map[string]any{"type": "text", "text": text}},
		}},
	}
}
func documentText(raw json.RawMessage) string {
	if len(raw) == 0 || string(raw) == "null" {
		return ""
	}
	var plain string
	if json.Unmarshal(raw, &plain) == nil {
		return plain
	}
	var value any
	if json.Unmarshal(raw, &value) != nil {
		return ""
	}
	var parts []string
	collectText(value, &parts)
	return strings.Join(parts, "\n")
}
func collectText(value any, parts *[]string) {
	switch typed := value.(type) {
	case map[string]any:
		if text, ok := typed["text"].(string); ok {
			*parts = append(*parts, text)
		}
		if content, ok := typed["content"].([]any); ok {
			for _, child := range content {
				collectText(child, parts)
			}
		}
	case []any:
		for _, child := range typed {
			collectText(child, parts)
		}
	}
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
