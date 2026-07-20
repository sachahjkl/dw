package action

import (
	"context"
	"fmt"
)

// EventSink receives ordered action events. Emit must not retain or mutate Data
// unless its concrete contract explicitly permits it.
type EventSink interface {
	Emit(context.Context, EventEnvelope) error
}

// EventSinkFunc adapts a function to EventSink.
type EventSinkFunc func(context.Context, EventEnvelope) error

func (f EventSinkFunc) Emit(ctx context.Context, event EventEnvelope) error { return f(ctx, event) }

// InputPort obtains a response from CLI, TUI, or another explicit UI adapter.
type InputPort interface {
	Request(context.Context, Prompt) (Response, error)
}

// InputPortFunc adapts a function to InputPort.
type InputPortFunc func(context.Context, Prompt) (Response, error)

func (f InputPortFunc) Request(ctx context.Context, prompt Prompt) (Response, error) {
	return f(ctx, prompt)
}

// Runtime contains the interactive side channels available to handlers.
type Runtime struct {
	Events EventSink
	Input  InputPort
}

// Emit safely emits when a sink is configured.
func (r Runtime) Emit(ctx context.Context, event EventEnvelope) error {
	if r.Events == nil {
		return nil
	}
	return r.Events.Emit(ctx, event)
}

// Ask validates prompt before invoking the configured input port.
func (r Runtime) Ask(ctx context.Context, prompt Prompt) (Response, error) {
	if err := prompt.Validate(); err != nil {
		return Response{}, err
	}
	if r.Input == nil {
		return Response{}, fmt.Errorf("action.input-unavailable")
	}
	response, err := r.Input.Request(ctx, prompt)
	if err != nil {
		return Response{}, err
	}
	if response.Kind != prompt.Kind {
		return Response{}, fmt.Errorf("action.input-kind-mismatch:%s:%s", prompt.Kind, response.Kind)
	}
	return response, nil
}
