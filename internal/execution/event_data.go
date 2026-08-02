package execution

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sync"

	"github.com/sachahjkl/dw/internal/action"
)

type eventDataDescriptor interface {
	Type() action.EventDataType
	Schema() SchemaVersion
	Encode(action.EventData) (EncodedEventData, error)
	Decode(EncodedEventData) (action.EventData, error)
}

type EventDataCodec[T action.EventData] struct {
	Encode func(T) (Encoded, error)
	Decode func(Encoded) (T, error)
}

func JSONEventDataCodec[T action.EventData]() EventDataCodec[T] {
	return EventDataCodec[T]{
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

type jsonEventDataDescriptor[T action.EventData] struct {
	typeID action.EventDataType
	codec  EventDataCodec[T]
}

func (descriptor jsonEventDataDescriptor[T]) Type() action.EventDataType { return descriptor.typeID }
func (jsonEventDataDescriptor[T]) Schema() SchemaVersion                 { return 1 }

func (descriptor jsonEventDataDescriptor[T]) Encode(data action.EventData) (EncodedEventData, error) {
	value, ok := data.(T)
	if !ok {
		return EncodedEventData{}, fmt.Errorf("execution.event-data-kind-mismatch:%s:%T", descriptor.typeID, data)
	}
	if value.EventDataType() != descriptor.typeID || value.EventDataSchema() != 1 {
		return EncodedEventData{}, fmt.Errorf("execution.event-data-discriminator-mismatch:%s", descriptor.typeID)
	}
	encoded, err := descriptor.codec.Encode(value)
	if err != nil {
		return EncodedEventData{}, err
	}
	return EncodedEventData{Type: descriptor.typeID, Schema: encoded.Schema, JSON: encoded.JSON}, nil
}

func (descriptor jsonEventDataDescriptor[T]) Decode(encoded EncodedEventData) (action.EventData, error) {
	if encoded.Type != descriptor.typeID || encoded.Schema != descriptor.Schema() {
		return nil, fmt.Errorf("execution.event-data-discriminator-mismatch:%s", encoded.Type)
	}
	value, err := descriptor.codec.Decode(Encoded{Schema: encoded.Schema, JSON: encoded.JSON})
	if err != nil {
		return nil, err
	}
	if value.EventDataType() != descriptor.typeID || SchemaVersion(value.EventDataSchema()) != descriptor.Schema() {
		return nil, fmt.Errorf("execution.event-data-discriminator-mismatch:%s", descriptor.typeID)
	}
	return value, nil
}

type EventDataRegistry struct {
	mu          sync.RWMutex
	descriptors map[action.EventDataType]eventDataDescriptor
}

func NewEventDataRegistry() *EventDataRegistry {
	return &EventDataRegistry{descriptors: make(map[action.EventDataType]eventDataDescriptor)}
}

func RegisterEventData[T action.EventData](registry *EventDataRegistry, typeID action.EventDataType) error {
	if registry == nil || typeID == "" {
		return fmt.Errorf("execution.invalid-event-data-descriptor")
	}
	descriptor := jsonEventDataDescriptor[T]{typeID: typeID, codec: JSONEventDataCodec[T]()}
	registry.mu.Lock()
	defer registry.mu.Unlock()
	if _, exists := registry.descriptors[typeID]; exists {
		return fmt.Errorf("execution.duplicate-event-data-descriptor:%s", typeID)
	}
	registry.descriptors[typeID] = descriptor
	return nil
}

func (registry *EventDataRegistry) Encode(data action.EventData) (EncodedEventData, error) {
	if data == nil || data.EventDataType() == "" {
		return EncodedEventData{}, fmt.Errorf("execution.invalid-event-data")
	}
	registry.mu.RLock()
	descriptor, ok := registry.descriptors[data.EventDataType()]
	registry.mu.RUnlock()
	if !ok {
		return EncodedEventData{}, fmt.Errorf("execution.unregistered-event-data:%s", data.EventDataType())
	}
	return descriptor.Encode(data)
}

func (registry *EventDataRegistry) Decode(encoded EncodedEventData) (action.EventData, error) {
	registry.mu.RLock()
	descriptor, ok := registry.descriptors[encoded.Type]
	registry.mu.RUnlock()
	if !ok {
		return nil, fmt.Errorf("execution.unregistered-event-data:%s", encoded.Type)
	}
	return descriptor.Decode(encoded)
}
