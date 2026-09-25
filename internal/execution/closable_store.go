package execution

import (
	"context"
	"errors"
	"sync"
	"time"
)

var errStoreClosed = errors.New("execution.store-closed")

// closableStore serializes Close with in-flight operations so that late writers observe a clean
// error instead of using a closed database.
type closableStore struct {
	inner  Store
	mu     sync.RWMutex
	closed bool
}

func (store *closableStore) begin() error {
	store.mu.RLock()
	if store.closed {
		store.mu.RUnlock()
		return errStoreClosed
	}
	return nil
}

func (store *closableStore) Create(ctx context.Context, item storedExecution, event Event) (storedExecution, bool, error) {
	if err := store.begin(); err != nil {
		return storedExecution{}, false, err
	}
	defer store.mu.RUnlock()
	return store.inner.Create(ctx, item, event)
}

func (store *closableStore) Get(ctx context.Context, id ExecutionID) (storedExecution, error) {
	if err := store.begin(); err != nil {
		return storedExecution{}, err
	}
	defer store.mu.RUnlock()
	return store.inner.Get(ctx, id)
}

func (store *closableStore) List(ctx context.Context, principal PrincipalID, filter ListFilter) ([]storedExecution, error) {
	if err := store.begin(); err != nil {
		return nil, err
	}
	defer store.mu.RUnlock()
	return store.inner.List(ctx, principal, filter)
}

func (store *closableStore) Commit(ctx context.Context, executor ExecutorID, item storedExecution, event *Event, prompt *promptUpdate) error {
	if err := store.begin(); err != nil {
		return err
	}
	defer store.mu.RUnlock()
	return store.inner.Commit(ctx, executor, item, event, prompt)
}

func (store *closableStore) EventsAfter(ctx context.Context, id ExecutionID, after EventSequence, limit uint16) ([]Event, error) {
	if err := store.begin(); err != nil {
		return nil, err
	}
	defer store.mu.RUnlock()
	return store.inner.EventsAfter(ctx, id, after, limit)
}

func (store *closableStore) Renew(ctx context.Context, executor ExecutorID, ids []ExecutionID, expiresAt time.Time) error {
	if err := store.begin(); err != nil {
		return err
	}
	defer store.mu.RUnlock()
	return store.inner.Renew(ctx, executor, ids, expiresAt)
}

func (store *closableStore) Recover(ctx context.Context, executor ExecutorID, now time.Time, lease time.Duration) ([]storedExecution, error) {
	if err := store.begin(); err != nil {
		return nil, err
	}
	defer store.mu.RUnlock()
	return store.inner.Recover(ctx, executor, now, lease)
}

func (store *closableStore) Prune(ctx context.Context, root string, keep uint16) error {
	if err := store.begin(); err != nil {
		return err
	}
	defer store.mu.RUnlock()
	return store.inner.Prune(ctx, root, keep)
}

func (store *closableStore) Close() error {
	store.mu.Lock()
	defer store.mu.Unlock()
	if store.closed {
		return nil
	}
	store.closed = true
	return store.inner.Close()
}
