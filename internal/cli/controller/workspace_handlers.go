package controller

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/config"
)

const actionAgentContext = action.ID("agent.context")

type AgentContextRequest struct {
	Root string `json:"root,omitempty"`
}

type AgentContextResult struct{ agent.ContextReport }

func (AgentContextRequest) ActionID() action.ID { return actionAgentContext }
func (AgentContextResult) ActionID() action.ID  { return actionAgentContext }

// IntegrationHandlers contains controller-owned non-workspace adapters.
func IntegrationHandlers() []action.Handler {
	return []action.Handler{controllerHandler[AgentContextRequest](actionAgentContext, func(_ context.Context, request AgentContextRequest, _ action.Runtime) (action.Result, error) {
		return AgentContextResult{ContextReport: agent.Context(config.ResolveRoot(request.Root))}, nil
	})}
}

func controllerHandler[T action.Request](id action.ID, execute func(context.Context, T, action.Runtime) (action.Result, error)) action.Handler {
	return action.HandlerFunc{Action: id, ExecuteFunc: func(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
		typed, ok := request.(T)
		if !ok {
			return nil, fmt.Errorf("cli.invalid-action-request:%s:%T", id, request)
		}
		return execute(ctx, typed, runtime)
	}}
}

func stateSetRoute() Route {
	return Route{Key: "work.item.state.set", Machine: jsonMachine, Build: buildWorkItemStateSet, Project: jsonOptionProject}
}

func doingRoute() Route {
	return Route{Key: "work.item.doing", Machine: jsonMachine, Build: buildWorkItemDoing, Project: jsonOptionProject}
}
