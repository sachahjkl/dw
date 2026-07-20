package workspace

import (
	"context"
	"strconv"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/work"
)

// CapabilityWorkPort adapts the shared provider-neutral work capabilities to
// workspace lifecycle operations. Missing capabilities remain typed
// work.UnsupportedCapabilityError values returned by work.Require.
type CapabilityWorkPort struct {
	Provider   work.Provider
	ProjectRef func(string) work.ProjectRef
}

func (p CapabilityWorkPort) project(key string) work.ProjectRef {
	if p.ProjectRef != nil {
		return p.ProjectRef(key)
	}
	return work.ProjectRef{Key: contract.ProjectKey(key), Project: key}
}
func (p CapabilityWorkPort) GetWorkItems(ctx context.Context, project string, ids []string) ([]WorkItem, error) {
	reader, err := work.Require[work.ItemReader](p.Provider, work.CapabilityItemReader)
	if err != nil {
		return nil, err
	}
	typed := make([]work.ItemID, len(ids))
	for index, id := range ids {
		typed[index] = work.ItemID(id)
	}
	items, err := reader.ReadItems(ctx, p.project(project), typed, work.ReadOptions{})
	if err != nil {
		return nil, err
	}
	result := make([]WorkItem, 0, len(items))
	for _, item := range items {
		kind := string(item.Type)
		state := string(item.State)
		title := item.Title
		url := item.URL
		result = append(result, WorkItem{ID: string(item.ID), Type: optionalString(kind), Title: optionalString(title), State: optionalString(state), URL: optionalString(url)})
	}
	return result, nil
}
func (p CapabilityWorkPort) UpdateWorkItemState(ctx context.Context, project, id, state string) error {
	writer, err := work.Require[work.StateWriter](p.Provider, work.CapabilityStateWriter)
	if err != nil {
		return err
	}
	_, err = writer.UpdateStates(ctx, p.project(project), []work.StateChange{{ID: work.ItemID(id), State: work.State(state), Comment: "work finish: PR ouverte"}})
	return err
}
func (p CapabilityWorkPort) CreateChildTask(ctx context.Context, project string, parent WorkItem, repository, title string) (ChildTask, error) {
	creator, err := work.Require[work.ChildCreator](p.Provider, work.CapabilityChildCreator)
	if err != nil {
		return ChildTask{}, err
	}
	created, err := creator.CreateChild(ctx, p.project(project), work.ChildCreate{ParentID: work.ItemID(parent.ID), Type: work.ItemType("Task"), Title: title, History: "work task child create"})
	if err != nil {
		return ChildTask{}, err
	}
	value := created.Title
	return ChildTask{Repository: repository, ID: string(created.ID), Title: optionalString(value)}, nil
}
func (p CapabilityWorkPort) FindActivePullRequest(ctx context.Context, project, repository, sourceRef string) (*WorkPullRequest, error) {
	reader, err := work.Require[work.PullRequestReader](p.Provider, work.CapabilityPullRequestReader)
	if err != nil {
		return nil, err
	}
	found, err := reader.ActivePullRequest(ctx, p.project(project), work.RepositoryName(repository), sourceRef)
	if err != nil || found == nil {
		return nil, err
	}
	id, err := strconv.ParseInt(string(found.ID), 10, 64)
	if err != nil {
		return nil, err
	}
	url := found.WebURL
	if url == "" {
		url = found.URL
	}
	return &WorkPullRequest{ID: id, URL: optionalString(url)}, nil
}
func (p CapabilityWorkPort) CreatePullRequest(ctx context.Context, project string, input PullRequestInput) (WorkPullRequest, error) {
	writer, err := work.Require[work.PullRequestWriter](p.Provider, work.CapabilityPullRequestWriter)
	if err != nil {
		return WorkPullRequest{}, err
	}
	ids := make([]work.ItemID, len(input.WorkItemIDs))
	for index, id := range input.WorkItemIDs {
		ids[index] = work.ItemID(id)
	}
	created, err := writer.CreatePullRequest(ctx, p.project(project), work.PullRequestCreate{Repository: work.RepositoryName(input.Repository), SourceRef: input.SourceRefName, TargetRef: input.TargetRefName, Title: input.Title, Description: input.Description, Draft: input.IsDraft, WorkItemIDs: ids})
	if err != nil {
		return WorkPullRequest{}, err
	}
	id, err := strconv.ParseInt(string(created.ID), 10, 64)
	if err != nil {
		return WorkPullRequest{}, err
	}
	url := created.WebURL
	if url == "" {
		url = created.URL
	}
	return WorkPullRequest{ID: id, URL: optionalString(url)}, nil
}
func (p CapabilityWorkPort) LinkWorkItemToPullRequest(ctx context.Context, project, repository string, pullRequestID int64, itemID string) error {
	writer, err := work.Require[work.PullRequestWriter](p.Provider, work.CapabilityPullRequestWriter)
	if err != nil {
		return err
	}
	return writer.LinkPullRequestWorkItem(ctx, p.project(project), work.RepositoryName(repository), work.PullRequestID(strconv.FormatInt(pullRequestID, 10)), work.ItemID(itemID))
}

func optionalString(value string) *string {
	if value == "" {
		return nil
	}
	return &value
}
