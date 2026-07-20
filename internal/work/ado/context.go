package ado

import (
	"context"
	"sort"
	"strings"
)

func (p *Provider) GetExpandedWorkItem(ctx context.Context, id string, token Token) ([]byte, error) {
	return p.getExpandedWorkItem(ctx, p.Options, id, token)
}

func (p *Provider) getExpandedWorkItem(ctx context.Context, options Options, id string, token Token) ([]byte, error) {
	return p.transport().Get(ctx, ExpandedWorkItemURL(options, id), token)
}

func (p *Provider) GetComments(ctx context.Context, id string, limit int, token Token) ([]AIContextComment, error) {
	return p.getComments(ctx, p.Options, id, limit, token)
}

func (p *Provider) getComments(ctx context.Context, options Options, id string, limit int, token Token) ([]AIContextComment, error) {
	comments := make([]AIContextComment, 0)
	if limit <= 0 {
		return comments, nil
	}
	body, err := p.transport().Get(ctx, WorkItemCommentsURL(options, id, uint32(limit)), token)
	if err != nil {
		return nil, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return nil, err
	}
	values := array(root["comments"])
	if values == nil {
		values = array(root["value"])
	}
	for _, value := range values {
		comment := object(value)
		text := fieldText(comment, "renderedText")
		if text == nil {
			text = fieldText(comment, "text")
		}
		comments = append(comments, AIContextComment{
			Author:      identityText(firstValue(comment, "createdBy", "author")),
			CreatedDate: fieldText(comment, "createdDate"),
			Text:        cleanText(text),
		})
	}
	return comments, nil
}

func (p *Provider) GetAIContext(ctx context.Context, id string, summary bool, commentLimit int, token Token) (AIContextItem, error) {
	return p.getAIContext(ctx, p.Options, id, summary, commentLimit, token)
}

func (p *Provider) getAIContext(ctx context.Context, options Options, id string, summary bool, commentLimit int, token Token) (AIContextItem, error) {
	expanded, err := p.getExpandedWorkItem(ctx, options, id, token)
	if err != nil {
		return AIContextItem{}, err
	}
	root, err := decodeObject(expanded)
	if err != nil {
		return AIContextItem{}, err
	}
	comments, err := p.getComments(ctx, options, id, commentLimit, token)
	if err != nil {
		comments = make([]AIContextComment, 0)
	}
	return mapAIContext(root, options, summary, comments), nil
}

func mapAIContext(root map[string]any, options Options, summary bool, comments []AIContextComment) AIContextItem {
	fields := object(root["fields"])
	idValue := elementText(root["id"])
	id := ""
	if idValue != nil {
		id = *idValue
	}
	relations := parseRelations(root)
	attachmentDirectory := AttachmentDirectoryPrefix + id
	attachments := make([]AIContextAttachment, 0)
	for _, relation := range relations {
		if relation.Kind == "attachment" {
			attachments = append(attachments, AIContextAttachment{Name: relation.Name, URL: relation.URL, Comment: relation.Comment, DirectoryHint: attachmentDirectory})
		}
	}
	if summary {
		relations = make([]AIContextRelation, 0)
	}
	return AIContextItem{
		SchemaVersion: AIContextVersion,
		WorkItem: AIContextWorkItem{
			ID: id, URL: stringPointer(WorkItemWebURL(options, id)), Title: fieldText(fields, "System.Title"), Type: fieldText(fields, "System.WorkItemType"), State: fieldText(fields, "System.State"), AssignedTo: identityText(fields["System.AssignedTo"]), AreaPath: fieldText(fields, "System.AreaPath"), IterationPath: fieldText(fields, "System.IterationPath"), Tags: splitTags(fieldText(fields, "System.Tags")),
		},
		Core:        AIContextCore{CreatedBy: identityText(fields["System.CreatedBy"]), CreatedDate: fieldText(fields, "System.CreatedDate"), ChangedBy: identityText(fields["System.ChangedBy"]), ChangedDate: fieldText(fields, "System.ChangedDate"), Priority: fieldText(fields, "Microsoft.VSTS.Common.Priority"), ValueArea: fieldText(fields, "Microsoft.VSTS.Common.ValueArea")},
		Content:     AIContextContent{Description: cleanText(fieldText(fields, "System.Description")), AcceptanceCriteria: cleanText(fieldText(fields, "Microsoft.VSTS.Common.AcceptanceCriteria")), ProductContext: extractProductContext(fields)},
		Links:       AIContextLinks{ParentIDs: distinctRelationIDs(relationsOrRaw(root), "parent"), ChildIDs: distinctRelationIDs(relationsOrRaw(root), "child"), PredecessorIDs: distinctRelationIDs(relationsOrRaw(root), "predecessor"), SuccessorIDs: distinctRelationIDs(relationsOrRaw(root), "successor")},
		Attachments: AIContextAttachments{DirectoryHint: attachmentDirectory, Items: attachments}, Relations: relations, Comments: comments,
	}
}

func relationsOrRaw(root map[string]any) []AIContextRelation { return parseRelations(root) }

func parseRelations(root map[string]any) []AIContextRelation {
	result := make([]AIContextRelation, 0)
	for _, raw := range array(root["relations"]) {
		relation := object(raw)
		rel := fieldText(relation, "rel")
		urlValue := fieldText(relation, "url")
		attributes := object(relation["attributes"])
		name := fieldText(attributes, "name")
		comment := cleanText(fieldText(attributes, "comment"))
		var id *string
		if urlValue != nil {
			id = workItemIDFromRelationURL(*urlValue)
		}
		result = append(result, AIContextRelation{Kind: relationKind(rel, id, urlValue), Rel: rel, WorkItemID: id, Name: name, URL: urlValue, Comment: comment, Artifact: nil})
	}
	return result
}

func relationKind(rel, relatedID, relationURL *string) string {
	if rel != nil {
		switch {
		case strings.Contains(*rel, "Hierarchy-Reverse"):
			return "parent"
		case strings.Contains(*rel, "Hierarchy-Forward"):
			return "child"
		case strings.Contains(*rel, "Dependency-Reverse"):
			return "predecessor"
		case strings.Contains(*rel, "Dependency-Forward"):
			return "successor"
		case strings.Contains(*rel, "AttachedFile"):
			return "attachment"
		}
	}
	if relatedID != nil {
		return "work-item"
	}
	if relationURL != nil {
		return "artifact"
	}
	return "relation"
}

func distinctRelationIDs(relations []AIContextRelation, kind string) []string {
	result := make([]string, 0)
	seen := make(map[string]struct{})
	for _, relation := range relations {
		if relation.Kind != kind || relation.WorkItemID == nil {
			continue
		}
		if _, exists := seen[*relation.WorkItemID]; exists {
			continue
		}
		seen[*relation.WorkItemID] = struct{}{}
		result = append(result, *relation.WorkItemID)
	}
	return result
}

func splitTags(value *string) []string {
	result := make([]string, 0)
	if value == nil {
		return result
	}
	for _, tag := range strings.Split(*value, ";") {
		if tag = strings.TrimSpace(tag); tag != "" {
			result = append(result, tag)
		}
	}
	return result
}

func extractProductContext(fields map[string]any) map[string]string {
	keys := make([]string, 0, len(fields))
	for name := range fields {
		if isContextField(name) {
			keys = append(keys, name)
		}
	}
	sort.Strings(keys)
	result := make(map[string]string, len(keys))
	for _, name := range keys {
		if text := cleanText(elementText(fields[name])); text != nil {
			result[friendlyFieldName(name)] = *text
		}
	}
	return result
}

func isContextField(name string) bool {
	normalized := strings.ToLower(strings.NewReplacer(".", "", "_", "", " ", "").Replace(name))
	return strings.Contains(normalized, "acceptance") || strings.Contains(normalized, "productowner") || strings.Contains(normalized, "product") || strings.Contains(normalized, "businessvalue") || strings.EqualFold(name, "Microsoft.VSTS.Common.AcceptanceCriteria")
}

func friendlyFieldName(name string) string {
	name = strings.ReplaceAll(name, "System.", "")
	name = strings.ReplaceAll(name, "Microsoft.VSTS.Common.", "")
	return strings.ReplaceAll(name, "Custom.", "")
}

func firstValue(value map[string]any, names ...string) any {
	for _, name := range names {
		if result, ok := value[name]; ok {
			return result
		}
	}
	return nil
}

func stringPointer(value string) *string { return &value }
