package action

import (
	"context"
	"fmt"
	"sync"

	"github.com/sachahjkl/dw/internal/l10n"
)

// Handler executes requests for exactly one action ID.
type Handler interface {
	ID() ID
	Execute(context.Context, Request, Runtime) (Result, error)
}

// HandlerFunc adapts a function while retaining an explicit action ID.
type HandlerFunc struct {
	Action      ID
	ExecuteFunc func(context.Context, Request, Runtime) (Result, error)
}

func (h HandlerFunc) ID() ID { return h.Action }
func (h HandlerFunc) Execute(ctx context.Context, request Request, runtime Runtime) (Result, error) {
	if h.ExecuteFunc == nil {
		return nil, fmt.Errorf("action.nil-handler-function:%s", h.Action)
	}
	return h.ExecuteFunc(ctx, request, runtime)
}

// DuplicateHandlerError reports a static composition error.
type DuplicateHandlerError struct{ Action ID }

func (e *DuplicateHandlerError) Error() string { return "action.duplicate-handler:" + string(e.Action) }
func (e *DuplicateHandlerError) Localized() l10n.Message {
	return l10n.M("error.duplicate-action-handler", l10n.A("name", e.Action))
}

// MissingHandlerError reports an unregistered action.
type MissingHandlerError struct{ Action ID }

func (e *MissingHandlerError) Error() string { return "action.missing-handler:" + string(e.Action) }
func (e *MissingHandlerError) Localized() l10n.Message {
	return l10n.M("error.missing-action", l10n.A("action", e.Action))
}

// ResultMismatchError prevents a handler returning another action's result.
type ResultMismatchError struct{ Requested, Reported ID }

func (e *ResultMismatchError) Error() string {
	return "action.result-mismatch:" + string(e.Requested) + ":" + string(e.Reported)
}
func (e *ResultMismatchError) Localized() l10n.Message {
	return l10n.M("error.invalid-action-result", l10n.A("actual", e.Requested), l10n.A("reported", e.Reported))
}

// Dispatcher is an ordered, concurrency-safe static handler registry.
type Dispatcher struct {
	mu       sync.RWMutex
	handlers map[ID]Handler
	order    []ID
}

func NewDispatcher() *Dispatcher {
	return &Dispatcher{handlers: make(map[ID]Handler)}
}

// Register adds one handler. Duplicate IDs and empty handlers are rejected.
func (d *Dispatcher) Register(handler Handler) error {
	if handler == nil {
		return fmt.Errorf("action.nil-handler")
	}
	id := handler.ID()
	if id == "" {
		return fmt.Errorf("action.empty-handler-id")
	}
	d.mu.Lock()
	defer d.mu.Unlock()
	if _, exists := d.handlers[id]; exists {
		return &DuplicateHandlerError{Action: id}
	}
	d.handlers[id] = handler
	d.order = append(d.order, id)
	return nil
}

// Dispatch executes without holding the registry lock. The request/result IDs
// are checked at both sides of the handler boundary.
func (d *Dispatcher) Dispatch(ctx context.Context, request Request, runtime Runtime) (ResultEnvelope, error) {
	if request == nil {
		return ResultEnvelope{}, fmt.Errorf("action.nil-request")
	}
	id := request.ActionID()
	d.mu.RLock()
	handler, ok := d.handlers[id]
	d.mu.RUnlock()
	if !ok {
		return ResultEnvelope{}, &MissingHandlerError{Action: id}
	}
	result, err := handler.Execute(ctx, request, runtime)
	if err != nil {
		return ResultEnvelope{}, err
	}
	if result == nil {
		return ResultEnvelope{}, fmt.Errorf("action.nil-result:%s", id)
	}
	if result.ActionID() != id {
		return ResultEnvelope{}, &ResultMismatchError{Requested: id, Reported: result.ActionID()}
	}
	return ResultEnvelope{Action: id, Result: result}, nil
}

// IDs returns handler IDs in registration order.
func (d *Dispatcher) IDs() []ID {
	d.mu.RLock()
	defer d.mu.RUnlock()
	return append([]ID(nil), d.order...)
}

// Handler returns a registered handler without changing order.
func (d *Dispatcher) Handler(id ID) (Handler, bool) {
	d.mu.RLock()
	defer d.mu.RUnlock()
	handler, ok := d.handlers[id]
	return handler, ok
}
