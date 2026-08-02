package action

import (
	"context"
	"errors"
	"strings"
	"testing"
)

type dispatcherTestRequest struct{ id ID }

func (r dispatcherTestRequest) ActionID() ID { return r.id }

type dispatcherTestResult struct{ id ID }

func (r dispatcherTestResult) ActionID() ID { return r.id }

func TestDispatcherPreservesPartialResultWithError(t *testing.T) {
	failure := errors.New("handler failed")
	dispatcher := NewDispatcher()
	err := dispatcher.Register(HandlerFunc{Action: "test.run", ExecuteFunc: func(context.Context, Request, Runtime) (Result, error) {
		return dispatcherTestResult{id: "test.run"}, failure
	}})
	if err != nil {
		t.Fatal(err)
	}

	envelope, err := dispatcher.Dispatch(context.Background(), dispatcherTestRequest{id: "test.run"}, Runtime{})
	if !errors.Is(err, failure) {
		t.Fatalf("Dispatch error = %v, want handler failure", err)
	}
	if envelope.Action != "test.run" || envelope.Result == nil || envelope.Result.ActionID() != "test.run" {
		t.Fatalf("Dispatch envelope = %#v, want partial test.run result", envelope)
	}
}

func TestDispatcherRejectsMismatchedPartialResult(t *testing.T) {
	dispatcher := NewDispatcher()
	err := dispatcher.Register(HandlerFunc{Action: "test.run", ExecuteFunc: func(context.Context, Request, Runtime) (Result, error) {
		return dispatcherTestResult{id: "test.other"}, errors.New("handler failed")
	}})
	if err != nil {
		t.Fatal(err)
	}

	envelope, err := dispatcher.Dispatch(context.Background(), dispatcherTestRequest{id: "test.run"}, Runtime{})
	if envelope.Result != nil || err == nil || !strings.HasPrefix(err.Error(), "action.result-mismatch:") {
		t.Fatalf("Dispatch = (%#v, %v), want empty result mismatch", envelope, err)
	}
}

func TestDispatcherReturnsErrorWithoutNilPartialResult(t *testing.T) {
	failure := errors.New("handler failed")
	dispatcher := NewDispatcher()
	err := dispatcher.Register(HandlerFunc{Action: "test.run", ExecuteFunc: func(context.Context, Request, Runtime) (Result, error) {
		return nil, failure
	}})
	if err != nil {
		t.Fatal(err)
	}

	envelope, err := dispatcher.Dispatch(context.Background(), dispatcherTestRequest{id: "test.run"}, Runtime{})
	if envelope.Result != nil || !errors.Is(err, failure) {
		t.Fatalf("Dispatch = (%#v, %v), want empty envelope and handler failure", envelope, err)
	}
}
