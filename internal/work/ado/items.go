package ado

import (
	"context"
	"strconv"
	"strings"
	"sync"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work"
)

const AssignedWIQL = "select [System.Id]\nfrom WorkItems\nwhere [System.TeamProject] = @project\n  and [System.AssignedTo] = @Me\norder by [System.ChangedDate] desc"

func (p *Provider) GetWorkItem(ctx context.Context, options Options, id string, token Token) (WorkItemSnapshot, error) {
	body, err := p.transport().Get(ctx, WorkItemURL(options, id), token)
	if err != nil {
		return WorkItemSnapshot{}, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return WorkItemSnapshot{}, err
	}
	return snapshotFromObject(root), nil
}

const (
	workItemsBatchLimit       = 200
	workItemsBatchConcurrency = 4
)

var workItemsBatchFields = []string{"System.Id", "System.WorkItemType", "System.State", "System.Title"}

type workItemsBatchRequest struct {
	IDs         []uint64 `json:"ids"`
	Fields      []string `json:"fields,omitempty"`
	Expand      string   `json:"$expand,omitempty"`
	ErrorPolicy string   `json:"errorPolicy"`
}

func (p *Provider) GetWorkItemsBatch(ctx context.Context, options Options, ids []string, token Token) ([]WorkItemSnapshot, error) {
	objects, err := p.workItemObjects(ctx, options, ids, false, token)
	if err != nil {
		return nil, err
	}
	result := make([]WorkItemSnapshot, 0, len(objects))
	for _, value := range objects {
		result = append(result, snapshotFromObject(value))
	}
	return result, nil
}

// workItemObjects returns the accessible work items in request order; unknown,
// inaccessible, or non-numeric IDs are omitted.
func (p *Provider) workItemObjects(ctx context.Context, options Options, ids []string, relations bool, token Token) ([]map[string]any, error) {
	numeric := make([]uint64, 0, len(ids))
	seen := make(map[uint64]struct{}, len(ids))
	for _, id := range ids {
		value, err := strconv.ParseUint(strings.TrimSpace(id), 10, 64)
		if err != nil {
			continue
		}
		if _, exists := seen[value]; !exists {
			seen[value] = struct{}{}
			numeric = append(numeric, value)
		}
	}
	if len(numeric) == 0 {
		return make([]map[string]any, 0), nil
	}
	chunks := make([][]uint64, 0, (len(numeric)+workItemsBatchLimit-1)/workItemsBatchLimit)
	for start := 0; start < len(numeric); start += workItemsBatchLimit {
		chunks = append(chunks, numeric[start:min(start+workItemsBatchLimit, len(numeric))])
	}
	transport := p.transport()
	url := WorkItemsBatchURL(options)
	values := make([][]any, len(chunks))
	errs := make([]error, len(chunks))
	limiter := make(chan struct{}, workItemsBatchConcurrency)
	var group sync.WaitGroup
	for index, chunk := range chunks {
		group.Add(1)
		go func() {
			defer group.Done()
			limiter <- struct{}{}
			defer func() { <-limiter }()
			request := workItemsBatchRequest{IDs: chunk, ErrorPolicy: "Omit"}
			if relations {
				request.Expand = "relations"
			} else {
				request.Fields = workItemsBatchFields
			}
			body, err := transport.Post(ctx, url, token, request)
			if err != nil {
				errs[index] = err
				return
			}
			root, err := decodeObject(body)
			if err != nil {
				errs[index] = err
				return
			}
			values[index] = array(root["value"])
		}()
	}
	group.Wait()
	for _, err := range errs {
		if err != nil {
			return nil, err
		}
	}
	byID := make(map[string]map[string]any)
	for _, chunk := range values {
		for _, value := range chunk {
			item := object(value)
			if id := elementText(item["id"]); item != nil && id != nil {
				byID[*id] = item
			}
		}
	}
	result := make([]map[string]any, 0, len(byID))
	for _, id := range numeric {
		if item, found := byID[strconv.FormatUint(id, 10)]; found {
			result = append(result, item)
		}
	}
	return result, nil
}

func parentIDFromObject(item map[string]any) *string {
	for _, value := range array(item["relations"]) {
		relation := object(value)
		rel, _ := relation["rel"].(string)
		urlValue, _ := relation["url"].(string)
		if strings.EqualFold(rel, RelationHierarchyReverse) {
			if parent := workItemIDFromRelationURL(urlValue); parent != nil {
				return parent
			}
		}
	}
	return nil
}

func (p *Provider) QueryAssignedItems(ctx context.Context, options Options, top int, token Token) ([]WorkItemSnapshot, error) {
	if top < 0 {
		top = 20
	}
	body, err := p.transport().Post(ctx, WIQLURL(options, top), token, struct {
		Query string `json:"query"`
	}{Query: AssignedWIQL})
	if err != nil {
		return nil, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, err
	}
	ids := make([]string, 0)
	for _, value := range array(root["workItems"]) {
		if len(ids) >= top {
			break
		}
		if id := elementText(object(value)["id"]); id != nil {
			ids = append(ids, *id)
		}
	}
	return p.GetWorkItemsBatch(ctx, options, ids, token)
}

func (p *Provider) GetRelatedWorkItemIDs(ctx context.Context, options Options, id, relation string, token Token) ([]string, error) {
	body, err := p.getExpandedWorkItem(ctx, options, id, token)
	if err != nil {
		return nil, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, err
	}
	result := make([]string, 0)
	for _, value := range array(root["relations"]) {
		item := object(value)
		rel, _ := item["rel"].(string)
		urlValue, _ := item["url"].(string)
		if strings.EqualFold(rel, relation) {
			if related := workItemIDFromRelationURL(urlValue); related != nil {
				result = append(result, *related)
			}
		}
	}
	return result, nil
}

func (p *Provider) ReadItems(ctx context.Context, project work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.Item, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	values := make([]string, len(ids))
	for index, id := range ids {
		values[index] = string(id)
	}
	objects, err := p.workItemObjects(ctx, adoOptions, values, options.IncludeRelations, token)
	if err != nil {
		return nil, err
	}
	items := make([]work.Item, 0, len(objects))
	for _, value := range objects {
		item := snapshotWorkItem(adoOptions, snapshotFromObject(value))
		if options.IncludeRelations {
			if parent := parentIDFromObject(value); parent != nil {
				item.ParentID = contract.Some(work.ItemID(*parent))
			}
		}
		items = append(items, item)
	}
	return items, nil
}

func (p *Provider) QueryAssigned(ctx context.Context, project work.ProjectRef, query work.AssignedQuery) ([]work.Item, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	items, err := p.QueryAssignedItems(ctx, adoOptions, query.Top, token)
	if err != nil {
		return nil, err
	}
	result := make([]work.Item, 0, len(items))
	for _, item := range items {
		if query.ExcludeFinalStates && IsFinalState(valueOrEmpty(item.Type), valueOrEmpty(item.State)) {
			continue
		}
		result = append(result, snapshotWorkItem(adoOptions, item))
	}
	return result, nil
}

func (p *Provider) ReadRelations(ctx context.Context, project work.ProjectRef, ids []work.ItemID) ([]work.Relation, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	result := make([]work.Relation, 0)
	for _, id := range ids {
		body, err := p.getExpandedWorkItem(ctx, adoOptions, string(id), token)
		if err != nil {
			return nil, err
		}
		root, err := decodeObject(body)
		if err != nil {
			return nil, err
		}
		for _, relation := range parseRelations(root) {
			mapped := work.Relation{SourceID: id, Kind: genericRelationKind(relation.Kind)}
			if relation.WorkItemID != nil {
				mapped.TargetID = contract.Some(work.ItemID(*relation.WorkItemID))
			}
			if relation.Name != nil {
				mapped.Name = *relation.Name
			}
			if relation.URL != nil {
				mapped.URL = *relation.URL
			}
			if relation.Comment != nil {
				mapped.Comment = *relation.Comment
			}
			if relation.Artifact != nil {
				mapped.Artifact = *relation.Artifact
			}
			result = append(result, mapped)
		}
	}
	return result, nil
}

func (p *Provider) ReadRawItem(ctx context.Context, project work.ProjectRef, id work.ItemID) (wirejson.Value, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return wirejson.Value{}, err
	}
	body, err := p.transport().Get(ctx, ExpandedWorkItemURL(adoOptions, string(id)), token)
	if err != nil {
		return wirejson.Value{}, err
	}
	value, err := wirejson.Parse(body)
	if err != nil {
		return wirejson.Value{}, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
	}
	return value, nil
}

func snapshotWorkItem(options Options, snapshot WorkItemSnapshot) work.Item {
	item := work.Item{ID: work.ItemID(snapshot.ID), URL: WorkItemWebURL(options, snapshot.ID)}
	if snapshot.Type != nil {
		item.Type = work.ItemType(*snapshot.Type)
	}
	if snapshot.State != nil {
		item.State = work.State(*snapshot.State)
	}
	if snapshot.Title != nil {
		item.Title = *snapshot.Title
	}
	item.Tags = make([]string, 0)
	return item
}

func genericRelationKind(kind string) work.RelationKind {
	switch kind {
	case "parent":
		return work.RelationParent
	case "child":
		return work.RelationChild
	case "predecessor":
		return work.RelationPredecessor
	case "successor":
		return work.RelationSuccessor
	case "attachment":
		return work.RelationAttachment
	default:
		return work.RelationOther
	}
}

func valueOrEmpty(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}
