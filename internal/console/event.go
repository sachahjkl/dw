package console

import (
	"errors"
	"strings"
	"sync"

	"github.com/sachahjkl/dw/internal/action"
)

type EventKey = action.ID

type EventField struct {
	Key   string
	Value string
}

// EventProjection is an ordered, provider-neutral projection. Field order is part of the CLI contract.
type EventProjection struct {
	ActionID  string
	Fields    []EventField
	Transient bool
	// AppendFields opts into showing ordered structured detail after the localized human message.
	AppendFields bool
}

func FormatEvent(event EventProjection) string {
	var output strings.Builder
	output.WriteString(singleLine(event.ActionID))
	for _, field := range event.Fields {
		if field.Key == "" {
			continue
		}
		output.WriteByte(' ')
		output.WriteString(field.Key)
		output.WriteByte('=')
		output.WriteString(singleLine(field.Value))
	}
	return output.String()
}

func AppendEventFields(message string, event EventProjection) string {
	if !event.AppendFields || len(event.Fields) == 0 {
		return message
	}
	var output strings.Builder
	output.WriteString(message)
	for _, field := range event.Fields {
		if field.Key == "" {
			continue
		}
		output.WriteByte(' ')
		output.WriteString(field.Key)
		output.WriteByte('=')
		output.WriteString(singleLine(field.Value))
	}
	return output.String()
}

type EventProjector func(any) (EventProjection, error)

type EventRegistry struct {
	mu         sync.RWMutex
	projectors map[EventKey]EventProjector
}

func NewEventRegistry() *EventRegistry {
	return &EventRegistry{projectors: make(map[EventKey]EventProjector)}
}

func (r *EventRegistry) Register(kind EventKey, projector EventProjector) error {
	if kind == "" || projector == nil {
		return errors.New("console.invalid-event-renderer-registration")
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, exists := r.projectors[kind]; exists {
		return errors.New("console.duplicate-event-renderer:" + string(kind))
	}
	r.projectors[kind] = projector
	return nil
}

func RegisterEvent[T any](registry *EventRegistry, kind EventKey, projector func(T) EventProjection) error {
	return registry.Register(kind, func(payload any) (EventProjection, error) {
		value, ok := payload.(T)
		if !ok {
			return EventProjection{}, PayloadTypeError{Kind: string(kind)}
		}
		return projector(value), nil
	})
}

func (r *EventRegistry) Format(kind EventKey, payload any) (EventProjection, string, error) {
	r.mu.RLock()
	projector, ok := r.projectors[kind]
	r.mu.RUnlock()
	if !ok {
		return EventProjection{}, "", RendererNotFoundError{Kind: string(kind)}
	}
	event, err := projector(payload)
	if err != nil {
		return EventProjection{}, "", err
	}
	return event, FormatEvent(event), nil
}

func (r *EventRegistry) FormatEnvelope(envelope action.EventEnvelope) (EventProjection, string, error) {
	event, line, err := r.Format(envelope.Action, envelope.Data)
	if err != nil {
		return EventProjection{}, "", err
	}
	if event.ActionID == "" {
		event.ActionID = string(envelope.Action)
		line = FormatEvent(event)
	}
	return event, line, nil
}
