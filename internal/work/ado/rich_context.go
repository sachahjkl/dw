package ado

import (
	"context"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work"
)

func (p *Provider) ReadRichContext(ctx context.Context, project work.ProjectRef, ids []work.ItemID, readOptions work.ReadOptions) ([]work.RichContext, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	options := adoOptions
	result := make([]work.RichContext, 0, len(ids))
	for _, id := range ids {
		raw, err := p.getExpandedWorkItem(ctx, options, string(id), token)
		if err != nil {
			return nil, err
		}
		root, err := decodeObject(raw)
		if err != nil {
			return nil, err
		}
		comments := make([]AIContextComment, 0)
		if readOptions.IncludeComments {
			comments, err = p.getComments(ctx, options, string(id), readOptions.CommentLimit, token)
			if err != nil {
				comments = make([]AIContextComment, 0)
			}
		}
		mapped := mapAIContext(root, options, !readOptions.IncludeRelations, comments)
		extra, err := wirejson.Parse(raw)
		if err != nil {
			return nil, &Error{Kind: ErrorJSON, Detail: err.Error(), Cause: err}
		}
		contextItem := work.RichContext{
			Item:        work.Item{ID: work.ItemID(mapped.WorkItem.ID), Tags: append([]string(nil), mapped.WorkItem.Tags...)},
			Extra:       extra,
			Relations:   make([]work.Relation, 0),
			Comments:    make([]work.Comment, 0),
			Attachments: make([]work.Attachment, 0),
		}
		if mapped.WorkItem.Type != nil {
			contextItem.Item.Type = work.ItemType(*mapped.WorkItem.Type)
		}
		if mapped.WorkItem.State != nil {
			contextItem.Item.State = work.State(*mapped.WorkItem.State)
		}
		if mapped.WorkItem.Title != nil {
			contextItem.Item.Title = *mapped.WorkItem.Title
		}
		if mapped.WorkItem.URL != nil {
			contextItem.Item.URL = *mapped.WorkItem.URL
		}
		if mapped.WorkItem.AssignedTo != nil {
			contextItem.Item.AssignedTo = *mapped.WorkItem.AssignedTo
		}
		if mapped.WorkItem.AreaPath != nil {
			contextItem.Item.AreaPath = *mapped.WorkItem.AreaPath
		}
		if mapped.WorkItem.IterationPath != nil {
			contextItem.Item.IterationPath = *mapped.WorkItem.IterationPath
		}
		if mapped.Content.Description != nil {
			contextItem.Description = *mapped.Content.Description
		}
		if mapped.Content.AcceptanceCriteria != nil {
			contextItem.AcceptanceCriteria = *mapped.Content.AcceptanceCriteria
		}
		if mapped.Core.CreatedBy != nil {
			contextItem.CreatedBy = *mapped.Core.CreatedBy
		}
		if mapped.Core.CreatedDate != nil {
			contextItem.CreatedDate = contract.Timestamp(*mapped.Core.CreatedDate)
		}
		if mapped.Core.ChangedBy != nil {
			contextItem.ChangedBy = *mapped.Core.ChangedBy
		}
		if mapped.Core.ChangedDate != nil {
			contextItem.ChangedDate = contract.Timestamp(*mapped.Core.ChangedDate)
		}
		if mapped.Core.Priority != nil {
			contextItem.Priority = *mapped.Core.Priority
		}
		if mapped.Core.ValueArea != nil {
			contextItem.ValueArea = *mapped.Core.ValueArea
		}
		contextItem.ProductContext = make(map[string]string, len(mapped.Content.ProductContext))
		for name, value := range mapped.Content.ProductContext {
			contextItem.ProductContext[name] = value
		}
		for _, relation := range mapped.Relations {
			value := work.Relation{SourceID: work.ItemID(mapped.WorkItem.ID), Kind: genericRelationKind(relation.Kind)}
			if relation.WorkItemID != nil {
				value.TargetID = contract.Some(work.ItemID(*relation.WorkItemID))
			}
			if relation.Name != nil {
				value.Name = *relation.Name
			}
			if relation.URL != nil {
				value.URL = *relation.URL
			}
			if relation.Comment != nil {
				value.Comment = *relation.Comment
			}
			if relation.Artifact != nil {
				value.Artifact = *relation.Artifact
			}
			contextItem.Relations = append(contextItem.Relations, value)
		}
		for _, comment := range mapped.Comments {
			value := work.Comment{}
			if comment.Author != nil {
				value.Author = *comment.Author
			}
			if comment.CreatedDate != nil {
				value.CreatedAt = contract.Timestamp(*comment.CreatedDate)
			}
			if comment.Text != nil {
				value.Text = *comment.Text
			}
			contextItem.Comments = append(contextItem.Comments, value)
		}
		for _, attachment := range mapped.Attachments.Items {
			value := work.Attachment{}
			if attachment.Name != nil {
				value.Name = *attachment.Name
			}
			if attachment.URL != nil {
				value.URL = *attachment.URL
			}
			if attachment.Comment != nil {
				value.Comment = *attachment.Comment
			}
			contextItem.Attachments = append(contextItem.Attachments, value)
		}
		result = append(result, contextItem)
	}
	return result, nil
}
