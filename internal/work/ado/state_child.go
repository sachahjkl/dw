package ado

import (
	"context"
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
)

type jsonPatchOperation struct {
	Op    string `json:"op"`
	Path  string `json:"path"`
	Value any    `json:"value,omitempty"`
	From  string `json:"from,omitempty"`
}

func patchAdd(path string, value any) jsonPatchOperation {
	return jsonPatchOperation{Op: "add", Path: path, Value: value}
}

func (p *Provider) UpdateWorkItemState(ctx context.Context, options Options, id, state, history string, token Token) error {
	_, err := p.transport().Patch(ctx, WorkItemURL(options, id), token, []jsonPatchOperation{
		patchAdd("/fields/System.History", history),
		patchAdd("/fields/System.State", state),
	}, "application/json-patch+json")
	return err
}

func (p *Provider) UpdateStates(ctx context.Context, project work.ProjectRef, changes []work.StateChange) ([]work.StateChangeResult, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return nil, err
	}
	options := adoOptions
	result := make([]work.StateChangeResult, 0, len(changes))
	for _, change := range changes {
		if err := p.UpdateWorkItemState(ctx, options, string(change.ID), string(change.State), change.Comment, token); err != nil {
			return nil, err
		}
		result = append(result, work.StateChangeResult{ID: change.ID, Current: change.State, Changed: true})
	}
	return result, nil
}

func IsFinalState(itemType, state string) bool {
	normalizedState := normalizeStateOrType(state)
	if normalizedState == "" {
		return false
	}
	normalizedType := normalizeStateOrType(itemType)
	validated := normalizedState == "valide"
	finalWithoutValidated := normalizedState == "cloture" || normalizedState == "abandonne"
	switch normalizedType {
	case "user story", "anomalie":
		return validated || finalWithoutValidated
	case "bug", "activite":
		return finalWithoutValidated
	default:
		return validated || finalWithoutValidated
	}
}

func (p *Provider) IsFinalState(itemType work.ItemType, state work.State) bool {
	return IsFinalState(string(itemType), string(state))
}

func normalizeStateOrType(value string) string {
	value = strings.ToLower(strings.TrimSpace(value))
	return strings.NewReplacer("é", "e", "è", "e", "ê", "e", "à", "a", "â", "a", "ô", "o", "û", "u", "ù", "u").Replace(value)
}

func DefaultStartState(itemType string) (string, bool) {
	switch normalizeStateOrType(itemType) {
	case "user story", "anomalie":
		return "En réalisation", true
	case "bug", "activite", "task", "tache":
		return "En développement", true
	default:
		return "", false
	}
}

func DefaultFinishState(itemType string) (string, bool) {
	switch normalizeStateOrType(itemType) {
	case "bug", "activite", "task", "tache":
		return "PR en attente", true
	default:
		return "", false
	}
}

func ChildTaskTitle(repository, title string) string {
	var prefix string
	switch strings.ToLower(repository) {
	case "front":
		prefix = "FRONT"
	case "back":
		prefix = "BACK"
	case "sql", "db", "database":
		prefix = "SQL"
	default:
		prefix = strings.ToUpper(repository)
	}
	return "[" + prefix + "] " + title
}

func (p *Provider) CreateChildTask(ctx context.Context, options Options, parent WorkItemSnapshot, repository, title, source string, token Token) (ChildTaskCreateResult, error) {
	history := fmt.Sprintf("Créé automatiquement par Dev Workflow via %s. Parent #%s. Repository: %s.", source, parent.ID, repository)
	return p.createChild(ctx, options, parent.ID, "Task", title, history, "creation "+source, repository, token)
}

func (p *Provider) CreateChild(ctx context.Context, project work.ProjectRef, create work.ChildCreate) (work.ChildCreateResult, error) {
	adoOptions, token, err := p.session(ctx, project)
	if err != nil {
		return work.ChildCreateResult{}, err
	}
	itemType := string(create.Type)
	if itemType == "" {
		itemType = "Task"
	}
	result, err := p.createChild(ctx, adoOptions, string(create.ParentID), itemType, create.Title, create.History, "", "", token)
	if err != nil {
		return work.ChildCreateResult{}, err
	}
	return work.ChildCreateResult{ID: work.ItemID(result.ID), Title: result.Title, URL: WorkItemWebURL(adoOptions, result.ID)}, nil
}

func (p *Provider) createChild(ctx context.Context, options Options, parentID, itemType, title, history, relationComment, repository string, token Token) (ChildTaskCreateResult, error) {
	assignedTo := ""
	if body, err := p.transport().Get(ctx, ConnectionDataURL(options), token); err == nil {
		if root, decodeErr := decodeObject(body); decodeErr == nil {
			assignedTo = authenticatedIdentity(root)
		}
	}
	patch := []jsonPatchOperation{patchAdd("/fields/System.Title", title)}
	if strings.TrimSpace(assignedTo) != "" {
		patch = append(patch, patchAdd("/fields/System.AssignedTo", assignedTo))
	}
	patch = append(patch, patchAdd("/fields/System.History", history))
	relation := map[string]any{"rel": RelationHierarchyReverse, "url": WorkItemAPIURL(options, parentID), "attributes": map[string]any{"comment": relationComment}}
	patch = append(patch, patchAdd("/relations/-", relation))
	body, err := p.transport().PostWithContentType(ctx, CreateWorkItemURL(options, itemType), token, patch, "application/json-patch+json")
	if err != nil {
		return ChildTaskCreateResult{}, err
	}
	root, err := decodeObject(body)
	if err != nil {
		return ChildTaskCreateResult{}, err
	}
	id := elementText(root["id"])
	result := ChildTaskCreateResult{Repository: repository, Title: title}
	if id != nil {
		result.ID = *id
	}
	return result, nil
}

func authenticatedIdentity(root map[string]any) string {
	identity := object(root["authenticatedUser"])
	properties := object(identity["properties"])
	account := object(properties["Account"])
	for _, value := range []any{account["$value"], identity["uniqueName"], identity["providerDisplayName"]} {
		if text, ok := value.(string); ok && strings.TrimSpace(text) != "" {
			return text
		}
	}
	return ""
}
