package execution

import (
	"encoding/json"
	"reflect"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

func TestIdentifiersRoundTripText(t *testing.T) {
	tests := []struct {
		name  string
		new   func() (string, error)
		parse func(string) (string, error)
	}{
		{name: "executor", new: func() (string, error) { value, err := NewExecutorID(); return value.String(), err }, parse: func(text string) (string, error) { value, err := ParseExecutorID(text); return value.String(), err }},
		{name: "execution", new: func() (string, error) { value, err := NewExecutionID(); return value.String(), err }, parse: func(text string) (string, error) { value, err := ParseExecutionID(text); return value.String(), err }},
		{name: "attempt", new: func() (string, error) { value, err := NewAttemptID(); return value.String(), err }, parse: func(text string) (string, error) { value, err := ParseAttemptID(text); return value.String(), err }},
		{name: "idempotency", new: func() (string, error) { value, err := NewIdempotencyKey(); return value.String(), err }, parse: func(text string) (string, error) { value, err := ParseIdempotencyKey(text); return value.String(), err }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			text, err := test.new()
			if err != nil {
				t.Fatal(err)
			}
			if len(text) != 32 || text != strings.ToLower(text) {
				t.Fatalf("identifier = %q, want 32 lowercase hex characters", text)
			}
			parsed, err := test.parse(text)
			if err != nil || parsed != text {
				t.Fatalf("parse = (%q, %v), want %q", parsed, err, text)
			}
			if _, err := test.parse(strings.ToUpper(text)); err == nil {
				t.Fatal("uppercase identifier was accepted")
			}
		})
	}
}

func TestValidateTransitionExhaustive(t *testing.T) {
	statuses := []Status{StatusQueued, StatusRunning, StatusWaitingInput, StatusCanceling, StatusCanceled, StatusSucceeded, StatusFailed, StatusInterrupted}
	allowed := map[[2]Status]bool{
		{StatusQueued, StatusRunning}: true, {StatusQueued, StatusCanceled}: true, {StatusQueued, StatusInterrupted}: true,
		{StatusRunning, StatusWaitingInput}: true, {StatusRunning, StatusCanceling}: true, {StatusRunning, StatusSucceeded}: true, {StatusRunning, StatusFailed}: true, {StatusRunning, StatusInterrupted}: true,
		{StatusWaitingInput, StatusRunning}: true, {StatusWaitingInput, StatusCanceling}: true, {StatusWaitingInput, StatusInterrupted}: true,
		{StatusCanceling, StatusCanceled}: true, {StatusCanceling, StatusSucceeded}: true, {StatusCanceling, StatusFailed}: true, {StatusCanceling, StatusInterrupted}: true,
	}
	for _, from := range statuses {
		for _, to := range statuses {
			err := ValidateTransition(from, to)
			if allowed[[2]Status{from, to}] && err != nil {
				t.Errorf("ValidateTransition(%s, %s) = %v, want nil", from, to, err)
			}
			if !allowed[[2]Status{from, to}] && (err == nil || err.Error() != "execution.invalid-transition:"+string(from)+":"+string(to)) {
				t.Errorf("ValidateTransition(%s, %s) = %v, want invalid transition", from, to, err)
			}
		}
	}
}

func TestMessageV1RoundTripSupportedArguments(t *testing.T) {
	message := l10n.M("test.message",
		l10n.A("string", "value"),
		l10n.A("integer", int32(-12)),
		l10n.A("unsigned", uint16(12)),
		l10n.A("boolean", true),
		l10n.A("decimal", 12.5),
	)
	encoded, err := EncodeMessage(message)
	if err != nil {
		t.Fatal(err)
	}
	if encoded.Schema != 1 || len(encoded.Args) != 5 || encoded.Args[4].Value != "12.5" {
		t.Fatalf("encoded message = %#v", encoded)
	}
	decoded, err := DecodeMessage(encoded)
	if err != nil {
		t.Fatal(err)
	}
	if decoded.ID != message.ID || len(decoded.Args) != len(message.Args) {
		t.Fatalf("decoded message = %#v", decoded)
	}
}

func TestMessageV1RejectsUnsupportedArgument(t *testing.T) {
	_, err := EncodeMessage(l10n.M("test.message", l10n.A("nested", l10n.M("nested"))))
	if err == nil || !strings.HasPrefix(err.Error(), "execution.unsupported-message-argument:") {
		t.Fatalf("EncodeMessage error = %v, want unsupported argument", err)
	}
}

type registryRequest struct {
	Value string `json:"value"`
}

func (registryRequest) ActionID() action.ID { return "test.registry" }

type registryResult struct {
	Value string `json:"value"`
}

func (registryResult) ActionID() action.ID { return "test.registry" }

type registryEvent struct {
	Value string `json:"value"`
}

func (registryEvent) EventDataType() action.EventDataType { return "test.event" }
func (registryEvent) EventDataSchema() uint16             { return 1 }

func TestJSONDescriptorStrictRoundTrip(t *testing.T) {
	descriptor := NewJSONDescriptor[registryRequest, registryResult]("test.registry", noRegistryLock)
	request := registryRequest{Value: "request"}
	encoded, err := descriptor.EncodeRequest(request)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := descriptor.DecodeRequest(encoded)
	if err != nil || !reflect.DeepEqual(decoded, request) {
		t.Fatalf("DecodeRequest = (%#v, %v)", decoded, err)
	}
	encoded.JSON = json.RawMessage(`{"value":"request","unknown":true}`)
	if _, err := descriptor.DecodeRequest(encoded); err == nil {
		t.Fatal("DecodeRequest accepted an unknown field")
	}
	if _, err := descriptor.EncodeRequest(wrongRegistryRequest{}); err == nil {
		t.Fatal("EncodeRequest accepted the wrong concrete request")
	}
}

func TestEventDataRegistryStrictRoundTrip(t *testing.T) {
	registry := NewEventDataRegistry()
	if err := RegisterEventData[registryEvent](registry, "test.event"); err != nil {
		t.Fatal(err)
	}
	encoded, err := registry.Encode(registryEvent{Value: "value"})
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := registry.Decode(encoded)
	if err != nil || !reflect.DeepEqual(decoded, registryEvent{Value: "value"}) {
		t.Fatalf("Decode = (%#v, %v)", decoded, err)
	}
}

type wrongRegistryRequest struct{}

func (wrongRegistryRequest) ActionID() action.ID { return "test.registry" }

func noRegistryLock(registryRequest) (LockSpec, error) {
	return LockSpec{Mode: LockNone}, nil
}
