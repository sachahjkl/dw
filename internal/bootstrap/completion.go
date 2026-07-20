package bootstrap

import (
	"context"

	"github.com/sachahjkl/dw/internal/cli/complete"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/workspace"
)

type completionResolver struct {
	workspace *workspace.Engine
}

func (resolver completionResolver) ResolveCompletion(request complete.Context) ([]complete.Candidate, error) {
	root := config.ResolveRoot(request.Root)
	var values []string
	var err error

	switch request.Kind {
	case spec.CompleteProject:
		values = config.ProjectValues(root)
	case spec.CompleteRepository:
		values, err = resolver.workspace.RepositoryValues(context.Background(), root, request.Project, request.Workspace)
	case spec.CompleteWorkspace:
		values = workspace.WorkspaceValues(root, request.Project, request.WorkItem)
	case spec.CompleteWorkItem:
		values = workspace.WorkItemValues(root, request.Project)
	case spec.CompleteADOState:
		values, err = completionStates(root)
	case spec.CompleteDatabase:
		values = config.DatabaseValues(root, request.Project)
	case spec.CompleteEnvironment:
		values = config.EnvironmentValues(root, request.Project)
	case spec.CompleteEnvVariable:
		return complete.EnvironmentResolver{}.ResolveCompletion(request)
	case spec.CompleteSecret:
		values = config.SecretKeyValues(root)
	default:
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	result := make([]complete.Candidate, len(values))
	for index, value := range values {
		result[index] = complete.Candidate{Label: value}
	}
	return result, nil
}

func completionStates(root string) ([]string, error) {
	workflow := config.LoadWorkflowConfig(root)
	values := make([]string, 0, 9)
	appendState := func(value string) {
		if value == "" {
			return
		}
		for _, existing := range values {
			if existing == value {
				return
			}
		}
		values = append(values, value)
	}
	appendState("En réalisation")
	appendState("En développement")
	appendState("PR en attente")
	if workflow.TaskStart != nil {
		for _, state := range []*string{
			workflow.TaskStart.UserStoryState,
			workflow.TaskStart.AnomalyState,
			workflow.TaskStart.BugState,
			workflow.TaskStart.TaskState,
		} {
			if state != nil {
				appendState(*state)
			}
		}
	}
	if workflow.TaskFinish != nil {
		for _, state := range []*string{workflow.TaskFinish.BugState, workflow.TaskFinish.TaskState} {
			if state != nil {
				appendState(*state)
			}
		}
	}
	return values, nil
}
