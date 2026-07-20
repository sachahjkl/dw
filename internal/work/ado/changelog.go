package ado

import (
	"context"
	"sort"
	"strconv"
	"strings"
)

func ExtractWorkItemIDsFromCommitMessages(commitLog string) []string {
	result := make([]string, 0)
	seen := make(map[string]struct{})
	for offset := 0; offset < len(commitLog); {
		index := strings.IndexByte(commitLog[offset:], '#')
		if index < 0 {
			break
		}
		start := offset + index + 1
		end := start
		for end < len(commitLog) && commitLog[end] >= '0' && commitLog[end] <= '9' {
			end++
		}
		if end > start {
			id := commitLog[start:end]
			if _, exists := seen[id]; !exists {
				seen[id] = struct{}{}
				result = append(result, id)
			}
		}
		offset = start
	}
	return result
}

func (p *Provider) GroupWorkItemsByParent(ctx context.Context, options Options, items []WorkItemSnapshot, token Token) ([]WorkItemGroup, error) {
	groups := make(map[string][]WorkItemSnapshot)
	parents := make(map[string]WorkItemSnapshot)
	parentIDs := make([]string, 0)
	for _, item := range items {
		related, err := p.GetRelatedWorkItemIDs(ctx, options, item.ID, RelationHierarchyReverse, token)
		if err != nil {
			return nil, err
		}
		parentID := item.ID
		if len(related) != 0 {
			parentID = related[0]
		}
		if _, exists := groups[parentID]; !exists {
			groups[parentID] = make([]WorkItemSnapshot, 0)
			parentIDs = append(parentIDs, parentID)
		}
		if parentID == item.ID {
			parents[parentID] = item
		} else {
			if _, exists := parents[parentID]; !exists {
				parent, err := p.GetWorkItem(ctx, options, parentID, token)
				if err != nil {
					return nil, err
				}
				parents[parentID] = parent
			}
			groups[parentID] = append(groups[parentID], item)
		}
	}
	sort.Strings(parentIDs)
	result := make([]WorkItemGroup, 0, len(parentIDs))
	for _, parentID := range parentIDs {
		parent, exists := parents[parentID]
		if !exists {
			continue
		}
		children := groups[parentID]
		sort.Slice(children, func(i, j int) bool { return children[i].ID < children[j].ID })
		result = append(result, WorkItemGroup{Parent: parent, Items: children})
	}
	return result, nil
}

func (p *Provider) ResolvePullRequestWorkItemIDs(ctx context.Context, options Options, repositories, pullRequestIDs []string, token Token) ([]string, error) {
	if len(repositories) == 0 {
		return nil, &Error{Kind: ErrorInvalidInput, Detail: "PR mode requires an explicit repository, or a project with configured AzureDevOpsRepository entries."}
	}
	ids := make([]string, 0)
	for _, pullRequestID := range pullRequestIDs {
		numericID, err := strconv.ParseInt(pullRequestID, 10, 64)
		if err != nil {
			return nil, &Error{Kind: ErrorInvalidInput, Detail: "Invalid pull request ID: " + pullRequestID, Cause: err}
		}
		type match struct {
			repository string
			ids        []string
		}
		matches := make([]match, 0)
		for _, repository := range repositories {
			workItemIDs, found, err := p.TryGetPullRequestWorkItemIDs(ctx, options, repository, numericID, token)
			if err != nil {
				return nil, err
			}
			if found {
				matches = append(matches, match{repository: repository, ids: workItemIDs})
			}
		}
		switch len(matches) {
		case 0:
			return nil, &Error{Kind: ErrorRequest, Detail: "Pull request #" + pullRequestID + " was not found in tested Azure DevOps repos: " + strings.Join(repositories, ", ")}
		case 1:
			ids = append(ids, matches[0].ids...)
		default:
			names := make([]string, len(matches))
			for index, value := range matches {
				names[index] = value.repository
			}
			return nil, &Error{Kind: ErrorInvalidInput, Detail: "Pull request #" + pullRequestID + " was found in multiple repos (" + strings.Join(names, ", ") + "). Specify the repository."}
		}
	}
	seen := make(map[string]struct{})
	unique := make([]string, 0, len(ids))
	for _, id := range ids {
		if _, exists := seen[id]; !exists {
			seen[id] = struct{}{}
			unique = append(unique, id)
		}
	}
	return unique, nil
}

func (p *Provider) ActiveChildItems(ctx context.Context, options Options, parentID string, token Token) ([]WorkItemSnapshot, error) {
	ids, err := p.GetRelatedWorkItemIDs(ctx, options, parentID, RelationHierarchyForward, token)
	if err != nil {
		return nil, err
	}
	seen := make(map[string]struct{})
	unique := make([]string, 0, len(ids))
	for _, id := range ids {
		if _, exists := seen[id]; !exists {
			seen[id] = struct{}{}
			unique = append(unique, id)
		}
	}
	items, err := p.GetWorkItemsBatch(ctx, options, unique, token)
	if err != nil {
		return nil, err
	}
	active := items[:0]
	for _, item := range items {
		if !IsFinalState(valueOrEmpty(item.Type), valueOrEmpty(item.State)) {
			active = append(active, item)
		}
	}
	return active, nil
}
