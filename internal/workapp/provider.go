package workapp

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
	"github.com/sachahjkl/dw/internal/work"
)

type Service struct {
	Providers    *work.Registry
	GitChangelog GitChangelogPort
	Choices      InteractiveCatalog
	Lookup       WorkspaceLookup
	Starter      WorkspaceStarter
	Syncer       WorkspaceSyncer
	Children     WorkspaceChildWriter
	Opener       WorkspaceOpener
	Pruner       WorkspacePruner
	Finisher     WorkspaceFinisher
}

func New(providers *work.Registry) *Service { return &Service{Providers: providers} }

func (s *Service) provider(name string) (work.Provider, error) {
	if s == nil || s.Providers == nil {
		return nil, &work.ProviderNotFoundError{Provider: work.ProviderName(name)}
	}
	if name != "" {
		return s.Providers.Get(work.ProviderName(name))
	}
	providers := s.Providers.Providers()
	if len(providers) == 0 {
		return nil, &work.ProviderNotFoundError{}
	}
	return providers[0], nil
}
func projectRef(root, key string) work.ProjectRef {
	return work.ProjectRef{Key: contract.ProjectKey(key), Root: root}
}

func collectEvent(ctx context.Context, events *[]Event, sink EventSink, event Event) error {
	*events = append(*events, event)
	if sink != nil {
		return sink(ctx, event)
	}
	return nil
}

func (s *Service) AuthLogin(ctx context.Context, request AuthLoginRequest, sink EventSink) (AuthLoginReport, error) {
	provider, err := s.provider(request.Provider)
	if err != nil {
		return AuthLoginReport{}, err
	}
	auth, err := work.Require[work.Authenticator](provider, work.CapabilityAuthenticator)
	if err != nil {
		return AuthLoginReport{}, err
	}
	report := AuthLoginReport{Mode: request.Mode}
	mode := work.AuthBrowser
	switch request.Mode {
	case AuthLoginDeviceCode:
		mode = work.AuthDevice
	case AuthLoginEnvironmentPAT:
		mode = work.AuthEnvironment
		report.UsesEnvironmentPAT = true
	}
	status, err := auth.Login(ctx, projectRef(request.Root, ""), mode, func(device work.DeviceLogin) error {
		event := Event{Kind: "device-login-required", VerificationURI: device.VerificationURI, UserCode: device.UserCode, ExpiresInSeconds: device.ExpiresInSeconds, PollIntervalSeconds: device.PollIntervalSeconds}
		return collectEvent(ctx, &report.Events, sink, event)
	})
	if err != nil {
		return AuthLoginReport{}, err
	}
	if status.Source != "" {
		report.Source = stringPtr(status.Source)
	}
	if expires, ok := status.ExpiresOn.Get(); ok {
		value := expires.String()
		report.ExpiresOn = &value
	}
	return report, nil
}

func (s *Service) AuthStatus(ctx context.Context, request AuthStatusRequest) (AuthStatusReport, error) {
	provider, err := s.provider(request.Provider)
	if err != nil {
		return AuthStatusReport{}, err
	}
	auth, err := work.Require[work.Authenticator](provider, work.CapabilityAuthenticator)
	if err != nil {
		return AuthStatusReport{}, err
	}
	status, err := auth.AuthStatus(ctx, projectRef(request.Root, ""))
	if err != nil {
		return AuthStatusReport{}, err
	}
	report := AuthStatusReport{Connected: status.Authenticated}
	if status.Source != "" {
		report.Source = stringPtr(status.Source)
	}
	if expires, ok := status.ExpiresOn.Get(); ok {
		value := expires.String()
		report.ExpiresOn = &value
	}
	return report, nil
}

func (s *Service) AuthLogout(ctx context.Context, request AuthLogoutRequest) (AuthLogoutReport, error) {
	provider, err := s.provider(request.Provider)
	if err != nil {
		return AuthLogoutReport{}, err
	}
	auth, err := work.Require[work.Authenticator](provider, work.CapabilityAuthenticator)
	if err != nil {
		return AuthLogoutReport{}, err
	}
	removed, err := auth.Logout(ctx, projectRef(request.Root, ""))
	if err != nil {
		return AuthLogoutReport{}, err
	}
	return AuthLogoutReport{RemovedLocalSession: removed}, nil
}

func (s *Service) Assigned(ctx context.Context, request AssignedRequest, sink EventSink) (AssignedReport, error) {
	if request.Project == "" {
		return AssignedReport{}, projectRequired("ado assigned")
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return AssignedReport{}, err
	}
	query, err := work.Require[work.AssignedQuerier](provider, work.CapabilityAssignedQuerier)
	if err != nil {
		return AssignedReport{}, err
	}
	report := AssignedReport{Root: request.Root, Project: request.Project, Top: request.Top, IncludeFinalStates: request.IncludeFinalStates, GroupByParent: request.GroupByParent, Items: []ItemSnapshot{}, Groups: []ItemGroup{}, Events: []Event{}}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "authenticating", Project: stringPtr(request.Project)}); err != nil {
		return AssignedReport{}, err
	}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "loading-assigned-work-items", Project: stringPtr(request.Project), Top: request.Top}); err != nil {
		return AssignedReport{}, err
	}
	top := request.Top
	if top < 0 {
		top = 20
	}
	items, err := query.QueryAssigned(ctx, projectRef(request.Root, request.Project), work.AssignedQuery{Top: top, ExcludeFinalStates: !request.IncludeFinalStates})
	if err != nil {
		return AssignedReport{}, err
	}
	if request.IncludeFinalStates {
		report.Items = itemsToSnapshots(items)
	} else {
		classifier, classifyErr := work.Require[work.StateClassifier](provider, work.CapabilityStateClassifier)
		if classifyErr != nil {
			return AssignedReport{}, classifyErr
		}
		filtered := make([]work.Item, 0, len(items))
		for _, item := range items {
			if !classifier.IsFinalState(item.Type, item.State) {
				filtered = append(filtered, item)
			}
		}
		items = filtered
		report.Items = itemsToSnapshots(items)
	}
	if request.GroupByParent && len(items) > 0 {
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "grouping-assigned-work-items", Project: stringPtr(request.Project)}); err != nil {
			return AssignedReport{}, err
		}
		groups, err := s.groupItems(ctx, provider, request.Root, request.Project, items)
		if err != nil {
			return AssignedReport{}, err
		}
		report.Groups = groups
	}
	return report, nil
}

func (s *Service) PullRequests(ctx context.Context, request PullRequestsRequest, sink EventSink) (PullRequestsReport, error) {
	if request.Project == "" {
		return PullRequestsReport{}, projectRequired("ado prs")
	}
	if len(request.Repositories) == 0 {
		return PullRequestsReport{}, repositoriesRequired("ado prs", "requires an explicit repository, or a project with configured work repositories")
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return PullRequestsReport{}, err
	}
	reader, err := work.Require[work.PullRequestReader](provider, work.CapabilityPullRequestReader)
	if err != nil {
		return PullRequestsReport{}, err
	}
	report := PullRequestsReport{Root: request.Root, Project: request.Project, Repositories: append([]string(nil), request.Repositories...), Items: []PullRequestItem{}, Events: []Event{}}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "loading-pull-requests", Project: stringPtr(request.Project)}); err != nil {
		return PullRequestsReport{}, err
	}
	repositories := make([]work.RepositoryName, len(request.Repositories))
	for i, value := range request.Repositories {
		repositories[i] = work.RepositoryName(value)
	}
	items, err := reader.ListPullRequests(ctx, projectRef(request.Root, request.Project), work.PullRequestQuery{Repositories: repositories, Status: "active"})
	if err != nil {
		return PullRequestsReport{}, err
	}
	projected, err := projectPullRequests(items)
	if err != nil {
		return PullRequestsReport{}, err
	}
	sort.SliceStable(projected, func(i, j int) bool {
		if projected[i].Repository == projected[j].Repository {
			return projected[i].PullRequestID < projected[j].PullRequestID
		}
		return projected[i].Repository < projected[j].Repository
	})
	report.Items = projected
	return report, nil
}

func (s *Service) ItemShow(ctx context.Context, request ItemShowRequest, sink EventSink) (ItemShowReport, error) {
	if request.Project == "" {
		return ItemShowReport{}, projectRequired("ado item show")
	}
	ids := distinctNonEmpty(request.IDs)
	if len(ids) == 0 {
		return ItemShowReport{}, ErrWorkItemsRequired
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return ItemShowReport{}, err
	}
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return ItemShowReport{}, err
	}
	report := ItemShowReport{Root: request.Root, Project: request.Project, RequestedIDs: append([]string(nil), request.IDs...), Items: []ItemSnapshot{}, Events: []Event{}}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "authenticating", Project: stringPtr(request.Project)}); err != nil {
		return ItemShowReport{}, err
	}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "loading-work-items", IDs: append([]string(nil), request.IDs...)}); err != nil {
		return ItemShowReport{}, err
	}
	items, err := reader.ReadItems(ctx, projectRef(request.Root, request.Project), itemIDs(ids), work.ReadOptions{})
	if err != nil {
		return ItemShowReport{}, err
	}
	report.Items = itemsToSnapshots(items)
	return report, nil
}

func PlanState(request StatePlanRequest) (StatePlanReport, error) {
	if request.Project == "" {
		return StatePlanReport{}, projectRequired("ado state set")
	}
	if len(request.IDs) == 0 {
		return StatePlanReport{}, ErrWorkItemsRequired
	}
	return StatePlanReport{Provider: request.Provider, Root: request.Root, Project: request.Project, IDs: append([]string(nil), request.IDs...), State: request.State, History: request.History}, nil
}
func (s *Service) ExecuteState(ctx context.Context, plan StatePlanReport, sink EventSink) (StateExecutionReport, error) {
	provider, err := s.provider(plan.Provider)
	if err != nil {
		return StateExecutionReport{}, err
	}
	writer, err := work.Require[work.StateWriter](provider, work.CapabilityStateWriter)
	if err != nil {
		return StateExecutionReport{}, err
	}
	report := StateExecutionReport{Plan: plan, Events: []Event{}, Updated: []StateUpdate{}}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "authenticating", Project: stringPtr(plan.Project)}); err != nil {
		return StateExecutionReport{}, err
	}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "updating-work-item-state", IDs: append([]string(nil), plan.IDs...), State: plan.State}); err != nil {
		return StateExecutionReport{}, err
	}
	for _, id := range plan.IDs {
		results, err := writer.UpdateStates(ctx, projectRef(plan.Root, plan.Project), []work.StateChange{{ID: work.ItemID(id), State: work.State(plan.State), Comment: plan.History}})
		if err != nil {
			return StateExecutionReport{}, err
		}
		if len(results) == 0 {
			return StateExecutionReport{}, problem(msgProviderStateResultMissing, "provider returned no state result for work item %s", l10n.A("id", id))
		}
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "updated-work-item-state", ID: id, State: plan.State}); err != nil {
			return StateExecutionReport{}, err
		}
		report.Updated = append(report.Updated, StateUpdate{ID: id, State: plan.State})
	}
	return report, nil
}

func (s *Service) Context(ctx context.Context, request ContextRequest, sink EventSink) (ContextReport, error) {
	if request.Project == "" {
		return ContextReport{}, projectRequired("ado context")
	}
	if len(request.IDs) == 0 {
		return ContextReport{}, ErrWorkItemsRequired
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return ContextReport{}, err
	}
	report := ContextReport{Root: request.Root, Project: request.Project, RequestedIDs: append([]string(nil), request.IDs...), Summary: request.Summary, Comments: request.Comments, IncludeComments: request.IncludeComments, Expanded: []json.RawMessage{}, Items: []RichContextItem{}, Events: []Event{}}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "authenticating", Project: stringPtr(request.Project)}); err != nil {
		return ContextReport{}, err
	}
	if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "loading-work-items", IDs: append([]string(nil), request.IDs...)}); err != nil {
		return ContextReport{}, err
	}
	reference := projectRef(request.Root, request.Project)
	if request.Organization != "" {
		reference.Key = ""
		reference.Organization = request.Organization
		reference.Project = request.Project
	}
	if request.Mode == ContextRaw {
		reader, err := work.Require[work.RawItemReader](provider, work.CapabilityRawItemReader)
		if err != nil {
			return ContextReport{}, err
		}
		for _, id := range request.IDs {
			value, err := reader.ReadRawItem(ctx, reference, work.ItemID(id))
			if err != nil {
				return ContextReport{}, err
			}
			encoded, err := wirejson.Compact(value)
			if err != nil {
				return ContextReport{}, err
			}
			report.Expanded = append(report.Expanded, json.RawMessage(encoded))
		}
		return report, nil
	}
	reader, err := work.Require[work.RichContextReader](provider, work.CapabilityRichContextReader)
	if err != nil {
		return ContextReport{}, err
	}
	limit := request.Comments
	if !request.IncludeComments {
		limit = 0
	}
	items, err := reader.ReadRichContext(ctx, reference, itemIDs(request.IDs), work.ReadOptions{IncludeRelations: true, IncludeComments: request.IncludeComments, CommentLimit: limit})
	if err != nil {
		return ContextReport{}, err
	}
	for _, item := range items {
		report.Items = append(report.Items, projectRichContext(item, request.IncludeComments))
	}
	return report, nil
}

func (s *Service) DoingPlan(ctx context.Context, request DoingRequest) (DoingPlanReport, error) {
	if len(request.IDs) == 0 {
		return DoingPlanReport{}, ErrWorkItemsRequired
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return DoingPlanReport{}, err
	}
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return DoingPlanReport{}, err
	}
	items, err := reader.ReadItems(ctx, projectRef(request.Root, request.Project), itemIDs(request.IDs), work.ReadOptions{})
	if err != nil {
		return DoingPlanReport{}, err
	}
	byID := make(map[string]work.Item, len(items))
	for _, item := range items {
		byID[string(item.ID)] = item
	}
	updates := make([]DoingPlanUpdate, 0, len(request.IDs))
	for _, id := range request.IDs {
		item, ok := byID[id]
		if !ok {
			return DoingPlanReport{}, itemNotFound(id)
		}
		if item.Type == "" {
			return DoingPlanReport{}, problem(msgItemTypeMissing, "work item #%s has no type", l10n.A("id", id))
		}
		target, ok := stateForType(request.States, string(item.Type))
		if !ok {
			return DoingPlanReport{}, problem(msgItemTypeUnsupported, "work item #%s has unsupported type `%s` for `work item doing`", l10n.A("id", id), l10n.A("type", item.Type))
		}
		current := optionalString(string(item.State))
		updates = append(updates, DoingPlanUpdate{ID: id, Type: string(item.Type), CurrentState: current, TargetState: target, Changed: !strings.EqualFold(string(item.State), target)})
	}
	return DoingPlanReport{Provider: request.Provider, Root: request.Root, Project: request.Project, Updates: updates}, nil
}
func (s *Service) DoingExecute(ctx context.Context, plan DoingPlanReport, sink EventSink) (DoingExecutionReport, error) {
	provider, err := s.provider(plan.Provider)
	if err != nil {
		return DoingExecutionReport{}, err
	}
	writer, err := work.Require[work.StateWriter](provider, work.CapabilityStateWriter)
	if err != nil {
		return DoingExecutionReport{}, err
	}
	report := DoingExecutionReport{Plan: plan, Events: []Event{}, Updated: []DoingUpdate{}}
	for _, update := range plan.Updates {
		if !update.Changed {
			continue
		}
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "updating-work-item-state", IDs: []string{update.ID}, State: update.TargetState}); err != nil {
			return DoingExecutionReport{}, err
		}
		_, err := writer.UpdateStates(ctx, projectRef(plan.Root, plan.Project), []work.StateChange{{ID: work.ItemID(update.ID), State: work.State(update.TargetState), Comment: "DevWorkflow: passage en cours"}})
		if err != nil {
			return DoingExecutionReport{}, err
		}
		if err := collectEvent(ctx, &report.Events, sink, Event{Kind: "updated-work-item-state", ID: update.ID, State: update.TargetState}); err != nil {
			return DoingExecutionReport{}, err
		}
		report.Updated = append(report.Updated, DoingUpdate{ID: update.ID, State: update.TargetState})
	}
	return report, nil
}

func (s *Service) groupItems(ctx context.Context, provider work.Provider, root, project string, items []work.Item) ([]ItemGroup, error) {
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return nil, err
	}
	parentIDs := make([]string, 0)
	for _, item := range items {
		if parent, ok := item.ParentID.Get(); ok {
			parentIDs = appendDistinct(parentIDs, string(parent))
		}
	}
	parents, err := reader.ReadItems(ctx, projectRef(root, project), itemIDs(parentIDs), work.ReadOptions{})
	if err != nil {
		return nil, err
	}
	byID := make(map[string]work.Item, len(parents))
	for _, parent := range parents {
		byID[string(parent.ID)] = parent
	}
	groups := make([]ItemGroup, 0, len(parents))
	for _, parentID := range parentIDs {
		parent, ok := byID[parentID]
		if !ok {
			continue
		}
		group := ItemGroup{Parent: itemToSnapshot(parent), Items: []ItemSnapshot{}}
		for _, item := range items {
			if id, set := item.ParentID.Get(); set && string(id) == parentID {
				group.Items = append(group.Items, itemToSnapshot(item))
			}
		}
		groups = append(groups, group)
	}
	return groups, nil
}

func itemToSnapshot(item work.Item) ItemSnapshot {
	return ItemSnapshot{ID: string(item.ID), Type: optionalString(string(item.Type)), State: optionalString(string(item.State)), Title: optionalString(item.Title), URL: optionalString(item.URL)}
}
func itemsToSnapshots(items []work.Item) []ItemSnapshot {
	result := make([]ItemSnapshot, 0, len(items))
	for _, item := range items {
		result = append(result, itemToSnapshot(item))
	}
	return result
}
func projectPullRequests(items []work.PullRequest) ([]PullRequestItem, error) {
	result := make([]PullRequestItem, 0, len(items))
	for _, item := range items {
		pullRequestID, err := parsePullRequestID(item.ID)
		if err != nil {
			return nil, err
		}
		ids := make([]string, len(item.WorkItemIDs))
		for i, id := range item.WorkItemIDs {
			ids[i] = string(id)
		}
		result = append(result, PullRequestItem{Repository: string(item.Repository), PullRequestID: pullRequestID, Title: optionalString(item.Title), Status: optionalString(item.Status), SourceRefName: optionalString(item.SourceRef), TargetRefName: optionalString(item.TargetRef), IsDraft: item.Draft, CreatedBy: optionalString(item.CreatedBy), URL: optionalString(item.URL), WebURL: optionalString(item.WebURL), WorkItemIDs: ids})
	}
	return result, nil
}
func projectRichContext(value work.RichContext, includeComments bool) RichContextItem {
	productContext := make(map[string]string, len(value.ProductContext))
	for key, field := range value.ProductContext {
		productContext[key] = field
	}
	item := RichContextItem{
		SchemaVersion: RichContextSchemaVersion,
		WorkItem:      RichContextWorkItem{ID: string(value.Item.ID), URL: optionalString(value.Item.URL), Title: optionalString(value.Item.Title), Type: optionalString(string(value.Item.Type)), State: optionalString(string(value.Item.State)), AssignedTo: optionalString(value.Item.AssignedTo), AreaPath: optionalString(value.Item.AreaPath), IterationPath: optionalString(value.Item.IterationPath), Tags: append([]string(nil), value.Item.Tags...)},
		Core:          RichContextCore{CreatedBy: optionalString(value.CreatedBy), CreatedDate: optionalString(value.CreatedDate.String()), ChangedBy: optionalString(value.ChangedBy), ChangedDate: optionalString(value.ChangedDate.String()), Priority: optionalString(value.Priority), ValueArea: optionalString(value.ValueArea)},
		Content:       RichContextContent{Description: optionalString(value.Description), AcceptanceCriteria: optionalString(value.AcceptanceCriteria), ProductContext: productContext},
		Attachments:   RichContextAttachments{DirectoryHint: AttachmentDirectoryPrefix + string(value.Item.ID) + "/", Items: []RichContextAttachment{}},
		Relations:     []RichContextRelation{}, Comments: []RichContextComment{},
	}
	for _, relation := range value.Relations {
		target, _ := relation.TargetID.Get()
		item.Relations = append(item.Relations, RichContextRelation{Kind: string(relation.Kind), Rel: optionalString(string(relation.Kind)), WorkItemID: optionalString(string(target)), Name: optionalString(relation.Name), URL: optionalString(relation.URL), Comment: optionalString(relation.Comment), Artifact: optionalString(relation.Artifact)})
		if id, ok := relation.TargetID.Get(); ok {
			switch relation.Kind {
			case work.RelationParent:
				item.Links.ParentIDs = appendDistinct(item.Links.ParentIDs, string(id))
			case work.RelationChild:
				item.Links.ChildIDs = appendDistinct(item.Links.ChildIDs, string(id))
			case work.RelationPredecessor:
				item.Links.PredecessorIDs = appendDistinct(item.Links.PredecessorIDs, string(id))
			case work.RelationSuccessor:
				item.Links.SuccessorIDs = appendDistinct(item.Links.SuccessorIDs, string(id))
			}
		}
	}
	for _, attachment := range value.Attachments {
		item.Attachments.Items = append(item.Attachments.Items, RichContextAttachment{Name: optionalString(attachment.Name), URL: optionalString(attachment.URL), Comment: optionalString(attachment.Comment), DirectoryHint: item.Attachments.DirectoryHint})
	}
	if includeComments {
		for _, comment := range value.Comments {
			created := comment.CreatedAt.String()
			item.Comments = append(item.Comments, RichContextComment{Author: optionalString(comment.Author), CreatedDate: optionalString(created), Text: optionalString(comment.Text)})
		}
	}
	return item
}
func itemIDs(values []string) []work.ItemID {
	result := make([]work.ItemID, len(values))
	for i, value := range values {
		result[i] = work.ItemID(value)
	}
	return result
}
func parsePullRequestID(id work.PullRequestID) (int64, error) {
	value := strings.TrimSpace(string(id))
	parsed, err := strconv.ParseInt(value, 10, 64)
	if err != nil || parsed <= 0 {
		if err == nil {
			err = fmt.Errorf("must be positive")
		}
		return 0, invalidProviderPullRequestID(value, err)
	}
	return parsed, nil
}
func formatPullRequestID(id int64) (work.PullRequestID, error) {
	if id <= 0 {
		return "", invalidPullRequestID(id)
	}
	return work.PullRequestID(strconv.FormatInt(id, 10)), nil
}
func optionalString(value string) *string {
	if value == "" {
		return nil
	}
	copy := value
	return &copy
}
func stringPtr(value string) *string { return &value }
func distinctNonEmpty(values []string) []string {
	result := make([]string, 0, len(values))
	for _, value := range values {
		if value != "" {
			result = appendDistinct(result, value)
		}
	}
	return result
}
func appendDistinct(values []string, value string) []string {
	for _, existing := range values {
		if existing == value {
			return values
		}
	}
	return append(values, value)
}
func stateForType(states map[string]string, itemType string) (string, bool) {
	if state, ok := states[itemType]; ok && strings.TrimSpace(state) != "" {
		return state, true
	}
	for kind, state := range states {
		if strings.EqualFold(kind, itemType) && strings.TrimSpace(state) != "" {
			return state, true
		}
	}
	return "", false
}
