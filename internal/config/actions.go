package config

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
)

const (
	ActionInit            action.ID = "init"
	ActionRefresh         action.ID = "refresh"
	ActionShow            action.ID = "config.show"
	ActionDoctor          action.ID = "config.doctor"
	ActionRootSet         action.ID = "config.root.set"
	ActionColorSet        action.ID = "config.color.set"
	ActionAgentConfig     action.ID = "agent.config"
	ActionAgentShow       action.ID = "agent.show"
	ActionAgentDefaultSet action.ID = "agent.default.set"
)

func (InitRequest) ActionID() action.ID        { return ActionInit }
func (InitReport) ActionID() action.ID         { return ActionInit }
func (RefreshRequest) ActionID() action.ID     { return ActionRefresh }
func (RefreshReport) ActionID() action.ID      { return ActionRefresh }
func (ConfigShow) ActionID() action.ID         { return ActionShow }
func (ConfigDoctorReport) ActionID() action.ID { return ActionDoctor }

type ShowRequest struct {
	Root string `json:"root,omitempty"`
}

func (ShowRequest) ActionID() action.ID { return ActionShow }

type DoctorRequest struct {
	Root string `json:"root,omitempty"`
}

func (DoctorRequest) ActionID() action.ID { return ActionDoctor }

type RootSetRequest struct {
	Path string `json:"path"`
}

func (RootSetRequest) ActionID() action.ID { return ActionRootSet }

type RootSetReport struct {
	Root string `json:"root"`
}

func (RootSetReport) ActionID() action.ID { return ActionRootSet }

type ColorSetRequest struct {
	Mode ColorMode `json:"mode"`
}

func (ColorSetRequest) ActionID() action.ID { return ActionColorSet }

type ColorSetReport struct {
	Mode ColorMode `json:"mode"`
}

func (ColorSetReport) ActionID() action.ID { return ActionColorSet }

type AgentConfigRequest struct {
	Root string `json:"root,omitempty"`
}

func (AgentConfigRequest) ActionID() action.ID { return ActionAgentConfig }

type AgentConfigReport struct {
	Root  string `json:"root"`
	Agent Agent  `json:"agent"`
}

func (AgentConfigReport) ActionID() action.ID { return ActionAgentConfig }

type AgentShowRequest struct {
	Root string `json:"root,omitempty"`
}

func (AgentShowRequest) ActionID() action.ID { return ActionAgentShow }

type AgentShowReport struct {
	Root  string `json:"root"`
	Agent Agent  `json:"agent"`
}

func (AgentShowReport) ActionID() action.ID { return ActionAgentShow }

type AgentDefaultSetRequest struct {
	Root  string `json:"root,omitempty"`
	Agent Agent  `json:"agent"`
}

func (AgentDefaultSetRequest) ActionID() action.ID { return ActionAgentDefaultSet }

type AgentDefaultSetReport struct {
	Root  string `json:"root"`
	Agent Agent  `json:"agent"`
}

func (AgentDefaultSetReport) ActionID() action.ID { return ActionAgentDefaultSet }

// Handlers returns the complete config handler set in stable CLI order.
func Handlers() []action.Handler {
	return []action.Handler{
		action.HandlerFunc{Action: ActionInit, ExecuteFunc: executeInit},
		action.HandlerFunc{Action: ActionRefresh, ExecuteFunc: executeRefresh},
		action.HandlerFunc{Action: ActionShow, ExecuteFunc: executeShow},
		action.HandlerFunc{Action: ActionDoctor, ExecuteFunc: executeDoctor},
		action.HandlerFunc{Action: ActionRootSet, ExecuteFunc: executeRootSet},
		action.HandlerFunc{Action: ActionColorSet, ExecuteFunc: executeColorSet},
		action.HandlerFunc{Action: ActionAgentConfig, ExecuteFunc: executeAgentConfig},
		action.HandlerFunc{Action: ActionAgentShow, ExecuteFunc: executeAgentShow},
		action.HandlerFunc{Action: ActionAgentDefaultSet, ExecuteFunc: executeAgentDefaultSet},
	}
}

func executeInit(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(InitRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionInit, request)
	}
	return InitRoot(typed)
}

func executeRefresh(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(RefreshRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionRefresh, request)
	}
	if typed.Root == "" {
		typed.Root = ResolveRoot("")
	}
	return RefreshRoot(typed)
}

func executeShow(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(ShowRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionShow, request)
	}
	return Show(typed.Root), nil
}

func executeDoctor(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(DoctorRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionDoctor, request)
	}
	return Doctor(typed.Root), nil
}

func executeRootSet(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(RootSetRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionRootSet, request)
	}
	root, err := SetUserRoot(typed.Path)
	if err != nil {
		return nil, err
	}
	return RootSetReport{Root: root}, nil
}

func executeColorSet(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(ColorSetRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionColorSet, request)
	}
	mode, err := SetColorMode(typed.Mode)
	if err != nil {
		return nil, err
	}
	return ColorSetReport{Mode: mode}, nil
}

func executeAgentConfig(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(AgentConfigRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionAgentConfig, request)
	}
	root := ResolveRoot(typed.Root)
	return AgentConfigReport{Root: root, Agent: DefaultAgent(root)}, nil
}

func executeAgentShow(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(AgentShowRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionAgentShow, request)
	}
	root := ResolveRoot(typed.Root)
	return AgentShowReport{Root: root, Agent: DefaultAgent(root)}, nil
}

func executeAgentDefaultSet(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
	typed, ok := request.(AgentDefaultSetRequest)
	if !ok {
		return nil, actionRequestTypeError(ActionAgentDefaultSet, request)
	}
	root := ResolveRoot(typed.Root)
	agent, err := SetDefaultAgent(root, typed.Agent)
	if err != nil {
		return nil, err
	}
	return AgentDefaultSetReport{Root: root, Agent: agent}, nil
}

func actionRequestTypeError(id action.ID, request action.Request) error {
	return fmt.Errorf("config.invalid-action-request:%s:%T", id, request)
}
