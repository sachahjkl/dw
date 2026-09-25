package ado

import (
	"context"
	"sort"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
)

const maximumWorkItemIDDigits = 7

func ExtractWorkItemIDsFromCommitMessages(commitLog string) []string {
	result := make([]string, 0)
	seen := make(map[string]struct{})
	for offset := 0; offset < len(commitLog); {
		index := strings.IndexByte(commitLog[offset:], '#')
		if index < 0 {
			break
		}
		hash := offset + index
		start := hash + 1
		end := start
		for end < len(commitLog) && isASCIIDigit(commitLog[end]) {
			end++
		}
		offset = max(end, start)
		if end == start || end-start > maximumWorkItemIDDigits || !workItemReferenceAt(commitLog, hash, end) {
			continue
		}
		id := strings.TrimLeft(commitLog[start:end], "0")
		if id == "" {
			continue
		}
		if _, exists := seen[id]; !exists {
			seen[id] = struct{}{}
			result = append(result, id)
		}
	}
	return result
}

func workItemReferenceAt(text string, hash, end int) bool {
	if end < len(text) && (isWordByte(text[end]) || text[end] == ';') {
		return false
	}
	if hash == 0 {
		return true
	}
	previous := text[hash-1]
	switch {
	case hash >= 2 && strings.EqualFold(text[hash-2:hash], "AB"):
		return hash == 2 || !isWordByte(text[hash-3])
	case isWordByte(previous), previous == '&', previous == '=', previous == '"', previous == '\'', previous == '/':
		return false
	case previous == '(':
		githubPullRequest := (hash == 1 || isSpaceByte(text[hash-2])) && end < len(text) && text[end] == ')'
		return !githubPullRequest
	}
	return true
}

func isASCIIDigit(value byte) bool { return value >= '0' && value <= '9' }

func isWordByte(value byte) bool {
	return isASCIIDigit(value) || value == '_' || (value >= 'a' && value <= 'z') || (value >= 'A' && value <= 'Z') || value >= 0x80
}

func isSpaceByte(value byte) bool {
	return value == ' ' || value == '\t' || value == '\n' || value == '\r'
}

func (*Provider) ExtractCommitReferences(commitLog string) []work.ItemID {
	raw := ExtractWorkItemIDsFromCommitMessages(commitLog)
	result := make([]work.ItemID, len(raw))
	for index := range raw {
		result[index] = work.ItemID(raw[index])
	}
	return result
}

func (p *Provider) GroupWorkItemsByParent(ctx context.Context, options Options, items []WorkItemSnapshot, token Token) ([]WorkItemGroup, error) {
	ids := make([]string, len(items))
	for index, item := range items {
		ids[index] = item.ID
	}
	objects, err := p.workItemObjects(ctx, options, ids, true, token)
	if err != nil {
		return nil, err
	}
	parentOf := make(map[string]string, len(objects))
	for _, value := range objects {
		if id, parent := elementText(value["id"]), parentIDFromObject(value); id != nil && parent != nil {
			parentOf[*id] = *parent
		}
	}
	parentIDs := make([]string, 0)
	for _, item := range items {
		if parentID, found := parentOf[item.ID]; found && !containsText(parentIDs, parentID) {
			parentIDs = append(parentIDs, parentID)
		}
	}
	loaded, err := p.GetWorkItemsBatch(ctx, options, parentIDs, token)
	if err != nil {
		return nil, err
	}
	parents := make(map[string]WorkItemSnapshot, len(loaded))
	for _, parent := range loaded {
		parents[parent.ID] = parent
	}
	groups := make(map[string][]WorkItemSnapshot)
	orphans := make([]WorkItemSnapshot, 0)
	for _, item := range items {
		parentID, found := parentOf[item.ID]
		if !found {
			if _, exists := groups[item.ID]; !exists {
				groups[item.ID] = make([]WorkItemSnapshot, 0)
			}
			parents[item.ID] = item
			continue
		}
		if _, loaded := parents[parentID]; !loaded {
			orphans = append(orphans, item)
			continue
		}
		groups[parentID] = append(groups[parentID], item)
	}
	keys := make([]string, 0, len(groups))
	for key := range groups {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	result := make([]WorkItemGroup, 0, len(keys)+1)
	for _, key := range keys {
		children := groups[key]
		sort.Slice(children, func(i, j int) bool { return children[i].ID < children[j].ID })
		result = append(result, WorkItemGroup{Parent: parents[key], Items: children})
	}
	if len(orphans) != 0 {
		sort.Slice(orphans, func(i, j int) bool { return orphans[i].ID < orphans[j].ID })
		result = append(result, WorkItemGroup{Items: orphans})
	}
	return result, nil
}

func containsText(values []string, value string) bool {
	for _, candidate := range values {
		if candidate == value {
			return true
		}
	}
	return false
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
