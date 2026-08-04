package execution

import (
	"context"
	"crypto/sha256"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
)

type runtimeExecution struct {
	stored       storedExecution
	request      action.Request
	lock         LockSpec
	prompt       action.Prompt
	response     chan action.Response
	cancel       context.CancelFunc
	done         chan struct{}
	doneClosed   bool
	ownershipErr error
	subscribers  map[uint64]*eventSubscriber
}

type eventSubscriber struct {
	live   chan Event
	events chan Event
	errors chan error
	cancel context.CancelFunc
}

type Service struct {
	dispatcher *action.Dispatcher
	registry   *Registry
	events     *EventDataRegistry
	store      Store
	locker     Locker
	executorID ExecutorID
	settings   runtimeconfig.Execution

	mu         sync.Mutex
	accepting  bool
	queue      []ExecutionID
	active     *ExecutionID
	executions map[ExecutionID]*runtimeExecution
	nextSubID  uint64
	wake       chan struct{}
	ctx        context.Context
	cancel     context.CancelFunc
	workers    sync.WaitGroup
	closeOnce  sync.Once
}

func NewService(dispatcher *action.Dispatcher, registry *Registry, events *EventDataRegistry, store Store) (*Service, error) {
	return NewServiceWithConfig(dispatcher, registry, events, store, runtimeconfig.Default().Execution)
}

func NewServiceWithConfig(dispatcher *action.Dispatcher, registry *Registry, events *EventDataRegistry, store Store, settings runtimeconfig.Execution) (*Service, error) {
	locker, err := NewRootLockerWithRetry(DefaultLockDirectory(), runtimeconfig.Milliseconds(settings.LockRetryMilliseconds))
	if err != nil {
		return nil, err
	}
	return NewServiceWithLockerConfig(dispatcher, registry, events, store, locker, settings)
}

func NewServiceWithLocker(dispatcher *action.Dispatcher, registry *Registry, events *EventDataRegistry, store Store, locker Locker) (*Service, error) {
	return NewServiceWithLockerConfig(dispatcher, registry, events, store, locker, runtimeconfig.Default().Execution)
}

func NewServiceWithLockerConfig(dispatcher *action.Dispatcher, registry *Registry, events *EventDataRegistry, store Store, locker Locker, settings runtimeconfig.Execution) (*Service, error) {
	if err := runtimeconfig.ValidateExecution(settings); err != nil {
		return nil, err
	}
	if dispatcher == nil || registry == nil || events == nil || store == nil || locker == nil {
		return nil, fmt.Errorf("execution.invalid-service-dependency")
	}
	if err := registry.ValidateDispatcher(dispatcher); err != nil {
		return nil, err
	}
	executorID, err := NewExecutorID()
	if err != nil {
		return nil, err
	}
	ctx, cancel := context.WithCancel(context.Background())
	service := &Service{
		dispatcher: dispatcher, registry: registry, events: events, store: store, locker: locker, executorID: executorID, settings: settings,
		accepting: true, executions: make(map[ExecutionID]*runtimeExecution), wake: make(chan struct{}, 1), ctx: ctx, cancel: cancel,
	}
	recovered, err := store.Recover(ctx, executorID, time.Now().UTC(), runtimeconfig.Milliseconds(settings.LeaseDurationMilliseconds))
	if err != nil {
		cancel()
		return nil, err
	}
	for _, item := range recovered {
		if item.Record.Status != StatusQueued || !item.Resumable {
			continue
		}
		descriptor, ok := registry.Descriptor(item.Record.ActionID)
		if !ok {
			cancel()
			return nil, fmt.Errorf("execution.missing-descriptor:%s", item.Record.ActionID)
		}
		request, decodeErr := descriptor.DecodeRequest(item.Record.Request)
		if decodeErr != nil {
			cancel()
			return nil, decodeErr
		}
		lock, lockErr := descriptor.Lock(request)
		if lockErr != nil {
			cancel()
			return nil, lockErr
		}
		service.executions[item.Record.ExecutionID] = newRuntimeExecution(item, request, lock)
		service.queue = append(service.queue, item.Record.ExecutionID)
	}
	service.workers.Add(2)
	go service.workerLoop()
	go service.leaseLoop()
	if len(service.queue) > 0 {
		service.signalWorker()
	}
	return service, nil
}

func newRuntimeExecution(item storedExecution, request action.Request, lock LockSpec) *runtimeExecution {
	state := &runtimeExecution{stored: item, request: request, lock: lock, done: make(chan struct{}), subscribers: make(map[uint64]*eventSubscriber)}
	if item.Record.Status.Final() {
		close(state.done)
		state.doneClosed = true
	}
	return state
}

func (service *Service) Submit(ctx context.Context, submission Submission) (ExecutionID, error) {
	if err := ctx.Err(); err != nil {
		return ExecutionID{}, err
	}
	if submission.Request == nil || submission.Actor.Principal == "" || !submission.Actor.Origin.Valid() || submission.IdempotencyKey.IsZero() {
		return ExecutionID{}, fmt.Errorf("execution.invalid-submission")
	}
	if submission.Subject != nil && !submission.Subject.Valid() {
		return ExecutionID{}, fmt.Errorf("execution.invalid-subject")
	}
	descriptor, ok := service.registry.Descriptor(submission.Request.ActionID())
	if !ok {
		return ExecutionID{}, fmt.Errorf("execution.missing-descriptor:%s", submission.Request.ActionID())
	}
	encoded, err := descriptor.EncodeRequest(submission.Request)
	if err != nil {
		return ExecutionID{}, err
	}
	lock, err := descriptor.Lock(submission.Request)
	if err != nil {
		return ExecutionID{}, err
	}
	root := submission.Root
	if root != "" {
		root, err = CanonicalRoot(root)
		if err != nil {
			return ExecutionID{}, err
		}
	}
	executionID, err := NewExecutionID()
	if err != nil {
		return ExecutionID{}, err
	}
	attemptID, err := NewAttemptID()
	if err != nil {
		return ExecutionID{}, err
	}
	now := time.Now().UTC()
	requestHash := submissionHash(encoded.JSON, submission.Subject)
	message, err := EncodeMessage(l10n.M("execution.event.queued"))
	if err != nil {
		return ExecutionID{}, err
	}
	record := Record{
		ExecutionID: executionID, AttemptID: attemptID, ActionID: submission.Request.ActionID(), Status: StatusQueued,
		Root: root, Subject: cloneSubject(submission.Subject), Principal: submission.Actor.Principal, Origin: submission.Actor.Origin, Request: encoded, CreatedAt: now,
	}
	stored := storedExecution{
		Record: record, RequestHash: requestHash, IdempotencyKey: submission.IdempotencyKey, ExecutorID: service.executorID,
		LeaseExpiresAt: now.Add(runtimeconfig.Milliseconds(service.settings.LeaseDurationMilliseconds)), Resumable: !encoded.Redacted, LastSequence: 1,
	}
	queued := Event{ExecutionID: executionID, AttemptID: attemptID, Sequence: 1, At: now, Kind: EventQueued, ActionID: record.ActionID, Message: message}

	service.mu.Lock()
	defer service.mu.Unlock()
	if !service.accepting {
		return ExecutionID{}, fmt.Errorf("execution.closed")
	}
	created, existing, err := service.store.Create(ctx, stored, queued)
	if err != nil {
		return ExecutionID{}, err
	}
	if existing {
		return created.Record.ExecutionID, nil
	}
	service.executions[executionID] = newRuntimeExecution(stored, submission.Request, lock)
	service.queue = append(service.queue, executionID)
	service.signalWorker()
	return executionID, nil
}

func submissionHash(request []byte, subject *Subject) [sha256.Size]byte {
	hash := sha256.New()
	_, _ = hash.Write(request)
	if subject != nil {
		for _, value := range []string{subject.Kind, subject.Project, subject.Key, subject.Relation} {
			_, _ = hash.Write([]byte{0})
			_, _ = hash.Write([]byte(value))
		}
	}
	var result [sha256.Size]byte
	copy(result[:], hash.Sum(nil))
	return result
}

func cloneSubject(subject *Subject) *Subject {
	if subject == nil {
		return nil
	}
	copy := *subject
	return &copy
}

func (service *Service) Get(ctx context.Context, actor Actor, id ExecutionID) (Record, error) {
	if err := ctx.Err(); err != nil {
		return Record{}, err
	}
	item, err := service.store.Get(ctx, id)
	if err != nil {
		return Record{}, err
	}
	if err := authorize(actor, item.Record); err != nil {
		return Record{}, err
	}
	service.mu.Lock()
	if state := service.executions[id]; state != nil && state.stored.Record.PendingPrompt != nil {
		prompt := *state.stored.Record.PendingPrompt
		item.Record.PendingPrompt = &prompt
	}
	service.mu.Unlock()
	return service.hydrateRecord(item.Record)
}

func (service *Service) List(ctx context.Context, actor Actor, filter ListFilter) ([]Record, error) {
	if actor.Principal == "" {
		return nil, fmt.Errorf("execution.forbidden")
	}
	if filter.Root != "" {
		root, err := CanonicalRoot(filter.Root)
		if err != nil {
			return nil, err
		}
		filter.Root = root
	}
	for _, status := range filter.Statuses {
		if !status.Valid() {
			return nil, fmt.Errorf("execution.invalid-status:%s", status)
		}
	}
	items, err := service.store.List(ctx, actor.Principal, filter)
	if err != nil {
		return nil, err
	}
	records := make([]Record, 0, len(items))
	for _, item := range items {
		record, hydrateErr := service.hydrateRecord(item.Record)
		if hydrateErr != nil {
			record = item.Record
			record.TypedResult = nil
		}
		records = append(records, record)
	}
	return records, nil
}

func (service *Service) Cancel(ctx context.Context, actor Actor, id ExecutionID) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	service.mu.Lock()
	defer service.mu.Unlock()
	state, err := service.stateLocked(ctx, id)
	if err != nil {
		return err
	}
	if err := authorize(actor, state.stored.Record); err != nil {
		return err
	}
	now := time.Now().UTC()
	state.stored.CancelRequestedAt = &now
	switch state.stored.Record.Status {
	case StatusQueued:
		service.removeQueuedLocked(id)
		if err := service.transitionLocked(ctx, state, StatusCanceled, EventCanceled, "execution.event.canceled", nil); err != nil {
			return err
		}
		service.finishLocked(state)
	case StatusRunning:
		if err := service.transitionLocked(ctx, state, StatusCanceling, EventCanceling, "execution.event.canceling", nil); err != nil {
			return err
		}
		if state.cancel != nil {
			state.cancel()
		}
	case StatusWaitingInput:
		pending := state.stored.Record.PendingPrompt
		state.stored.Record.PendingPrompt = nil
		update := discardPromptUpdate(pending)
		if err := service.transitionLocked(ctx, state, StatusCanceling, EventCanceling, "execution.event.canceling", update); err != nil {
			state.stored.Record.PendingPrompt = pending
			return err
		}
		state.prompt, state.response = nil, nil
		if state.cancel != nil {
			state.cancel()
		}
	case StatusCanceling:
		return nil
	default:
		if state.stored.Record.Status.Final() {
			return nil
		}
		return fmt.Errorf("execution.invalid-status:%s", state.stored.Record.Status)
	}
	return nil
}

func (service *Service) Respond(ctx context.Context, actor Actor, id ExecutionID, promptID action.PromptID, response action.Response) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	service.mu.Lock()
	defer service.mu.Unlock()
	state, err := service.stateLocked(ctx, id)
	if err != nil {
		return err
	}
	if err := authorize(actor, state.stored.Record); err != nil {
		return err
	}
	if state.stored.Record.Status != StatusWaitingInput || state.prompt == nil || state.prompt.PromptID() != promptID {
		return fmt.Errorf("execution.prompt-not-pending:%s", promptID)
	}
	if err := action.ValidateResponse(state.prompt, response); err != nil {
		return err
	}
	encoded, redacted, err := EncodeResponse(response)
	if err != nil {
		return err
	}
	now := time.Now().UTC()
	pending := state.stored.Record.PendingPrompt
	state.stored.Record.Status = StatusRunning
	state.stored.Record.PendingPrompt = nil
	update := &promptUpdate{PromptID: promptID, PromptStatus: "answered", ResponseJSON: encoded, RespondedAt: &now, Redacted: redacted}
	if err := service.store.Commit(ctx, service.executorID, state.stored, nil, update); err != nil {
		state.stored.Record.Status = StatusWaitingInput
		state.stored.Record.PendingPrompt = pending
		return err
	}
	select {
	case state.response <- response:
		state.prompt = nil
		state.response = nil
		return nil
	default:
		return fmt.Errorf("execution.prompt-response-unavailable:%s", promptID)
	}
}

func (service *Service) Subscribe(ctx context.Context, actor Actor, id ExecutionID, after EventSequence) (Subscription, error) {
	service.mu.Lock()
	defer service.mu.Unlock()
	state, err := service.stateLocked(ctx, id)
	if err != nil {
		return Subscription{}, err
	}
	if err := authorize(actor, state.stored.Record); err != nil {
		return Subscription{}, err
	}
	backlog, err := service.loadEventBacklog(ctx, id, after)
	if err != nil {
		return Subscription{}, err
	}
	for index := range backlog {
		if backlog[index].Payload == nil {
			continue
		}
		typed, decodeErr := service.events.Decode(*backlog[index].Payload)
		if decodeErr != nil {
			return Subscription{}, decodeErr
		}
		backlog[index].TypedData = typed
	}
	events := make(chan Event, service.settings.SubscriberCapacity)
	errorsChannel := make(chan error, 1)
	if state.stored.Record.Status.Final() {
		go sendFinalBacklog(ctx, backlog, events, errorsChannel)
		return Subscription{Events: events, Errors: errorsChannel}, nil
	}
	if state.stored.ExecutorID != service.executorID {
		go service.feedPersistedSubscription(ctx, id, after, backlog, events, errorsChannel)
		return Subscription{Events: events, Errors: errorsChannel}, nil
	}
	subContext, cancel := context.WithCancel(ctx)
	service.nextSubID++
	subID := service.nextSubID
	subscriber := &eventSubscriber{live: make(chan Event, service.settings.SubscriberCapacity), events: events, errors: errorsChannel, cancel: cancel}
	state.subscribers[subID] = subscriber
	go service.feedSubscriber(subContext, id, subID, backlog, subscriber)
	return Subscription{Events: events, Errors: errorsChannel}, nil
}

func (service *Service) loadEventBacklog(ctx context.Context, id ExecutionID, after EventSequence) ([]Event, error) {
	limit := service.settings.EventFetchLimit
	var backlog []Event
	for {
		page, err := service.store.EventsAfter(ctx, id, after, limit)
		if err != nil {
			return nil, err
		}
		backlog = append(backlog, page...)
		if len(page) == 0 || limit == 0 || len(page) < int(limit) {
			return backlog, nil
		}
		after = page[len(page)-1].Sequence
	}
}

func (service *Service) Wait(ctx context.Context, actor Actor, id ExecutionID) (Record, error) {
	ticker := time.NewTicker(runtimeconfig.Milliseconds(service.settings.PersistencePollMilliseconds))
	defer ticker.Stop()
	for {
		service.mu.Lock()
		state, err := service.stateLocked(ctx, id)
		if err != nil {
			service.mu.Unlock()
			return Record{}, err
		}
		if err := authorize(actor, state.stored.Record); err != nil {
			service.mu.Unlock()
			return Record{}, err
		}
		done := state.done
		final := state.stored.Record.Status.Final()
		service.mu.Unlock()
		if final {
			return service.Get(ctx, actor, id)
		}
		select {
		case <-done:
		case <-ticker.C:
			item, getErr := service.store.Get(ctx, id)
			if getErr != nil {
				return Record{}, getErr
			}
			if err := authorize(actor, item.Record); err != nil {
				return Record{}, err
			}
			if item.Record.Status.Final() {
				return service.hydrateRecord(item.Record)
			}
		case <-ctx.Done():
			return Record{}, ctx.Err()
		}
	}
}

func (service *Service) Close(ctx context.Context) error {
	var closeErr error
	service.closeOnce.Do(func() {
		service.mu.Lock()
		service.accepting = false
		for _, id := range append([]ExecutionID(nil), service.queue...) {
			state := service.executions[id]
			if state == nil {
				continue
			}
			if state.stored.Resumable {
				previousExecutorID := state.stored.ExecutorID
				previousLease := state.stored.LeaseExpiresAt
				state.stored.ExecutorID = ExecutorID{}
				state.stored.LeaseExpiresAt = time.Time{}
				if err := service.store.Commit(context.Background(), service.executorID, state.stored, nil, nil); err != nil {
					state.stored.ExecutorID = previousExecutorID
					state.stored.LeaseExpiresAt = previousLease
					if closeErr == nil {
						closeErr = err
					}
				}
			} else if err := service.transitionLocked(context.Background(), state, StatusInterrupted, EventInterrupted, "execution.event.interrupted", nil); err != nil && closeErr == nil {
				closeErr = err
			}
		}
		service.queue = nil
		if service.active != nil {
			if state := service.executions[*service.active]; state != nil && !state.stored.Record.Status.Final() {
				pending := state.stored.Record.PendingPrompt
				state.stored.Record.PendingPrompt = nil
				if err := service.transitionLocked(context.Background(), state, StatusInterrupted, EventInterrupted, "execution.event.interrupted", discardPromptUpdate(pending)); err != nil {
					state.stored.Record.PendingPrompt = pending
					if closeErr == nil {
						closeErr = err
					}
				} else {
					state.prompt, state.response = nil, nil
					service.finishLocked(state)
				}
				if state.cancel != nil {
					state.cancel()
				}
			}
		}
		service.cancel()
		service.mu.Unlock()

		wait := make(chan struct{})
		go func() { service.workers.Wait(); close(wait) }()
		timer := time.NewTimer(runtimeconfig.Milliseconds(service.settings.CloseTimeoutMilliseconds))
		defer timer.Stop()
		select {
		case <-wait:
		case <-ctx.Done():
			closeErr = ctx.Err()
		case <-timer.C:
			closeErr = fmt.Errorf("execution.close-timeout")
		}
		if err := service.store.Close(); err != nil && closeErr == nil {
			closeErr = err
		}
	})
	return closeErr
}

func (service *Service) workerLoop() {
	defer service.workers.Done()
	for {
		id, ok := service.nextExecution()
		if !ok {
			return
		}
		service.run(id)
	}
}

func (service *Service) nextExecution() (ExecutionID, bool) {
	for {
		service.mu.Lock()
		if len(service.queue) > 0 {
			id := service.queue[0]
			service.queue = service.queue[1:]
			service.active = &id
			service.mu.Unlock()
			return id, true
		}
		service.mu.Unlock()
		select {
		case <-service.wake:
		case <-service.ctx.Done():
			return ExecutionID{}, false
		}
	}
}

func (service *Service) run(id ExecutionID) {
	service.mu.Lock()
	state := service.executions[id]
	if state == nil || state.stored.Record.Status != StatusQueued {
		service.active = nil
		service.mu.Unlock()
		return
	}
	runContext, cancel := context.WithCancel(service.ctx)
	state.cancel = cancel
	if err := service.transitionLocked(context.Background(), state, StatusRunning, EventStarted, "execution.event.started", nil); err != nil {
		service.active = nil
		service.finishLocked(state)
		service.mu.Unlock()
		return
	}
	request := state.request
	lock := state.lock
	service.mu.Unlock()

	runtime := action.Runtime{
		Events: action.EventSinkFunc(func(ctx context.Context, event action.EventEnvelope) error {
			return service.emitHandlerEvent(ctx, id, event)
		}),
		Input: action.InputPortFunc(func(ctx context.Context, prompt action.Prompt) (action.Response, error) {
			return service.requestInput(ctx, id, prompt)
		}),
	}
	var envelope action.ResultEnvelope
	lockHandle, dispatchErr := service.locker.Acquire(runContext, lock)
	if dispatchErr == nil {
		envelope, dispatchErr = service.dispatcher.Dispatch(runContext, request, runtime)
		if releaseErr := lockHandle.Release(); releaseErr != nil && dispatchErr == nil {
			dispatchErr = releaseErr
		}
	}
	cancel()

	service.mu.Lock()
	defer service.mu.Unlock()
	state = service.executions[id]
	if state == nil || state.stored.Record.Status.Final() {
		service.active = nil
		return
	}
	if envelope.Result != nil {
		descriptor, _ := service.registry.Descriptor(state.stored.Record.ActionID)
		encoded, encodeErr := descriptor.EncodeResult(envelope.Result)
		if encodeErr != nil && dispatchErr == nil {
			dispatchErr = encodeErr
		} else if encodeErr == nil {
			state.stored.Record.Result = &encoded
			state.stored.Record.TypedResult = envelope.Result
		}
	}
	status, kind, messageID := StatusSucceeded, EventSucceeded, l10n.ID("execution.event.succeeded")
	if state.ownershipErr != nil {
		status, kind, messageID = StatusInterrupted, EventInterrupted, "execution.event.interrupted"
		failure := FailureFromError(state.ownershipErr)
		state.stored.Record.Failure = &failure
	} else if errors.Is(dispatchErr, context.Canceled) {
		status, kind, messageID = StatusCanceled, EventCanceled, "execution.event.canceled"
	} else if dispatchErr != nil {
		status, kind, messageID = StatusFailed, EventFailed, "execution.event.failed"
		failure := FailureFromError(dispatchErr)
		state.stored.Record.Failure = &failure
	}
	pending := state.stored.Record.PendingPrompt
	var promptMutation *promptUpdate
	if pending != nil {
		state.stored.Record.PendingPrompt = nil
		promptMutation = discardPromptUpdate(pending)
	}
	if transitionErr := service.transitionLocked(context.Background(), state, status, kind, messageID, promptMutation); transitionErr != nil {
		state.stored.Record.PendingPrompt = pending
		failure := FailureFromError(transitionErr)
		state.stored.Record.Failure = &failure
	}
	service.active = nil
	service.finishLocked(state)
	_ = service.store.Prune(context.Background(), state.stored.Record.Root, service.settings.MaxTerminalRecordsPerRoot)
	service.signalWorker()
}

func (service *Service) requestInput(ctx context.Context, id ExecutionID, prompt action.Prompt) (action.Response, error) {
	encoded, err := EncodePrompt(prompt)
	if err != nil {
		return nil, err
	}
	service.mu.Lock()
	state := service.executions[id]
	if state == nil || state.stored.Record.Status != StatusRunning {
		service.mu.Unlock()
		return nil, fmt.Errorf("execution.prompt-invalid-state")
	}
	state.prompt = prompt
	state.response = make(chan action.Response, 1)
	state.stored.Record.PendingPrompt = &encoded
	if err := service.transitionLocked(ctx, state, StatusWaitingInput, EventInputRequired, "execution.event.input-required", &promptUpdate{Prompt: &encoded, PromptStatus: "pending"}); err != nil {
		state.prompt, state.response = nil, nil
		service.mu.Unlock()
		return nil, err
	}
	response := state.response
	service.mu.Unlock()
	select {
	case value := <-response:
		return value, nil
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

func (service *Service) emitHandlerEvent(ctx context.Context, id ExecutionID, envelope action.EventEnvelope) error {
	message, err := EncodeMessage(envelope.Message)
	if err != nil {
		return err
	}
	var payload *EncodedEventData
	if envelope.Data != nil {
		encoded, encodeErr := service.events.Encode(envelope.Data)
		if encodeErr != nil {
			return encodeErr
		}
		if len(encoded.JSON) > service.settings.MaxPayloadBytes {
			return fmt.Errorf("execution.payload-too-large")
		}
		payload = &encoded
	}
	kind := EventProgress
	switch envelope.Kind {
	case action.EventWarning:
		kind = EventWarning
	case action.EventLog:
		kind = EventLog
	case action.EventProgress:
	default:
		return fmt.Errorf("execution.invalid-handler-event-kind:%s", envelope.Kind)
	}
	service.mu.Lock()
	defer service.mu.Unlock()
	state := service.executions[id]
	if state == nil {
		return fmt.Errorf("execution.not-found:%s", id)
	}
	return service.appendEventLocked(ctx, state, kind, message, payload, envelope.Data, nil)
}

func (service *Service) transitionLocked(ctx context.Context, state *runtimeExecution, status Status, kind EventKind, messageID l10n.ID, prompt *promptUpdate) error {
	previousStatus := state.stored.Record.Status
	if err := ValidateTransition(previousStatus, status); err != nil {
		return err
	}
	message, err := EncodeMessage(l10n.M(messageID))
	if err != nil {
		return err
	}
	previousStartedAt := state.stored.Record.StartedAt
	previousFinishedAt := state.stored.Record.FinishedAt
	now := time.Now().UTC()
	state.stored.Record.Status = status
	if status == StatusRunning && state.stored.Record.StartedAt == nil {
		state.stored.Record.StartedAt = &now
	}
	if status.Final() {
		state.stored.Record.FinishedAt = &now
	}
	if err := service.appendEventLocked(ctx, state, kind, message, nil, nil, prompt); err != nil {
		state.stored.Record.Status = previousStatus
		state.stored.Record.StartedAt = previousStartedAt
		state.stored.Record.FinishedAt = previousFinishedAt
		return err
	}
	return nil
}

func (service *Service) appendEventLocked(ctx context.Context, state *runtimeExecution, kind EventKind, message MessageV1, payload *EncodedEventData, typed action.EventData, prompt *promptUpdate) error {
	maxEvents := EventSequence(service.settings.MaxEvents)
	if state.stored.LastSequence >= maxEvents ||
		state.stored.LastSequence >= maxEvents-2 && !finalEventKind(kind) && kind != EventCanceling {
		return fmt.Errorf("execution.event-limit")
	}
	sequence := state.stored.LastSequence + 1
	event := Event{
		ExecutionID: state.stored.Record.ExecutionID, AttemptID: state.stored.Record.AttemptID, Sequence: sequence,
		At: time.Now().UTC(), Kind: kind, ActionID: state.stored.Record.ActionID, Message: message, Payload: payload, TypedData: typed,
	}
	state.stored.LastSequence = sequence
	if err := service.store.Commit(ctx, service.executorID, state.stored, &event, prompt); err != nil {
		state.stored.LastSequence--
		return err
	}
	service.publishLocked(state, event)
	return nil
}

func finalEventKind(kind EventKind) bool {
	switch kind {
	case EventCanceled, EventSucceeded, EventFailed, EventInterrupted:
		return true
	default:
		return false
	}
}

func discardPromptUpdate(prompt *EncodedPrompt) *promptUpdate {
	if prompt == nil {
		return nil
	}
	return &promptUpdate{PromptID: prompt.ID, PromptStatus: "discarded", Redacted: prompt.Redacted}
}

func (service *Service) publishLocked(state *runtimeExecution, event Event) {
	for id, subscriber := range state.subscribers {
		select {
		case subscriber.live <- event:
		default:
			select {
			case subscriber.errors <- fmt.Errorf("execution.slow-subscriber"):
			default:
			}
			subscriber.cancel()
			delete(state.subscribers, id)
		}
	}
}

func (service *Service) feedSubscriber(ctx context.Context, executionID ExecutionID, subID uint64, backlog []Event, subscriber *eventSubscriber) {
	defer close(subscriber.events)
	defer close(subscriber.errors)
	for _, event := range backlog {
		select {
		case subscriber.events <- event:
		case <-ctx.Done():
			service.removeSubscriber(executionID, subID)
			return
		}
	}
	for {
		select {
		case event, ok := <-subscriber.live:
			if !ok {
				return
			}
			select {
			case subscriber.events <- event:
			case <-ctx.Done():
				service.removeSubscriber(executionID, subID)
				return
			}
		case <-ctx.Done():
			service.removeSubscriber(executionID, subID)
			return
		}
	}
}

func sendFinalBacklog(ctx context.Context, backlog []Event, events chan Event, errorsChannel chan error) {
	defer close(events)
	defer close(errorsChannel)
	for _, event := range backlog {
		select {
		case events <- event:
		case <-ctx.Done():
			return
		}
	}
}

func (service *Service) feedPersistedSubscription(ctx context.Context, id ExecutionID, after EventSequence, backlog []Event, events chan Event, errorsChannel chan error) {
	defer close(events)
	defer close(errorsChannel)
	last := after
	send := func(items []Event) bool {
		for _, event := range items {
			if event.Payload != nil {
				typed, err := service.events.Decode(*event.Payload)
				if err != nil {
					select {
					case errorsChannel <- err:
					default:
					}
					return false
				}
				event.TypedData = typed
			}
			select {
			case events <- event:
				last = event.Sequence
			case <-ctx.Done():
				return false
			}
		}
		return true
	}
	if !send(backlog) {
		return
	}
	ticker := time.NewTicker(runtimeconfig.Milliseconds(service.settings.PersistencePollMilliseconds))
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			persisted, err := service.store.EventsAfter(ctx, id, last, service.settings.EventFetchLimit)
			if err != nil {
				select {
				case errorsChannel <- err:
				default:
				}
				return
			}
			if !send(persisted) {
				return
			}
			item, err := service.store.Get(ctx, id)
			if err != nil {
				select {
				case errorsChannel <- err:
				default:
				}
				return
			}
			if item.Record.Status.Final() {
				return
			}
		}
	}
}

func (service *Service) removeSubscriber(executionID ExecutionID, subID uint64) {
	service.mu.Lock()
	defer service.mu.Unlock()
	if state := service.executions[executionID]; state != nil {
		delete(state.subscribers, subID)
	}
}

func (service *Service) leaseLoop() {
	defer service.workers.Done()
	ticker := time.NewTicker(runtimeconfig.Milliseconds(service.settings.LeaseRenewMilliseconds))
	defer ticker.Stop()
	for {
		select {
		case now := <-ticker.C:
			service.renewLeases(now.UTC())
		case <-service.ctx.Done():
			return
		}
	}
}

func (service *Service) renewLeases(now time.Time) {
	service.mu.Lock()
	defer service.mu.Unlock()
	ids := make([]ExecutionID, 0, len(service.executions))
	for id, state := range service.executions {
		if !state.stored.Record.Status.Final() && state.stored.ExecutorID == service.executorID {
			ids = append(ids, id)
		}
	}
	if len(ids) == 0 {
		return
	}
	expiresAt := now.Add(runtimeconfig.Milliseconds(service.settings.LeaseDurationMilliseconds))
	if err := service.store.Renew(context.Background(), service.executorID, ids, expiresAt); err != nil {
		ownershipErr := fmt.Errorf("execution.lease-renewal:%w", err)
		service.accepting = false
		service.queue = nil
		for _, id := range ids {
			state := service.executions[id]
			state.ownershipErr = ownershipErr
			if state.cancel != nil {
				state.cancel()
			}
		}
		service.cancel()
		return
	}
	for _, id := range ids {
		service.executions[id].stored.LeaseExpiresAt = expiresAt
	}
}

func (service *Service) stateLocked(ctx context.Context, id ExecutionID) (*runtimeExecution, error) {
	if state := service.executions[id]; state != nil {
		return state, nil
	}
	item, err := service.store.Get(ctx, id)
	if err != nil {
		return nil, err
	}
	state := newRuntimeExecution(item, nil, LockSpec{Mode: LockNone})
	service.executions[id] = state
	return state, nil
}

func (service *Service) hydrateRecord(record Record) (Record, error) {
	descriptor, ok := service.registry.Descriptor(record.ActionID)
	if !ok {
		return Record{}, fmt.Errorf("execution.missing-descriptor:%s", record.ActionID)
	}
	if record.Result != nil {
		result, err := descriptor.DecodeResult(*record.Result)
		if err != nil {
			return Record{}, err
		}
		record.TypedResult = result
	}
	return record, nil
}

func (service *Service) finishLocked(state *runtimeExecution) {
	if !state.doneClosed {
		close(state.done)
		state.doneClosed = true
	}
	for id, subscriber := range state.subscribers {
		close(subscriber.live)
		delete(state.subscribers, id)
	}
}

func (service *Service) removeQueuedLocked(id ExecutionID) {
	for index, queued := range service.queue {
		if queued == id {
			service.queue = append(service.queue[:index], service.queue[index+1:]...)
			return
		}
	}
}

func (service *Service) signalWorker() {
	select {
	case service.wake <- struct{}{}:
	default:
	}
}

func authorize(actor Actor, record Record) error {
	if actor.Principal == "" || actor.Principal != record.Principal {
		return fmt.Errorf("execution.forbidden")
	}
	return nil
}
