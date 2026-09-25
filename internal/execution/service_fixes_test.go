package execution

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

func TestRespondEmitsInputReceivedEvent(t *testing.T) {
	service, store := newTestService(t, "test.respond-event", func(ctx context.Context, request serviceRequest, runtime action.Runtime) (action.Result, error) {
		if _, err := runtime.Ask(ctx, action.ConfirmPrompt{Meta: action.PromptMeta{ID: "confirm", Label: l10n.M("test.confirm")}}); err != nil {
			return nil, err
		}
		return serviceResult{ID: request.ID}, nil
	})
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.respond-event"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	waitForStatus(t, service, id, StatusWaitingInput)
	if err := service.Respond(context.Background(), testActor(), id, "confirm", action.ConfirmResponse{Accepted: true}); err != nil {
		t.Fatal(err)
	}
	if _, err := service.Wait(context.Background(), testActor(), id); err != nil {
		t.Fatal(err)
	}
	events, _ := store.EventsAfter(context.Background(), id, 0, 0)
	want := []EventKind{EventQueued, EventStarted, EventInputRequired, EventInputReceived, EventSucceeded}
	if len(events) != len(want) {
		t.Fatalf("events = %#v", events)
	}
	for index, event := range events {
		if event.Kind != want[index] {
			t.Fatalf("event %d = %s, want %s", index, event.Kind, want[index])
		}
	}
}

func TestHandlerSuccessAfterCancelFinalizesCanceled(t *testing.T) {
	started, release := make(chan struct{}), make(chan struct{})
	service, _ := newTestService(t, "test.ignore-cancel", func(_ context.Context, request serviceRequest, _ action.Runtime) (action.Result, error) {
		close(started)
		<-release
		return serviceResult{ID: request.ID}, nil
	})
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.ignore-cancel"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	<-started
	if err := service.Cancel(context.Background(), testActor(), id); err != nil {
		t.Fatal(err)
	}
	close(release)
	record, err := service.Wait(context.Background(), testActor(), id)
	if err != nil || record.Status != StatusCanceled {
		t.Fatalf("record = (%#v, %v)", record, err)
	}
}

type failingFinalStore struct{ *memoryStore }

func (store failingFinalStore) Commit(ctx context.Context, owner ExecutorID, item storedExecution, event *Event, prompt *promptUpdate) error {
	if item.Record.Status.Final() && event != nil {
		return errors.New("test.commit-failed")
	}
	return store.memoryStore.Commit(ctx, owner, item, event, prompt)
}

func TestFailedFinalTransitionStillTerminates(t *testing.T) {
	dispatcher := action.NewDispatcher()
	if err := dispatcher.Register(action.HandlerFunc{Action: "test.stuck", ExecuteFunc: func(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
		return serviceResult{ID: request.ActionID()}, nil
	}}); err != nil {
		t.Fatal(err)
	}
	registry := NewRegistry()
	if err := registry.Register(NewJSONDescriptor[serviceRequest, serviceResult]("test.stuck", func(serviceRequest) (LockSpec, error) {
		return LockSpec{Mode: LockNone}, nil
	})); err != nil {
		t.Fatal(err)
	}
	locker, err := NewRootLocker(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	service, err := NewServiceWithLocker(dispatcher, registry, NewEventDataRegistry(), failingFinalStore{newMemoryStore()}, locker)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = service.Close(context.Background()) })
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.stuck"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	service.mu.Lock()
	done := service.executions[id].done
	service.mu.Unlock()
	select {
	case <-done:
	case <-ctx.Done():
		t.Fatal("execution never reached a terminal state")
	}
	service.mu.Lock()
	status := service.executions[id].stored.Record.Status
	service.mu.Unlock()
	if status != StatusFailed {
		t.Fatalf("status = %s, want failed", status)
	}
}

func TestCloseWaitsForWorkersAndRejectsLateWrites(t *testing.T) {
	started, release := make(chan struct{}), make(chan struct{})
	var emitErr error
	var wg sync.WaitGroup
	wg.Add(1)
	service, _ := newTestService(t, "test.late-write", func(ctx context.Context, request serviceRequest, runtime action.Runtime) (action.Result, error) {
		defer wg.Done()
		close(started)
		<-release
		emitErr = runtime.Emit(context.Background(), action.EventEnvelope{Action: request.ID, Kind: action.EventLog, Message: l10n.M("test.progress")})
		return nil, nil
	})
	service.settings.CloseTimeoutMilliseconds = 50
	if _, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.late-write"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)}); err != nil {
		t.Fatal(err)
	}
	<-started
	if err := service.Close(context.Background()); err == nil {
		t.Fatal("Close did not report the timeout")
	}
	close(release)
	wg.Wait()
	if emitErr == nil {
		t.Fatal("late write succeeded after Close")
	}
	if _, err := service.store.Get(context.Background(), ExecutionID{}); !errors.Is(err, errStoreClosed) {
		t.Fatalf("store Get after Close = %v", err)
	}
}

func TestConcurrentSubmitAndGet(t *testing.T) {
	service, _ := newTestService(t, "test.concurrent", func(_ context.Context, request serviceRequest, _ action.Runtime) (action.Result, error) {
		return serviceResult{ID: request.ID}, nil
	})
	var wg sync.WaitGroup
	for range 16 {
		wg.Add(1)
		go func() {
			defer wg.Done()
			id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.concurrent"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
			if err != nil {
				t.Error(err)
				return
			}
			if _, err := service.Wait(context.Background(), testActor(), id); err != nil {
				t.Error(err)
			}
		}()
	}
	wg.Wait()
}
