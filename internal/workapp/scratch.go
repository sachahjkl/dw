package workapp

import (
	"context"
	"fmt"
	"strings"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

func (s *Service) ScratchStart(ctx context.Context, request ScratchStartRequest, _ EventSink) (ScratchStartResult, error) {
	if s.Scratch == nil {
		return ScratchStartResult{}, capabilityUnavailable("workspace scratch start")
	}
	plan, err := s.Scratch.PlanScratchStart(ctx, workspace.ScratchStartRequest{Root: request.Root, Project: request.Project, Title: request.Title, Slug: request.Slug, Repositories: request.Repositories})
	result := ScratchStartResult{Plan: plan}
	if err != nil || !request.Execute {
		return result, err
	}
	execution, err := s.Scratch.ExecuteScratchStart(ctx, plan, nil)
	result.Execution = &execution
	return result, err
}

func (s *Service) ScratchPromote(ctx context.Context, request ScratchPromoteRequest, _ EventSink) (ScratchPromoteResult, error) {
	if s.Scratch == nil || s.Lookup == nil {
		return ScratchPromoteResult{}, capabilityUnavailable("workspace scratch promote")
	}
	if strings.TrimSpace(request.WorkItemID) == "" {
		return ScratchPromoteResult{}, fmt.Errorf("work item ID is required")
	}
	path, err := s.Lookup.Resolve(ctx, request.Root, request.Workspace, "", nil, false)
	if err != nil {
		return ScratchPromoteResult{}, err
	}
	manifest, err := s.Lookup.Manifest(ctx, path)
	if err != nil {
		return ScratchPromoteResult{}, err
	}
	if manifest.Kind != workspace.KindScratch {
		return ScratchPromoteResult{}, fmt.Errorf("workspace is not a scratch workspace: %s", path)
	}
	provider, err := s.provider(s.providerName(request.Provider, request.Root, manifest.Project))
	if err != nil {
		return ScratchPromoteResult{}, err
	}
	reader, err := work.Require[work.ItemReader](provider, work.CapabilityItemReader)
	if err != nil {
		return ScratchPromoteResult{}, err
	}
	items, err := reader.ReadItems(ctx, projectRef(request.Root, manifest.Project), []work.ItemID{work.ItemID(request.WorkItemID)}, work.ReadOptions{})
	if err != nil {
		return ScratchPromoteResult{}, err
	}
	if len(items) == 0 {
		return ScratchPromoteResult{}, fmt.Errorf("target work item was not found: %s", request.WorkItemID)
	}
	targets := workItemsToWorkspace(items)
	target := targets[0]
	kind := "feat"
	if target.Type != nil {
		switch strings.ToLower(strings.TrimSpace(*target.Type)) {
		case "bug", "anomalie":
			kind = "bugfix"
		case "task", "tache", "activite":
			kind = "chore"
		}
	}
	createChildren := request.CreateChildTasks || containsTrimmedFold(request.RequiredChildTaskTypes, valueOrEmpty(target.Type))
	stateNames := make([]string, 0)
	if state, ok := stateForType(request.States, valueOrEmpty(target.Type)); ok {
		stateNames = append(stateNames, state)
	}
	original, plan, err := s.Scratch.PlanScratchPromotion(ctx, path, target, kind, "", createChildren, stateNames)
	result := ScratchPromoteResult{Plan: plan}
	if err != nil || !request.Execute {
		return result, err
	}
	var creator work.ChildCreator
	if createChildren {
		creator, err = work.Require[work.ChildCreator](provider, work.CapabilityChildCreator)
		if err != nil {
			return result, err
		}
	}
	var writer work.StateWriter
	if len(stateNames) > 0 {
		writer, err = work.Require[work.StateWriter](provider, work.CapabilityStateWriter)
		if err != nil {
			return result, err
		}
	}
	execution, err := s.Scratch.ExecuteScratchPromotionLocal(ctx, original, plan)
	result.Execution = &execution
	if err != nil {
		return result, err
	}
	if creator != nil {
		for _, repository := range plan.Repositories {
			created, createErr := creator.CreateChild(ctx, projectRef(request.Root, manifest.Project), work.ChildCreate{ParentID: work.ItemID(target.ID), Type: work.ItemType("Task"), Title: workspace.ChildTaskTitle(repository, valueOrEmpty(target.Title)), History: "workspace scratch promote"})
			if created.ID != "" {
				execution.ProviderEffects = append(execution.ProviderEffects, "created child #"+string(created.ID))
				if s.Children != nil {
					updated, persistErr := s.Children.AddChild(ctx, plan.NewWorkspace, workspace.ChildTask{Repository: repository, ID: string(created.ID), Title: optionalString(created.Title)})
					if persistErr != nil {
						return result, persistErr
					}
					execution.Manifest = updated
				}
			}
			if createErr != nil {
				return result, createErr
			}
		}
	}
	if writer != nil {
		_, updateErr := writer.UpdateStates(ctx, projectRef(request.Root, manifest.Project), []work.StateChange{{ID: work.ItemID(target.ID), State: work.State(stateNames[0]), Comment: "workspace scratch promote"}})
		if updateErr != nil {
			return result, updateErr
		}
		execution.ProviderEffects = append(execution.ProviderEffects, "updated state of #"+target.ID)
	}
	contextReport, contextErr := s.refreshWorkspaceContext(ctx, request.Provider, request.Root, plan.NewWorkspace, nil)
	if contextErr != nil {
		return result, contextErr
	}
	execution.ContextFile = contextReport.ContextFile
	result.Execution = &execution
	return result, nil
}

func valueOrEmpty(value *string) string {
	if value == nil {
		return ""
	}
	return *value
}
