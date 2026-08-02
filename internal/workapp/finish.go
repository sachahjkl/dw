package workapp

import (
	"context"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

func (s *Service) Finish(ctx context.Context, request FinishRequest, sink EventSink) (FinishReport, error) {
	if request.CreatePR && request.SkipWork {
		return FinishReport{}, problem(msgFinishPRProviderless, "PR creation cannot be combined with provider-less mode")
	}
	if s.Lookup == nil || s.Finisher == nil {
		return FinishReport{}, capabilityUnavailable("workspace finish")
	}
	workspacePath, err := s.Lookup.Resolve(ctx, request.Root, request.Workspace, "", nil, request.Continue)
	if err != nil {
		return FinishReport{}, err
	}
	message := ""
	if request.Message != nil {
		message = *request.Message
	}
	plan, err := s.Finisher.PlanFinish(ctx, request.Root, workspacePath, message, request.CreatePR, request.Ready)
	if err != nil {
		return FinishReport{}, err
	}
	report := FinishReport{Plan: plan, Events: []Event{}}
	if !request.Execute {
		return report, nil
	}
	if !plan.Handoff.IsValid {
		return FinishReport{}, ErrInvalidHandoff
	}
	var provider work.Provider
	var prReader work.PullRequestReader
	var prWriter work.PullRequestWriter
	if request.CreatePR {
		provider, err = s.provider(s.providerName(request.Provider, request.Root, plan.Manifest.Project))
		if err != nil {
			return FinishReport{}, err
		}
		prReader, err = work.Require[work.PullRequestReader](provider, work.CapabilityPullRequestReader)
		if err != nil {
			return FinishReport{}, err
		}
		prWriter, err = work.Require[work.PullRequestWriter](provider, work.CapabilityPullRequestWriter)
		if err != nil {
			return FinishReport{}, err
		}
	}
	var sinkErr error
	var emit func(workspace.ActionEvent)
	if sink != nil {
		emit = func(event workspace.ActionEvent) {
			if request.CreatePR && event.Type == "skippingPullRequestCreation" {
				return
			}
			if sinkErr != nil {
				return
			}
			sinkErr = sink(ctx, Event{
				Kind:            event.Type,
				Repository:      event.Repository,
				Operation:       event.Operation,
				RepositoryCount: event.RepositoryCount,
				WorkItemID:      event.WorkItemID,
				Error:           event.Error,
			})
		}
	}
	local, err := s.Finisher.ExecuteLocalFinish(ctx, plan, workspace.FinishExecuteOptions{SkipVerification: request.SkipVerify, ForceWithLease: request.ForceWithLease}, emit)
	report.Execution = &local
	if err == nil {
		err = sinkErr
	}
	if err != nil {
		return report, err
	}
	if !request.CreatePR {
		local.Events = append(local.Events, workspace.ActionEvent{Type: "skippingPullRequestCreation"})
		report.Execution = &local
		return report, nil
	}
	local.Events = append(local.Events, workspace.ActionEvent{Type: "authenticatingWorkProviderForPullRequests", RepositoryCount: len(plan.PullRequestCandidates)})
	reference := projectRef(request.Root, plan.Manifest.Project)
	sourceRef := "refs/heads/" + plan.Manifest.BranchName
	for _, candidate := range plan.PullRequestCandidates {
		if strings.TrimSpace(candidate.ProviderRepository) == "" {
			local.PullRequests = append(local.PullRequests, workspace.PullRequestResult{Repository: candidate.Repository, Action: "skipped", SkipReason: "missingProviderRepository"})
			continue
		}
		local.Events = append(local.Events, workspace.ActionEvent{Type: "checkingActivePullRequest", Repository: candidate.Repository})
		existing, findErr := prReader.ActivePullRequest(ctx, reference, work.RepositoryName(candidate.ProviderRepository), sourceRef)
		if findErr != nil {
			return report, findErr
		}
		if existing != nil {
			projected, projectionErr := finishPullRequestResult(candidate.Repository, "existing", existing.ID, existing.URL, existing.WebURL)
			if projectionErr != nil {
				return report, projectionErr
			}
			local.PullRequests = append(local.PullRequests, projected)
			continue
		}
		local.Events = append(local.Events, workspace.ActionEvent{Type: "creatingPullRequest", Repository: candidate.Repository})
		handoff := handoffFor(plan.HandoffSummaries, candidate.Repository)
		created, createErr := prWriter.CreatePullRequest(ctx, reference, work.PullRequestCreate{Repository: work.RepositoryName(candidate.ProviderRepository), SourceRef: sourceRef, TargetRef: "refs/heads/" + candidate.TargetBranch, Title: finishPullRequestTitle(plan.Manifest), Description: workspace.PullRequestDescription(plan.Manifest, candidate, "", local.VerificationResults, handoff), Draft: !plan.Ready, WorkItemIDs: itemIDs(plan.Manifest.AllKnownWorkItemIDs())})
		if createErr != nil {
			return report, createErr
		}
		projected, projectionErr := finishPullRequestResult(candidate.Repository, "created", created.ID, created.URL, created.WebURL)
		if projectionErr != nil {
			return report, projectionErr
		}
		local.PullRequests = append(local.PullRequests, projected)
		for _, id := range plan.Manifest.AllKnownWorkItemIDs() {
			if linkErr := prWriter.LinkPullRequestWorkItem(ctx, reference, work.RepositoryName(candidate.ProviderRepository), created.ID, work.ItemID(id)); linkErr != nil {
				local.Events = append(local.Events, workspace.ActionEvent{Type: "pullRequestWorkItemLinkSkipped", WorkItemID: id, Error: linkErr.Error()})
			}
		}
	}
	if !request.SkipWork && len(request.FinishStates) > 0 {
		reader, requireErr := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
		if requireErr != nil {
			return report, requireErr
		}
		writer, requireErr := work.Require[work.StateWriter](provider, work.CapabilityStateWriter)
		if requireErr != nil {
			return report, requireErr
		}
		ids := plan.Manifest.AllKnownWorkItemIDs()
		local.Events = append(local.Events, workspace.ActionEvent{Type: "updatingFinishWorkItemStates"})
		for _, id := range ids {
			loaded, readErr := reader.ReadItems(ctx, projectRef(request.Root, plan.Manifest.Project), []work.ItemID{work.ItemID(id)}, work.ReadOptions{})
			if readErr != nil {
				return report, readErr
			}
			if len(loaded) == 0 {
				return report, itemNotFound(id)
			}
			item := loaded[0]
			target, ok := finishStateForType(request.FinishStates, string(item.Type))
			current := optionalString(string(item.State))
			kind := optionalString(string(item.Type))
			update := workspace.WorkItemStateUpdate{ID: string(item.ID), Label: workItemLabel(item), Type: kind, CurrentState: current}
			if !ok {
				update.Outcome = "unsupportedWorkItemType"
				local.WorkItemUpdates = append(local.WorkItemUpdates, update)
				continue
			}
			update.TargetState = optionalString(target)
			if strings.EqualFold(string(item.State), target) {
				update.Outcome = "alreadyInTargetState"
				local.WorkItemUpdates = append(local.WorkItemUpdates, update)
				continue
			}
			_, writeErr := writer.UpdateStates(ctx, projectRef(request.Root, plan.Manifest.Project), []work.StateChange{{ID: item.ID, State: work.State(target), Comment: "dw workspace finish: pull request opened"}})
			if writeErr != nil {
				return report, writeErr
			}
			update.Changed = true
			update.Outcome = "updated"
			local.WorkItemUpdates = append(local.WorkItemUpdates, update)
		}
	}
	return report, nil
}

func finishPullRequestResult(repository, actionName string, id work.PullRequestID, url, webURL string) (workspace.PullRequestResult, error) {
	parsed, err := parsePullRequestID(id)
	if err != nil {
		return workspace.PullRequestResult{}, err
	}
	if webURL != "" {
		url = webURL
	}
	return workspace.PullRequestResult{Repository: repository, Action: actionName, URL: optionalString(url), PullRequestID: &parsed}, nil
}

func handoffFor(values []workspace.HandoffSummary, repository string) workspace.HandoffSummary {
	for _, value := range values {
		if strings.EqualFold(value.Repository, repository) {
			return value
		}
	}
	return workspace.HandoffSummary{Repository: repository}
}
func finishPullRequestTitle(manifest workspace.Manifest) string {
	title := manifest.Slug
	if manifest.WorkItemTitle != nil && strings.TrimSpace(*manifest.WorkItemTitle) != "" {
		title = *manifest.WorkItemTitle
	}
	return "#" + manifest.WorkItemID + " - " + title
}
func finishStateForType(states map[string]string, itemType string) (string, bool) {
	normalized := normalizeWorkItemType(itemType)
	if normalized != "bug" && normalized != "activite" && normalized != "task" && normalized != "tache" {
		return "", false
	}
	return stateForType(states, itemType)
}
func normalizeWorkItemType(value string) string {
	value = strings.ToLower(strings.TrimSpace(value))
	replacer := strings.NewReplacer("é", "e", "è", "e", "ê", "e", "ë", "e", "â", "a", "à", "a", "ä", "a", "ô", "o", "ö", "o", "ù", "u", "û", "u", "ü", "u", "î", "i", "ï", "i", "ç", "c")
	return replacer.Replace(value)
}
