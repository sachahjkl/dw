package doctor

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/contract"
)

const (
	ActionDoctor      action.ID = "doctor"
	ActionAgentDoctor action.ID = "agent.doctor"
)

type Request struct {
	Fix bool
}

type AgentRequest struct {
	Agent *contract.Agent
}

func (Request) ActionID() action.ID      { return ActionDoctor }
func (AgentRequest) ActionID() action.ID { return ActionAgentDoctor }
func (Report) ActionID() action.ID       { return ActionDoctor }
func (AgentReport) ActionID() action.ID  { return ActionAgentDoctor }

type Handler struct {
	action  action.ID
	service *Service
}

func Handlers(service *Service) []action.Handler {
	return []action.Handler{
		Handler{action: ActionDoctor, service: service},
		Handler{action: ActionAgentDoctor, service: service},
	}
}

func (handler Handler) ID() action.ID { return handler.action }

func (handler Handler) Execute(ctx context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	if handler.service == nil {
		return nil, errorsForNilService(handler.action)
	}
	switch handler.action {
	case ActionDoctor:
		typed, err := doctorRequest(request)
		if err != nil {
			return nil, err
		}
		return handler.service.Run(ctx, typed.Fix)
	case ActionAgentDoctor:
		typed, err := agentRequest(request)
		if err != nil {
			return nil, err
		}
		return RunAgents(ctx, handler.service.process, typed.Agent), nil
	default:
		return nil, fmt.Errorf("doctor.unknown-action:%s", handler.action)
	}
}

func doctorRequest(request action.Request) (Request, error) {
	switch value := request.(type) {
	case Request:
		return value, nil
	case *Request:
		if value != nil {
			return *value, nil
		}
	}
	return Request{}, fmt.Errorf("doctor.invalid-request:%T", request)
}

func agentRequest(request action.Request) (AgentRequest, error) {
	switch value := request.(type) {
	case AgentRequest:
		return value, nil
	case *AgentRequest:
		if value != nil {
			return *value, nil
		}
	}
	return AgentRequest{}, fmt.Errorf("doctor.invalid-agent-request:%T", request)
}

func errorsForNilService(id action.ID) error {
	return fmt.Errorf("doctor.nil-service:%s", id)
}
