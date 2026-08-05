package execution

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

type memoryStore struct {
	mu      sync.Mutex
	records map[ExecutionID]storedExecution
	events  map[ExecutionID][]Event
}

func TestListCanonicalizesRootFilter(t *testing.T) {
	const actionID action.ID = "test.list-root"
	service, _ := newTestService(t, actionID, func(_ context.Context, request serviceRequest, _ action.Runtime) (action.Result, error) {
		return serviceResult{ID: request.ID}, nil
	})
	root := t.TempDir()
	id, err := service.Submit(context.Background(), Submission{
		Request: serviceRequest{ID: actionID}, Root: root, Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t),
	})
	if err != nil {
		t.Fatal(err)
	}
	filterRoot := filepath.Join(root, ".")
	if runtime.GOOS == "windows" {
		filterRoot = strings.ToUpper(filterRoot)
	}
	records, err := service.List(context.Background(), testActor(), ListFilter{Root: filterRoot})
	if err != nil {
		t.Fatal(err)
	}
	if len(records) != 1 || records[0].ExecutionID != id {
		t.Fatalf("records = %#v, want execution %s", records, id)
	}
}

func TestListKeepsExecutionWithHistoricalUnreadableResult(t *testing.T) {
	const actionID action.ID = "test.list-historical-result"
	service, store := newTestService(t, actionID, func(_ context.Context, request serviceRequest, _ action.Runtime) (action.Result, error) {
		return serviceResult{ID: request.ID}, nil
	})
	id, err := service.Submit(context.Background(), Submission{
		Request: serviceRequest{ID: actionID}, Root: t.TempDir(), Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t),
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, err = service.Wait(context.Background(), testActor(), id); err != nil {
		t.Fatal(err)
	}
	store.mu.Lock()
	item := store.records[id]
	historical := Encoded{Schema: 1, JSON: json.RawMessage(`{"kind":"historical"}`)}
	item.Record.Result = &historical
	store.records[id] = item
	store.mu.Unlock()
	if _, err = service.Get(context.Background(), testActor(), id); err == nil {
		t.Fatal("Get accepted an unreadable historical result")
	}
	records, err := service.List(context.Background(), testActor(), ListFilter{})
	if err != nil {
		t.Fatal(err)
	}
	if len(records) != 1 || records[0].ExecutionID != id || records[0].TypedResult != nil || records[0].Result == nil || string(records[0].Result.JSON) != string(historical.JSON) {
		t.Fatalf("records = %#v", records)
	}
}

func newMemoryStore() *memoryStore {
	return &memoryStore{records: make(map[ExecutionID]storedExecution), events: make(map[ExecutionID][]Event)}
}

func (store *memoryStore) Create(_ context.Context, item storedExecution, event Event) (storedExecution, bool, error) {
	store.mu.Lock()
	defer store.mu.Unlock()
	for _, existing := range store.records {
		if existing.Record.Principal != item.Record.Principal || existing.IdempotencyKey != item.IdempotencyKey {
			continue
		}
		if existing.Record.ActionID == item.Record.ActionID && existing.Record.Root == item.Record.Root && existing.RequestHash == item.RequestHash {
			return existing, true, nil
		}
		return storedExecution{}, false, fmt.Errorf("execution.idempotency-conflict")
	}
	store.records[item.Record.ExecutionID] = item
	store.events[item.Record.ExecutionID] = []Event{event}
	return item, false, nil
}

func (store *memoryStore) Get(_ context.Context, id ExecutionID) (storedExecution, error) {
	store.mu.Lock()
	defer store.mu.Unlock()
	item, ok := store.records[id]
	if !ok {
		return storedExecution{}, fmt.Errorf("execution.not-found:%s", id)
	}
	return item, nil
}

func (store *memoryStore) List(_ context.Context, principal PrincipalID, filter ListFilter) ([]storedExecution, error) {
	store.mu.Lock()
	defer store.mu.Unlock()
	items := make([]storedExecution, 0)
	for _, item := range store.records {
		if item.Record.Principal != principal || filter.Root != "" && item.Record.Root != filter.Root || !statusIncluded(item.Record.Status, filter.Statuses) {
			continue
		}
		items = append(items, item)
	}
	sort.Slice(items, func(i, j int) bool { return items[i].Record.CreatedAt.After(items[j].Record.CreatedAt) })
	if filter.Limit != 0 && len(items) > int(filter.Limit) {
		items = items[:filter.Limit]
	}
	return items, nil
}

func statusIncluded(status Status, filters []Status) bool {
	if len(filters) == 0 {
		return true
	}
	for _, filter := range filters {
		if status == filter {
			return true
		}
	}
	return false
}

func (store *memoryStore) Commit(_ context.Context, owner ExecutorID, item storedExecution, event *Event, _ *promptUpdate) error {
	store.mu.Lock()
	defer store.mu.Unlock()
	current, ok := store.records[item.Record.ExecutionID]
	if !ok {
		return fmt.Errorf("execution.not-found:%s", item.Record.ExecutionID)
	}
	if current.ExecutorID != owner {
		return fmt.Errorf("execution.lease-lost:%s", item.Record.ExecutionID)
	}
	if event != nil {
		events := store.events[item.Record.ExecutionID]
		if event.Sequence != EventSequence(len(events)+1) {
			return fmt.Errorf("execution.noncontiguous-sequence")
		}
		store.events[item.Record.ExecutionID] = append(events, *event)
	}
	store.records[item.Record.ExecutionID] = item
	return nil
}

func (store *memoryStore) EventsAfter(_ context.Context, id ExecutionID, after EventSequence, limit uint16) ([]Event, error) {
	store.mu.Lock()
	defer store.mu.Unlock()
	events, ok := store.events[id]
	if !ok {
		return nil, fmt.Errorf("execution.not-found:%s", id)
	}
	result := make([]Event, 0)
	for _, event := range events {
		if event.Sequence > after {
			result = append(result, event)
		}
		if limit != 0 && len(result) == int(limit) {
			break
		}
	}
	return result, nil
}

func (store *memoryStore) Renew(_ context.Context, executor ExecutorID, ids []ExecutionID, expires time.Time) error {
	store.mu.Lock()
	defer store.mu.Unlock()
	for _, id := range ids {
		item := store.records[id]
		if item.ExecutorID != executor || item.Record.Status.Final() {
			return fmt.Errorf("execution.lease-lost:%s", id)
		}
		item.LeaseExpiresAt = expires
		store.records[id] = item
	}
	return nil
}

func (*memoryStore) Recover(context.Context, ExecutorID, time.Time, time.Duration) ([]storedExecution, error) {
	return nil, nil
}
func (*memoryStore) Prune(context.Context, string, uint16) error { return nil }
func (*memoryStore) Close() error                                { return nil }

type serviceRequest struct {
	ID    action.ID `json:"id"`
	Value string    `json:"value"`
}

func (request serviceRequest) ActionID() action.ID { return request.ID }

type serviceResult struct {
	ID    action.ID `json:"id"`
	Value string    `json:"value"`
}

func (result serviceResult) ActionID() action.ID { return result.ID }

type serviceEvent struct {
	Value string `json:"value"`
}

func (serviceEvent) EventDataType() action.EventDataType { return "test.service-event" }
func (serviceEvent) EventDataSchema() uint16             { return 1 }

func newTestService(t *testing.T, id action.ID, execute func(context.Context, serviceRequest, action.Runtime) (action.Result, error)) (*Service, *memoryStore) {
	t.Helper()
	dispatcher := action.NewDispatcher()
	if err := dispatcher.Register(action.HandlerFunc{Action: id, ExecuteFunc: func(ctx context.Context, request action.Request, runtime action.Runtime) (action.Result, error) {
		typed, ok := request.(serviceRequest)
		if !ok {
			return nil, fmt.Errorf("test.invalid-request")
		}
		return execute(ctx, typed, runtime)
	}}); err != nil {
		t.Fatal(err)
	}
	registry := NewRegistry()
	if err := registry.Register(NewJSONDescriptor[serviceRequest, serviceResult](id, func(serviceRequest) (LockSpec, error) {
		return LockSpec{Mode: LockNone}, nil
	})); err != nil {
		t.Fatal(err)
	}
	events := NewEventDataRegistry()
	if err := RegisterEventData[serviceEvent](events, "test.service-event"); err != nil {
		t.Fatal(err)
	}
	store := newMemoryStore()
	service, err := NewService(dispatcher, registry, events, store)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		closeContext, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = service.Close(closeContext)
	})
	return service, store
}

func testActor() Actor { return Actor{Principal: "unix:1000", Origin: OriginCLI} }

func newTestIdempotencyKey(t *testing.T) IdempotencyKey {
	t.Helper()
	key, err := NewIdempotencyKey()
	if err != nil {
		t.Fatal(err)
	}
	return key
}

func TestServiceSequencesLifecycleAndPreservesPartialResult(t *testing.T) {
	service, store := newTestService(t, "test.partial", func(ctx context.Context, request serviceRequest, runtime action.Runtime) (action.Result, error) {
		if err := runtime.Emit(ctx, action.EventEnvelope{Action: request.ID, Kind: action.EventProgress, Message: l10n.M("test.progress"), Data: serviceEvent{Value: "known"}}); err != nil {
			return nil, err
		}
		return serviceResult{ID: request.ID, Value: "partial"}, errors.New("test.failure")
	})
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.partial"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	record, err := service.Wait(context.Background(), testActor(), id)
	if err != nil {
		t.Fatal(err)
	}
	if record.Status != StatusFailed || record.TypedResult.(serviceResult).Value != "partial" || record.Failure == nil {
		t.Fatalf("record = %#v", record)
	}
	events, err := store.EventsAfter(context.Background(), id, 0, 0)
	if err != nil {
		t.Fatal(err)
	}
	wantKinds := []EventKind{EventQueued, EventStarted, EventProgress, EventFailed}
	if len(events) != len(wantKinds) {
		t.Fatalf("events = %#v", events)
	}
	for index, event := range events {
		if event.Sequence != EventSequence(index+1) || event.Kind != wantKinds[index] {
			t.Fatalf("event %d = %#v", index, event)
		}
	}
}

func TestFinalSubscriptionPaginatesCompleteEventBacklog(t *testing.T) {
	service, _ := newTestService(t, "test.paged-events", func(ctx context.Context, request serviceRequest, runtime action.Runtime) (action.Result, error) {
		for index := 0; index < 7; index++ {
			if err := runtime.Emit(ctx, action.EventEnvelope{Action: request.ID, Kind: action.EventProgress, Message: l10n.M("test.progress"), Data: serviceEvent{Value: fmt.Sprint(index)}}); err != nil {
				return nil, err
			}
		}
		return serviceResult{ID: request.ID}, nil
	})
	service.settings.EventFetchLimit = 3
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.paged-events"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	if _, err = service.Wait(context.Background(), testActor(), id); err != nil {
		t.Fatal(err)
	}
	subscription, err := service.Subscribe(context.Background(), testActor(), id, 0)
	if err != nil {
		t.Fatal(err)
	}
	var events []Event
	for event := range subscription.Events {
		events = append(events, event)
	}
	for streamErr := range subscription.Errors {
		if streamErr != nil {
			t.Fatal(streamErr)
		}
	}
	if len(events) != 10 {
		t.Fatalf("replayed events = %d, want 10", len(events))
	}
	for index, event := range events {
		if event.Sequence != EventSequence(index+1) {
			t.Fatalf("event %d sequence = %d", index, event.Sequence)
		}
	}
}

func TestServiceIdempotencyReturnsExistingAndRejectsConflict(t *testing.T) {
	service, _ := newTestService(t, "test.idempotency", func(_ context.Context, request serviceRequest, _ action.Runtime) (action.Result, error) {
		return serviceResult(request), nil
	})
	key := newTestIdempotencyKey(t)
	submission := Submission{Request: serviceRequest{ID: "test.idempotency", Value: "one"}, Root: "/root", Actor: testActor(), IdempotencyKey: key}
	first, err := service.Submit(context.Background(), submission)
	if err != nil {
		t.Fatal(err)
	}
	second, err := service.Submit(context.Background(), submission)
	if err != nil || second != first {
		t.Fatalf("second Submit = (%s, %v), want %s", second, err, first)
	}
	submission.Request = serviceRequest{ID: "test.idempotency", Value: "two"}
	if _, err := service.Submit(context.Background(), submission); err == nil || err.Error() != "execution.idempotency-conflict" {
		t.Fatalf("conflicting Submit error = %v", err)
	}
}

func TestServicePromptAndTargetedCancellation(t *testing.T) {
	started := make(chan struct{})
	release := make(chan struct{})
	service, _ := newTestService(t, "test.prompt", func(ctx context.Context, request serviceRequest, runtime action.Runtime) (action.Result, error) {
		if request.Value == "block" {
			close(started)
			select {
			case <-release:
				return serviceResult{ID: request.ID}, nil
			case <-ctx.Done():
				return nil, ctx.Err()
			}
		}
		response, err := runtime.Ask(ctx, action.ConfirmPrompt{Meta: action.PromptMeta{ID: "confirm", Label: l10n.M("test.confirm")}})
		if err != nil {
			return nil, err
		}
		return serviceResult{ID: request.ID, Value: fmt.Sprint(response.(action.ConfirmResponse).Accepted)}, nil
	})
	active, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.prompt", Value: "block"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	<-started
	queued, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.prompt", Value: "prompt"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	if err := service.Cancel(context.Background(), testActor(), queued); err != nil {
		t.Fatal(err)
	}
	queuedRecord, err := service.Wait(context.Background(), testActor(), queued)
	if err != nil || queuedRecord.Status != StatusCanceled {
		t.Fatalf("queued record = (%#v, %v)", queuedRecord, err)
	}
	if err := service.Cancel(context.Background(), testActor(), active); err != nil {
		t.Fatal(err)
	}
	activeRecord, err := service.Wait(context.Background(), testActor(), active)
	if err != nil || activeRecord.Status != StatusCanceled {
		t.Fatalf("active record = (%#v, %v)", activeRecord, err)
	}
	close(release)

	promptID, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.prompt", Value: "prompt"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	waitForStatus(t, service, promptID, StatusWaitingInput)
	if err := service.Respond(context.Background(), testActor(), promptID, "confirm", action.TextResponse{Value: "wrong"}); err == nil {
		t.Fatal("Respond accepted the wrong response type")
	}
	if err := service.Respond(context.Background(), testActor(), promptID, "confirm", action.ConfirmResponse{Accepted: true}); err != nil {

		t.Fatal(err)
	}
	promptRecord, err := service.Wait(context.Background(), testActor(), promptID)
	if err != nil || promptRecord.Status != StatusSucceeded || promptRecord.TypedResult.(serviceResult).Value != "true" {
		t.Fatalf("prompt record = (%#v, %v)", promptRecord, err)
	}
}
func TestServiceReconnectsAfterSequenceAndClosesSlowSubscriber(t *testing.T) {
	started := make(chan struct{})
	release := make(chan struct{})
	service, _ := newTestService(t, "test.stream", func(ctx context.Context, request serviceRequest, runtime action.Runtime) (action.Result, error) {
		close(started)
		<-release
		for index := range 200 {
			if err := runtime.Emit(ctx, action.EventEnvelope{Action: request.ID, Kind: action.EventProgress, Message: l10n.M("test.progress"), Data: serviceEvent{Value: fmt.Sprint(index)}}); err != nil {
				return nil, err
			}
		}
		return serviceResult{ID: request.ID}, nil
	})
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.stream"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	<-started
	slow, err := service.Subscribe(context.Background(), testActor(), id, 0)
	if err != nil {
		t.Fatal(err)
	}
	close(release)
	select {
	case streamErr := <-slow.Errors:
		if streamErr == nil || streamErr.Error() != "execution.slow-subscriber" {
			t.Fatalf("slow subscriber error = %v", streamErr)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("slow subscriber remained open")
	}
	if _, err := service.Wait(context.Background(), testActor(), id); err != nil {
		t.Fatal(err)
	}
	resumed, err := service.Subscribe(context.Background(), testActor(), id, 200)
	if err != nil {
		t.Fatal(err)
	}
	var sequences []EventSequence
	for event := range resumed.Events {
		sequences = append(sequences, event.Sequence)
	}
	for _, sequence := range sequences {
		if sequence <= 200 {
			t.Fatalf("resumed sequence = %d, want greater than 200", sequence)
		}
	}
}

func TestServiceRejectsAnotherPrincipal(t *testing.T) {
	service, _ := newTestService(t, "test.forbidden", func(_ context.Context, request serviceRequest, _ action.Runtime) (action.Result, error) {
		return serviceResult{ID: request.ID}, nil
	})
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.forbidden"}, Root: "/root", Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	other := Actor{Principal: "unix:1001", Origin: OriginWeb}
	if _, err := service.Get(context.Background(), other, id); err == nil || err.Error() != "execution.forbidden" {
		t.Fatalf("Get error = %v, want execution.forbidden", err)
	}
	if err := service.Cancel(context.Background(), other, id); err == nil || err.Error() != "execution.forbidden" {
		t.Fatalf("Cancel error = %v, want execution.forbidden", err)
	}
}

func TestServiceHoldsDescriptorLockForHandlerDuration(t *testing.T) {
	root := t.TempDir()
	locker, err := NewRootLocker(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	blocker, err := locker.Acquire(context.Background(), LockSpec{Mode: LockExclusive, Key: root})
	if err != nil {
		t.Fatal(err)
	}
	started := make(chan struct{})
	dispatcher := action.NewDispatcher()
	if err := dispatcher.Register(action.HandlerFunc{Action: "test.locked", ExecuteFunc: func(_ context.Context, request action.Request, _ action.Runtime) (action.Result, error) {
		close(started)
		return serviceResult{ID: request.ActionID()}, nil
	}}); err != nil {
		t.Fatal(err)
	}
	registry := NewRegistry()
	if err := registry.Register(NewJSONDescriptor[serviceRequest, serviceResult]("test.locked", func(serviceRequest) (LockSpec, error) {
		return LockSpec{Mode: LockExclusive, Key: root}, nil
	})); err != nil {
		t.Fatal(err)
	}
	service, err := NewServiceWithLocker(dispatcher, registry, NewEventDataRegistry(), newMemoryStore(), locker)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		closeContext, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		_ = service.Close(closeContext)
	})
	id, err := service.Submit(context.Background(), Submission{Request: serviceRequest{ID: "test.locked"}, Root: root, Actor: testActor(), IdempotencyKey: newTestIdempotencyKey(t)})
	if err != nil {
		t.Fatal(err)
	}
	waitForStatus(t, service, id, StatusRunning)
	select {
	case <-started:
		t.Fatal("handler started before the root lock was released")
	case <-time.After(100 * time.Millisecond):
	}
	if err := blocker.Release(); err != nil {
		t.Fatal(err)
	}
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("handler did not start after the root lock was released")
	}
	record, err := service.Wait(context.Background(), testActor(), id)
	if err != nil || record.Status != StatusSucceeded {
		t.Fatalf("Wait = (%#v, %v)", record, err)
	}
}

func waitForStatus(t *testing.T, service *Service, id ExecutionID, status Status) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		record, err := service.Get(context.Background(), testActor(), id)
		if err == nil && record.Status == status {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatalf("execution %s did not reach %s", id, status)
}
