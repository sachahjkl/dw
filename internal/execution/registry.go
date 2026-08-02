package execution

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"sync"

	"github.com/sachahjkl/dw/internal/action"
)

type LockMode string

const (
	LockNone      LockMode = "none"
	LockShared    LockMode = "shared"
	LockExclusive LockMode = "exclusive"
)

type LockSpec struct {
	Mode LockMode `json:"mode"`
	Key  string   `json:"key"`
}

func (spec LockSpec) Validate() error {
	switch spec.Mode {
	case LockNone:
		if spec.Key != "" {
			return fmt.Errorf("execution.lock-none-with-key")
		}
	case LockShared, LockExclusive:
		if spec.Key == "" {
			return fmt.Errorf("execution.lock-key-required")
		}
	default:
		return fmt.Errorf("execution.invalid-lock-mode:%s", spec.Mode)
	}
	return nil
}

type Descriptor interface {
	ActionID() action.ID
	EncodeRequest(action.Request) (Encoded, error)
	DecodeRequest(Encoded) (action.Request, error)
	EncodeResult(action.Result) (Encoded, error)
	DecodeResult(Encoded) (action.Result, error)
	Lock(action.Request) (LockSpec, error)
}

type JSONDescriptor[Q action.Request, R action.Result] struct {
	ID          action.ID
	Request     Codec[Q]
	Result      Codec[R]
	RequestLock func(Q) (LockSpec, error)
}

type Codec[T action.Request] struct {
	Encode func(T) (Encoded, error)
	Decode func(Encoded) (T, error)
}

func NewJSONDescriptor[Q action.Request, R action.Result](id action.ID, lock func(Q) (LockSpec, error)) *JSONDescriptor[Q, R] {
	return &JSONDescriptor[Q, R]{
		ID:          id,
		Request:     JSONCodec[Q](),
		Result:      JSONCodec[R](),
		RequestLock: lock,
	}
}

func JSONCodec[T action.Request]() Codec[T] {
	return Codec[T]{
		Encode: func(value T) (Encoded, error) {
			encoded, err := json.Marshal(value)
			if err != nil {
				return Encoded{}, fmt.Errorf("execution.encode-json:%w", err)
			}
			return Encoded{Schema: 1, JSON: encoded}, nil
		},
		Decode: func(encoded Encoded) (T, error) {
			var value T
			if encoded.Schema != 1 || encoded.Redacted {
				return value, fmt.Errorf("execution.unsupported-encoding:%d", encoded.Schema)
			}
			decoder := json.NewDecoder(bytes.NewReader(encoded.JSON))
			decoder.DisallowUnknownFields()
			if err := decoder.Decode(&value); err != nil {
				return value, fmt.Errorf("execution.decode-json:%w", err)
			}
			if err := decoder.Decode(&struct{}{}); err != io.EOF {
				if err == nil {
					return value, fmt.Errorf("execution.decode-json-trailing-value")
				}
				return value, fmt.Errorf("execution.decode-json:%w", err)
			}
			return value, nil
		},
	}
}

func (descriptor *JSONDescriptor[Q, R]) ActionID() action.ID { return descriptor.ID }

func (descriptor *JSONDescriptor[Q, R]) EncodeRequest(request action.Request) (Encoded, error) {
	value, ok := request.(Q)
	if !ok {
		return Encoded{}, fmt.Errorf("execution.request-kind-mismatch:%s:%T", descriptor.ID, request)
	}
	if value.ActionID() != descriptor.ID {
		return Encoded{}, fmt.Errorf("execution.request-action-mismatch:%s:%s", descriptor.ID, value.ActionID())
	}
	return descriptor.Request.Encode(value)
}

func (descriptor *JSONDescriptor[Q, R]) DecodeRequest(encoded Encoded) (action.Request, error) {
	value, err := descriptor.Request.Decode(encoded)
	if err != nil {
		return nil, err
	}
	if value.ActionID() != descriptor.ID {
		return nil, fmt.Errorf("execution.request-action-mismatch:%s:%s", descriptor.ID, value.ActionID())
	}
	return value, nil
}

func (descriptor *JSONDescriptor[Q, R]) EncodeResult(result action.Result) (Encoded, error) {
	value, ok := result.(R)
	if !ok {
		return Encoded{}, fmt.Errorf("execution.result-kind-mismatch:%s:%T", descriptor.ID, result)
	}
	if value.ActionID() != descriptor.ID {
		return Encoded{}, fmt.Errorf("execution.result-action-mismatch:%s:%s", descriptor.ID, value.ActionID())
	}
	return descriptor.Result.Encode(value)
}

func (descriptor *JSONDescriptor[Q, R]) DecodeResult(encoded Encoded) (action.Result, error) {
	value, err := descriptor.Result.Decode(encoded)
	if err != nil {
		return nil, err
	}
	if value.ActionID() != descriptor.ID {
		return nil, fmt.Errorf("execution.result-action-mismatch:%s:%s", descriptor.ID, value.ActionID())
	}
	return value, nil
}

func (descriptor *JSONDescriptor[Q, R]) Lock(request action.Request) (LockSpec, error) {
	value, ok := request.(Q)
	if !ok {
		return LockSpec{}, fmt.Errorf("execution.request-kind-mismatch:%s:%T", descriptor.ID, request)
	}
	if descriptor.RequestLock == nil {
		return LockSpec{Mode: LockNone}, nil
	}
	spec, err := descriptor.RequestLock(value)
	if err != nil {
		return LockSpec{}, err
	}
	if err := spec.Validate(); err != nil {
		return LockSpec{}, err
	}
	return spec, nil
}

type Registry struct {
	mu          sync.RWMutex
	descriptors map[action.ID]Descriptor
	order       []action.ID
}

func NewRegistry() *Registry {
	return &Registry{descriptors: make(map[action.ID]Descriptor)}
}

func (registry *Registry) Register(descriptor Descriptor) error {
	if descriptor == nil || descriptor.ActionID() == "" {
		return fmt.Errorf("execution.invalid-descriptor")
	}
	registry.mu.Lock()
	defer registry.mu.Unlock()
	id := descriptor.ActionID()
	if _, exists := registry.descriptors[id]; exists {
		return fmt.Errorf("execution.duplicate-descriptor:%s", id)
	}
	registry.descriptors[id] = descriptor
	registry.order = append(registry.order, id)
	return nil
}

func (registry *Registry) Descriptor(id action.ID) (Descriptor, bool) {
	registry.mu.RLock()
	defer registry.mu.RUnlock()
	descriptor, ok := registry.descriptors[id]
	return descriptor, ok
}

func (registry *Registry) IDs() []action.ID {
	registry.mu.RLock()
	defer registry.mu.RUnlock()
	return append([]action.ID(nil), registry.order...)
}

func (registry *Registry) ValidateDispatcher(dispatcher *action.Dispatcher) error {
	if dispatcher == nil {
		return fmt.Errorf("execution.nil-dispatcher")
	}
	handlers := dispatcher.IDs()
	descriptors := registry.IDs()
	sort.Slice(handlers, func(i, j int) bool { return handlers[i] < handlers[j] })
	sort.Slice(descriptors, func(i, j int) bool { return descriptors[i] < descriptors[j] })
	if len(handlers) != len(descriptors) {
		return fmt.Errorf("execution.descriptor-parity:%d:%d", len(handlers), len(descriptors))
	}
	for index := range handlers {
		if handlers[index] != descriptors[index] {
			return fmt.Errorf("execution.descriptor-parity:%s:%s", handlers[index], descriptors[index])
		}
	}
	return nil
}
