package workapp

import (
	"context"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

func (s *Service) Start(ctx context.Context, request StartRequest, sink EventSink) (StartPlanReport, *StartExecutionReport, error) {
	if len(request.WorkItemIDs) == 0 {
		return StartPlanReport{}, nil, problem(msgStartItemRequired, "work-item-id is required to build a task start plan")
	}
	if s.Starter == nil {
		return StartPlanReport{}, nil, capabilityUnavailable("workspace start")
	}
	project := request.Project
	if project == "" {
		project = "default"
	}
	events := make([]Event, 0)
	if err := collectEvent(ctx, &events, sink, Event{Kind: "planning-start", Project: stringPtr(project), IDs: append([]string(nil), request.WorkItemIDs...)}); err != nil {
		return StartPlanReport{}, nil, err
	}
	var provider work.Provider
	var normalized []work.Item
	var err error
	needsRead := !request.SkipWork && (request.WithActiveChildren || request.Execute || strings.TrimSpace(request.Slug) == "")
	if !request.SkipWork {
		provider, err = s.provider(request.Provider)
		if err != nil {
			return StartPlanReport{}, nil, err
		}
	}
	if needsRead {
		if err := collectEvent(ctx, &events, sink, Event{Kind: "loading-start-work-items", Project: stringPtr(project), IDs: append([]string(nil), request.WorkItemIDs...)}); err != nil {
			return StartPlanReport{}, nil, err
		}
		normalized, err = s.loadStartItems(ctx, provider, request.Root, project, request.WorkItemIDs, request.WithActiveChildren)
		if err != nil {
			return StartPlanReport{}, nil, err
		}
	}
	plannedIDs := append([]string(nil), request.WorkItemIDs...)
	if request.WithActiveChildren && len(normalized) > 0 {
		plannedIDs = plannedIDs[:0]
		for _, item := range normalized {
			plannedIDs = append(plannedIDs, string(item.ID))
		}
	}
	slug := request.Slug
	if strings.TrimSpace(slug) == "" && len(normalized) > 0 {
		slug = normalized[0].Title
	}
	if err := collectEvent(ctx, &events, sink, Event{Kind: "building-start-plan", Project: stringPtr(project), Repositories: append([]string(nil), request.Repositories...)}); err != nil {
		return StartPlanReport{}, nil, err
	}
	plan, err := s.Starter.PlanStart(ctx, workspace.StartRequest{Root: request.Root, WorkItemIDs: plannedIDs, Project: project, TaskID: request.TaskID, Type: request.Type, Repositories: append([]string(nil), request.Repositories...), Slug: slug})
	if err != nil {
		return StartPlanReport{}, nil, err
	}
	report := StartPlanReport{Root: request.Root, Plan: plan, WorkItems: workItemsToWorkspace(normalized), ChildTasks: []workspace.ChildTask{}, Provider: request.Provider}
	if !request.Execute {
		return report, nil, nil
	}
	if err := collectEvent(ctx, &events, sink, Event{Kind: "executing-start", Project: stringPtr(project), Repositories: append([]string(nil), plan.Repositories...)}); err != nil {
		return StartPlanReport{}, nil, err
	}
	items := normalized
	if len(items) == 0 && !request.SkipWork {
		reader, requireErr := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
		if requireErr != nil {
			return StartPlanReport{}, nil, requireErr
		}
		items, err = reader.ReadItems(ctx, projectRef(request.Root, project), itemIDs(plan.WorkItemIDs), work.ReadOptions{})
		if err != nil {
			return StartPlanReport{}, nil, err
		}
	}
	children := make([]workspace.ChildTask, 0)
	if !request.SkipWork && request.CreateChildTasks {
		creator, requireErr := work.Require[work.ChildCreator](provider, work.CapabilityChildCreator)
		if requireErr != nil {
			return StartPlanReport{}, nil, requireErr
		}
		if len(items) > 0 {
			for _, repository := range plan.Repositories {
				title := workspace.ChildTaskTitle(repository, firstNonEmpty(items[0].Title, string(items[0].ID)))
				created, createErr := creator.CreateChild(ctx, projectRef(request.Root, project), work.ChildCreate{ParentID: items[0].ID, Type: work.ItemType("Task"), Title: title, History: "work start"})
				if createErr != nil {
					return StartPlanReport{}, nil, createErr
				}
				childTitle := created.Title
				children = append(children, workspace.ChildTask{Repository: repository, ID: string(created.ID), Title: optionalString(childTitle)})
			}
			plan = workspace.StartPlanWithChildTasks(plan, children)
		}
	}
	stateUpdates := make([]StartStateUpdate, 0)
	if !request.SkipWork && len(request.States) > 0 {
		writer, requireErr := work.Require[work.StateWriter](provider, work.CapabilityStateWriter)
		if requireErr != nil {
			return StartPlanReport{}, nil, requireErr
		}
		for _, item := range items {
			target, ok := stateForType(request.States, string(item.Type))
			if !ok {
				continue
			}
			changed := !strings.EqualFold(string(item.State), target)
			if changed {
				_, updateErr := writer.UpdateStates(ctx, projectRef(request.Root, project), []work.StateChange{{ID: item.ID, State: work.State(target), Comment: "work start"}})
				if updateErr != nil {
					return StartPlanReport{}, nil, updateErr
				}
			}
			stateUpdates = append(stateUpdates, StartStateUpdate{ID: string(item.ID), Label: workItemLabel(item), TargetState: target, Changed: changed})
		}
	}
	local, err := s.Starter.ExecuteStart(ctx, plan, workItemsToWorkspace(items), children, nil)
	if err != nil {
		return StartPlanReport{}, nil, err
	}
	execution := StartExecutionReport{Plan: local.Plan, Manifest: local.Manifest, WorkItems: local.WorkItems, ChildTasks: local.ChildTasks, StateUpdates: stateUpdates, Events: events}
	return report, &execution, nil
}

func (s *Service) StartPullRequest(ctx context.Context, request StartPullRequestRequest, sink EventSink) (StartPullRequestPlanReport, *StartExecutionReport, error) {
	if request.Project == "" {
		return StartPullRequestPlanReport{}, nil, projectRequired("work pr start")
	}
	if len(request.Repositories) == 0 {
		return StartPullRequestPlanReport{}, nil, repositoriesRequired("work pr start", "requires an explicit repository, or a project with configured work repository entries")
	}
	providerRepositories := request.ProviderRepositories
	if len(providerRepositories) == 0 {
		providerRepositories = request.Repositories
	}
	providerID, idErr := formatPullRequestID(request.PullRequestID)
	if idErr != nil {
		return StartPullRequestPlanReport{}, nil, idErr
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return StartPullRequestPlanReport{}, nil, err
	}
	reader, err := work.Require[work.PullRequestReader](provider, work.CapabilityPullRequestReader)
	if err != nil {
		return StartPullRequestPlanReport{}, nil, err
	}
	events := make([]Event, 0)
	if err := collectEvent(ctx, &events, sink, Event{Kind: "resolving-pull-request-work-items", Repositories: append([]string(nil), providerRepositories...)}); err != nil {
		return StartPullRequestPlanReport{}, nil, err
	}
	ids := make([]string, 0)
	for _, repository := range providerRepositories {
		resolved, readErr := reader.PullRequestWorkItemIDs(ctx, projectRef(request.Root, request.Project), work.RepositoryName(repository), providerID)
		if readErr != nil {
			return StartPullRequestPlanReport{}, nil, readErr
		}
		for _, id := range resolved {
			ids = appendDistinct(ids, string(id))
		}
	}
	if len(ids) == 0 {
		return StartPullRequestPlanReport{}, nil, prItemsNotFound(request.PullRequestID, strings.Join(providerRepositories, ", "))
	}
	start, execution, err := s.Start(ctx, StartRequest{Provider: request.Provider, Root: request.Root, Project: request.Project, WorkItemIDs: ids, Type: request.Type, Repositories: request.Repositories, Slug: request.Slug, Execute: request.Execute, States: request.States}, sink)
	if err != nil {
		return StartPullRequestPlanReport{}, nil, err
	}
	return StartPullRequestPlanReport{PullRequestID: request.PullRequestID, Repositories: append([]string(nil), request.Repositories...), ProviderRepositories: append([]string(nil), providerRepositories...), WorkItemIDs: ids, Start: start}, execution, nil
}

func (s *Service) Open(ctx context.Context, request OpenRequest, sink EventSink) (OpenReport, error) {
	if s.Lookup == nil || (!request.ResolveOnly && s.Opener == nil) {
		return OpenReport{}, capabilityUnavailable("workspace open")
	}
	events := make([]Event, 0)
	ids := append([]string(nil), request.WorkItemIDs...)
	if request.PullRequestID != nil {
		if request.Project == "" {
			return OpenReport{}, problem(msgOpenPRProject, "work open --pr requires --project to resolve work provider settings")
		}
		if request.Repository == "" {
			return OpenReport{}, problem(msgOpenPRRepository, "work open --pr requires --repo, or a project with configured work repositories")
		}
		providerID, idErr := formatPullRequestID(*request.PullRequestID)
		if idErr != nil {
			return OpenReport{}, idErr
		}
		provider, err := s.provider(request.Provider)
		if err != nil {
			return OpenReport{}, err
		}
		reader, err := work.Require[work.PullRequestReader](provider, work.CapabilityPullRequestReader)
		if err != nil {
			return OpenReport{}, err
		}
		if err := collectEvent(ctx, &events, sink, Event{Kind: "resolving-pull-request-work-items", Repositories: []string{request.Repository}}); err != nil {
			return OpenReport{}, err
		}
		resolved, err := reader.PullRequestWorkItemIDs(ctx, projectRef(request.Root, request.Project), work.RepositoryName(request.Repository), providerID)
		if err != nil {
			return OpenReport{}, err
		}
		ids = ids[:0]
		for _, id := range resolved {
			ids = append(ids, string(id))
		}
		if len(ids) == 0 {
			return OpenReport{}, prItemsNotFound(*request.PullRequestID, request.Repository)
		}
	}
	workspacePath, err := s.Lookup.Resolve(ctx, request.Root, request.Workspace, request.Project, ids, request.Continue)
	if err != nil {
		return OpenReport{}, err
	}
	if request.ResolveOnly {
		return OpenReport{Workspace: workspacePath, Events: events}, nil
	}
	launch, err := s.Opener.Open(ctx, workspacePath, request.Repository, request.Agent, request.Continue)
	if err != nil {
		return OpenReport{}, err
	}
	return OpenReport{Workspace: workspacePath, Launch: launch, Events: events}, nil
}

func (s *Service) Sync(ctx context.Context, request SyncRequest, sink EventSink) (SyncReport, error) {
	if s.Lookup == nil || s.Syncer == nil {
		return SyncReport{}, capabilityUnavailable("workspace sync")
	}
	workspacePath, err := s.Lookup.Resolve(ctx, request.Root, request.Workspace, request.Project, request.WorkItemIDs, request.Continue)
	if err != nil {
		return SyncReport{}, err
	}
	manifest, err := s.Lookup.Manifest(ctx, workspacePath)
	if err != nil {
		return SyncReport{}, err
	}
	ids := parentWorkItemIDs(manifest)
	provider, err := s.provider(request.Provider)
	if err != nil {
		return SyncReport{}, err
	}
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return SyncReport{}, err
	}
	events := []Event{}
	if err := collectEvent(ctx, &events, sink, Event{Kind: "loading-work-items", IDs: append([]string(nil), ids...)}); err != nil {
		return SyncReport{}, err
	}
	items, err := reader.ReadItems(ctx, projectRef(request.Root, manifest.Project), itemIDs(ids), work.ReadOptions{})
	if err != nil {
		return SyncReport{}, err
	}
	snapshots := workItemsToWorkspace(items)
	updated, err := s.Syncer.ApplySnapshots(ctx, workspacePath, snapshots)
	if err != nil {
		return SyncReport{}, err
	}
	return SyncReport{Workspace: workspacePath, RequestedIDs: ids, Snapshots: snapshots, Manifest: updated, Events: events}, nil
}

func (s *Service) CreateChild(ctx context.Context, request ChildRequest, sink EventSink) (ChildReport, error) {
	if s.Lookup == nil || s.Children == nil {
		return ChildReport{}, capabilityUnavailable("workspace child")
	}
	workspacePath, err := s.Lookup.Resolve(ctx, request.Root, request.Workspace, request.Project, request.WorkItemIDs, request.Continue)
	if err != nil {
		return ChildReport{}, err
	}
	manifest, err := s.Lookup.Manifest(ctx, workspacePath)
	if err != nil {
		return ChildReport{}, err
	}
	parents := manifest.ParentWorkItems()
	if len(parents) == 0 {
		return ChildReport{}, problem(msgWorkspaceParentMissing, "workspace has no parent work item")
	}
	parent := parents[0]
	if !workspace.RequiresChildTasks(parent.Type) {
		return ChildReport{}, problem(msgChildUnsupported, "this command is only available for User Story and Anomalie")
	}
	provider, err := s.provider(request.Provider)
	if err != nil {
		return ChildReport{}, err
	}
	creator, err := work.Require[work.ChildCreator](provider, work.CapabilityChildCreator)
	if err != nil {
		return ChildReport{}, err
	}
	title := workspace.ChildTaskTitle(request.Repository, request.Title)
	created, err := creator.CreateChild(ctx, projectRef(request.Root, manifest.Project), work.ChildCreate{ParentID: work.ItemID(parent.ID), Type: work.ItemType("Task"), Title: title, History: "work task child create"})
	if err != nil {
		return ChildReport{}, err
	}
	childTitle := created.Title
	child := workspace.ChildTask{Repository: request.Repository, ID: string(created.ID), Title: optionalString(childTitle)}
	updated, err := s.Children.AddChild(ctx, workspacePath, child)
	if err != nil {
		return ChildReport{}, err
	}
	return ChildReport{Workspace: workspacePath, Repository: request.Repository, Parent: parent, RequestedTitle: title, Created: ChildCreateResult{Repository: request.Repository, ID: string(created.ID), Title: created.Title}, Manifest: updated, Events: []Event{}}, nil
}

func (s *Service) Prune(ctx context.Context, request PruneRequest, sink EventSink) (PruneReport, error) {
	if s.Pruner == nil {
		return PruneReport{}, capabilityUnavailable("workspace prune")
	}
	syncReports := make([]workspace.PruneSyncReport, 0)
	if !request.NoSync {
		found, err := s.Pruner.Find(ctx, request.Root, request.Project, request.WorkItemIDs)
		if err != nil {
			return PruneReport{}, err
		}
		for _, candidate := range found {
			manifest := candidate.Manifest
			ids := parentWorkItemIDs(manifest)
			result := workspace.PruneSyncReport{Workspace: candidate.Path, Status: "skipped"}
			provider, providerErr := s.provider(request.Provider)
			if providerErr != nil {
				result.Detail = workspace.PruneSyncDetail{Kind: "auth-unavailable", Error: providerErr.Error()}
				syncReports = append(syncReports, result)
				continue
			}
			reader, requireErr := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
			if requireErr != nil {
				result.Detail = workspace.PruneSyncDetail{Kind: "auth-unavailable", Error: requireErr.Error()}
				syncReports = append(syncReports, result)
				continue
			}
			items, readErr := reader.ReadItems(ctx, projectRef(request.Root, manifest.Project), itemIDs(ids), work.ReadOptions{})
			if readErr != nil {
				result.Detail = workspace.PruneSyncDetail{Kind: "sync-failed", Error: readErr.Error()}
				syncReports = append(syncReports, result)
				continue
			}
			updated, applyErr := s.Syncer.ApplySnapshots(ctx, candidate.Path, workItemsToWorkspace(items))
			if applyErr != nil {
				result.Detail = workspace.PruneSyncDetail{Kind: "sync-failed", Error: applyErr.Error()}
				syncReports = append(syncReports, result)
				continue
			}
			result.Status = "synced"
			result.Detail = workspace.PruneSyncDetail{Kind: "synced", WorkItems: updated.ParentWorkItems()}
			syncReports = append(syncReports, result)
		}
	}
	candidates, err := s.Pruner.PlanPrune(ctx, request.Root, request.Project, request.WorkItemIDs)
	if err != nil {
		return PruneReport{}, err
	}
	plan := workspace.PrunePlanReport{Root: request.Root, Project: request.Project, WorkItemIDs: append([]string(nil), request.WorkItemIDs...), Sync: syncReports, Candidates: candidates}
	report := PruneReport{Plan: plan, Events: []Event{}}
	if !request.Execute {
		return report, nil
	}
	selected := candidates
	if request.SelectedWorkspaces != nil {
		selected = selected[:0]
		for _, candidate := range candidates {
			if containsString(request.SelectedWorkspaces, candidate.Path) {
				selected = append(selected, candidate)
			}
		}
	}
	execution, err := s.Pruner.ExecutePrune(ctx, request.Root, selected)
	if err != nil {
		return PruneReport{}, err
	}
	report.Execution = &execution
	return report, nil
}

func (s *Service) loadStartItems(ctx context.Context, provider work.Provider, root, project string, selected []string, withChildren bool) ([]work.Item, error) {
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return nil, err
	}
	items, err := reader.ReadItems(ctx, projectRef(root, project), itemIDs(selected), work.ReadOptions{})
	if err != nil {
		return nil, err
	}
	if len(items) == 0 {
		for _, id := range selected {
			items = append(items, work.Item{ID: work.ItemID(id)})
		}
		return items, nil
	}
	if !withChildren {
		return items, nil
	}
	relations, err := work.Require[work.RelationReader](provider, work.CapabilityRelationReader)
	if err != nil {
		return nil, err
	}
	classifier, err := work.Require[work.StateClassifier](provider, work.CapabilityStateClassifier)
	if err != nil {
		return nil, err
	}
	loadedRelations, err := relations.ReadRelations(ctx, projectRef(root, project), itemIDs(selected))
	if err != nil {
		return nil, err
	}
	childIDs := make([]string, 0)
	for _, relation := range loadedRelations {
		if relation.Kind != work.RelationChild {
			continue
		}
		if target, ok := relation.TargetID.Get(); ok && !containsWorkItem(items, string(target)) {
			childIDs = appendDistinct(childIDs, string(target))
		}
	}
	children, err := reader.ReadItems(ctx, projectRef(root, project), itemIDs(childIDs), work.ReadOptions{})
	if err != nil {
		return nil, err
	}
	for _, child := range children {
		if classifier.IsFinalState(child.Type, child.State) || containsWorkItem(items, string(child.ID)) {
			continue
		}
		items = append(items, child)
	}
	return items, nil
}

func workItemsToWorkspace(items []work.Item) []workspace.WorkItem {
	result := make([]workspace.WorkItem, 0, len(items))
	for _, item := range items {
		result = append(result, workspace.WorkItem{ID: string(item.ID), Type: optionalString(string(item.Type)), Title: optionalString(item.Title), State: optionalString(string(item.State)), URL: optionalString(item.URL)})
	}
	return result
}
func parentWorkItemIDs(manifest workspace.Manifest) []string {
	parents := manifest.ParentWorkItems()
	result := make([]string, 0, len(parents))
	for _, item := range parents {
		result = append(result, item.ID)
	}
	return result
}
func containsWorkItem(items []work.Item, id string) bool {
	for _, item := range items {
		if string(item.ID) == id {
			return true
		}
	}
	return false
}
func firstNonEmpty(value, fallback string) string {
	if strings.TrimSpace(value) != "" {
		return value
	}
	return fallback
}
func workItemLabel(item work.Item) string {
	label := "#" + string(item.ID)
	if item.Type != "" {
		label += " [" + string(item.Type) + "]"
	}
	if item.Title != "" {
		label += " " + item.Title
	}
	return label
}
