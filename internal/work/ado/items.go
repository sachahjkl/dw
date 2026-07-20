package ado

import (
	"context"
	"strconv"
	"strings"

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

func (p *Provider) GetWorkItemsBatch(ctx context.Context, options Options, ids []string, token Token) ([]WorkItemSnapshot, error) {
	numeric := make([]uint64, 0, len(ids))
	for _, id := range ids {
		if value, err := strconv.ParseUint(id, 10, 64); err == nil {
			numeric = append(numeric, value)
		}
	}
	if len(numeric) == 0 {
		return make([]WorkItemSnapshot, 0), nil
	}
	body, err := p.transport().Post(ctx, WorkItemsBatchURL(options), token, struct {
		IDs    []uint64 `json:"ids"`
		Fields []string `json:"fields"`
	}{IDs: numeric, Fields: []string{"System.Id", "System.WorkItemType", "System.State", "System.Title"}})
	if err != nil {
		return nil, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, err
	}
	byID := make(map[string]WorkItemSnapshot)
	for _, value := range array(root["value"]) {
		snapshot := snapshotFromObject(object(value))
		byID[snapshot.ID] = snapshot
	}
	result := make([]WorkItemSnapshot, 0, len(ids))
	for _, id := range ids {
		if item, found := byID[id]; found {
			result = append(result, item)
		}
	}
	return result, nil
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
	snapshots, err := p.GetWorkItemsBatch(ctx, adoOptions, values, token)
	if err != nil {
		return nil, err
	}
	items := make([]work.Item, 0, len(snapshots))
	for _, snapshot := range snapshots {
		item := snapshotWorkItem(snapshot)
		if options.IncludeRelations {
			parents, relationErr := p.GetRelatedWorkItemIDs(ctx, adoOptions, snapshot.ID, RelationHierarchyReverse, token)
			if relationErr != nil {
				return nil, relationErr
			}
			if len(parents) != 0 {
				item.ParentID = contract.Some(work.ItemID(parents[0]))
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
		result = append(result, snapshotWorkItem(item))
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

func snapshotWorkItem(snapshot WorkItemSnapshot) work.Item {
	item := work.Item{ID: work.ItemID(snapshot.ID)}
	if snapshot.Type != nil {
		item.Type = work.ItemType(*snapshot.Type)
	}
	if snapshot.State != nil {
		item.State = work.State(*snapshot.State)
	}
	if snapshot.Title != nil {
		item.Title = *snapshot.Title
	}
	if snapshot.URL != nil {
		item.URL = *snapshot.URL
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
