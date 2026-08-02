package execution

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

func openTestSQLiteStore(t *testing.T) (*SQLiteStore, string) {
	t.Helper()
	path := filepath.Join(t.TempDir(), "execution.sqlite")
	store, err := OpenSQLiteStore(path)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = store.Close() })
	return store, path
}

func testStoredExecution(t *testing.T, status Status, resumable bool, lease time.Time) (storedExecution, Event) {
	t.Helper()
	executionID, err := NewExecutionID()
	if err != nil {
		t.Fatal(err)
	}
	attemptID, err := NewAttemptID()
	if err != nil {
		t.Fatal(err)
	}
	idempotencyKey, err := NewIdempotencyKey()
	if err != nil {
		t.Fatal(err)
	}
	executorID, err := NewExecutorID()
	if err != nil {
		t.Fatal(err)
	}
	requestJSON := json.RawMessage(`{"id":"test.sqlite","value":"request"}`)
	requestHash := sha256.Sum256(requestJSON)
	now := time.Now().UTC().Truncate(time.Millisecond)
	message, err := EncodeMessage(l10n.M("execution.event.queued"))
	if err != nil {
		t.Fatal(err)
	}
	item := storedExecution{
		Record: Record{
			ExecutionID: executionID,
			AttemptID:   attemptID,
			ActionID:    "test.sqlite",
			Status:      status,
			Root:        "/workspace",
			Principal:   "unix:1000",
			Origin:      OriginCLI,
			Request:     Encoded{Schema: 1, JSON: requestJSON, Redacted: !resumable},
			CreatedAt:   now,
		},
		RequestHash:    requestHash,
		IdempotencyKey: idempotencyKey,
		ExecutorID:     executorID,
		LeaseExpiresAt: lease,
		Resumable:      resumable,
		LastSequence:   1,
	}
	queued := Event{ExecutionID: executionID, AttemptID: attemptID, Sequence: 1, At: now, Kind: EventQueued, ActionID: item.Record.ActionID, Message: message}
	return item, queued
}

func TestSQLiteStoreInitializesSchemaPragmasAndPermissions(t *testing.T) {
	store, path := openTestSQLiteStore(t)
	var foreignKeys, busyTimeout, userVersion int
	var journalMode string
	if err := store.database.QueryRow(`PRAGMA foreign_keys`).Scan(&foreignKeys); err != nil {
		t.Fatal(err)
	}
	if err := store.database.QueryRow(`PRAGMA journal_mode`).Scan(&journalMode); err != nil {
		t.Fatal(err)
	}
	if err := store.database.QueryRow(`PRAGMA busy_timeout`).Scan(&busyTimeout); err != nil {
		t.Fatal(err)
	}
	if err := store.database.QueryRow(`PRAGMA user_version`).Scan(&userVersion); err != nil {
		t.Fatal(err)
	}
	if foreignKeys != 1 || journalMode != "wal" || busyTimeout != 5000 || userVersion != sqliteSchemaVersion {
		t.Fatalf("pragmas = foreign_keys:%d journal_mode:%s busy_timeout:%d user_version:%d", foreignKeys, journalMode, busyTimeout, userVersion)
	}
	if runtime.GOOS != "windows" {
		info, err := os.Stat(path)
		if err != nil {
			t.Fatal(err)
		}
		if info.Mode().Perm() != 0o600 {
			t.Fatalf("database mode = %o, want 600", info.Mode().Perm())
		}
	}
	for _, table := range []string{"executions", "attempts", "events", "prompts"} {
		var name string
		if err := store.database.QueryRow(`SELECT name FROM sqlite_master WHERE type='table' AND name=?`, table).Scan(&name); err != nil || name != table {
			t.Fatalf("table %s = (%q, %v)", table, name, err)
		}
	}
}

func TestSQLiteStoreIdempotencyExactMatchAndConflict(t *testing.T) {
	store, _ := openTestSQLiteStore(t)
	item, queued := testStoredExecution(t, StatusQueued, true, time.Now().Add(time.Minute))
	created, existing, err := store.Create(context.Background(), item, queued)
	if err != nil || existing || created.Record.ExecutionID != item.Record.ExecutionID {
		t.Fatalf("first create = (%s, %t, %v)", created.Record.ExecutionID, existing, err)
	}
	repeated, existing, err := store.Create(context.Background(), item, queued)
	if err != nil || !existing || repeated.Record.ExecutionID != item.Record.ExecutionID {
		t.Fatalf("repeated create = (%s, %t, %v)", repeated.Record.ExecutionID, existing, err)
	}
	conflict := item
	conflict.Record.Root = "/other"
	if _, _, err := store.Create(context.Background(), conflict, queued); err == nil || err.Error() != "execution.idempotency-conflict" {
		t.Fatalf("conflicting create error = %v", err)
	}
}

func TestSQLiteStoreRejectsCommitAndRenewAfterLeaseLoss(t *testing.T) {
	store, _ := openTestSQLiteStore(t)
	item, queued := testStoredExecution(t, StatusQueued, true, time.Now().Add(time.Minute))
	if _, _, err := store.Create(context.Background(), item, queued); err != nil {
		t.Fatal(err)
	}
	wrongOwner, err := NewExecutorID()
	if err != nil {
		t.Fatal(err)
	}
	item.Record.Status = StatusRunning
	if err = store.Commit(context.Background(), wrongOwner, item, nil, nil); err == nil || !strings.Contains(err.Error(), "execution.lease-lost") {
		t.Fatalf("commit error = %v", err)
	}
	if err = store.Renew(context.Background(), wrongOwner, []ExecutionID{item.Record.ExecutionID}, time.Now().Add(time.Minute)); err == nil || !strings.Contains(err.Error(), "execution.lease-lost") {
		t.Fatalf("renew error = %v", err)
	}
	persisted, err := store.Get(context.Background(), item.Record.ExecutionID)
	if err != nil {
		t.Fatal(err)
	}
	if persisted.Record.Status != StatusQueued || persisted.ExecutorID != item.ExecutorID {
		t.Fatalf("lease loss changed execution: %#v", persisted)
	}
}

func TestSQLiteStoreNeverPersistsSecretPromptOrResponse(t *testing.T) {
	store, _ := openTestSQLiteStore(t)
	item, queued := testStoredExecution(t, StatusQueued, true, time.Now().Add(time.Minute))
	if _, _, err := store.Create(context.Background(), item, queued); err != nil {
		t.Fatal(err)
	}
	item.Record.Status = StatusWaitingInput
	item.LastSequence = 2
	prompt := EncodedPrompt{ID: "password", Kind: action.PromptSecret, Schema: 1, JSON: json.RawMessage(`{"default":"must-not-persist"}`), Redacted: true}
	message, err := EncodeMessage(l10n.M("execution.event.input-required"))
	if err != nil {
		t.Fatal(err)
	}
	event := Event{ExecutionID: item.Record.ExecutionID, AttemptID: item.Record.AttemptID, Sequence: 2, At: time.Now().UTC(), Kind: EventInputRequired, ActionID: item.Record.ActionID, Message: message}
	if err := store.Commit(context.Background(), item.ExecutorID, item, &event, &promptUpdate{Prompt: &prompt, PromptStatus: "pending"}); err != nil {
		t.Fatal(err)
	}
	item.Record.Status = StatusRunning
	now := time.Now().UTC()
	if err := store.Commit(context.Background(), item.ExecutorID, item, nil, &promptUpdate{PromptID: prompt.ID, PromptStatus: "answered", ResponseJSON: json.RawMessage(`{"value":"secret-value"}`), RespondedAt: &now, Redacted: true}); err != nil {
		t.Fatal(err)
	}
	var promptIsNull, responseIsNull, responseRedacted int
	if err := store.database.QueryRow(`SELECT prompt_json IS NULL, response_json IS NULL, response_redacted FROM prompts WHERE execution_id=?`, identifierBytes(item.Record.ExecutionID.value)).Scan(&promptIsNull, &responseIsNull, &responseRedacted); err != nil {
		t.Fatal(err)
	}
	if promptIsNull != 1 || responseIsNull != 1 || responseRedacted != 1 {
		t.Fatalf("secret storage = prompt_null:%d response_null:%d redacted:%d", promptIsNull, responseIsNull, responseRedacted)
	}
}

func TestSQLiteStoreRecoveryClaimsOnlyExpiredResumableQueue(t *testing.T) {
	store, _ := openTestSQLiteStore(t)
	now := time.Now().UTC()
	expired, expiredEvent := testStoredExecution(t, StatusQueued, true, now.Add(-time.Second))
	valid, validEvent := testStoredExecution(t, StatusQueued, true, now.Add(time.Minute))
	if _, _, err := store.Create(context.Background(), expired, expiredEvent); err != nil {
		t.Fatal(err)
	}
	if _, _, err := store.Create(context.Background(), valid, validEvent); err != nil {
		t.Fatal(err)
	}
	newExecutor, err := NewExecutorID()
	if err != nil {
		t.Fatal(err)
	}
	claimed, err := store.Recover(context.Background(), newExecutor, now, 15*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	if len(claimed) != 1 || claimed[0].Record.ExecutionID != expired.Record.ExecutionID || claimed[0].ExecutorID != newExecutor {
		t.Fatalf("claimed = %#v", claimed)
	}
	secondExecutor, err := NewExecutorID()
	if err != nil {
		t.Fatal(err)
	}
	secondClaim, err := store.Recover(context.Background(), secondExecutor, now, 15*time.Second)
	if err != nil || len(secondClaim) != 0 {
		t.Fatalf("second claim = (%#v, %v)", secondClaim, err)
	}
}

func TestSQLiteStoreRecoveryInterruptsUnsafeAndActiveExecutions(t *testing.T) {
	store, _ := openTestSQLiteStore(t)
	now := time.Now().UTC()
	unsafe, unsafeEvent := testStoredExecution(t, StatusQueued, false, now.Add(-time.Second))
	running, runningEvent := testStoredExecution(t, StatusQueued, true, now.Add(-time.Second))
	if _, _, err := store.Create(context.Background(), unsafe, unsafeEvent); err != nil {
		t.Fatal(err)
	}
	if _, _, err := store.Create(context.Background(), running, runningEvent); err != nil {
		t.Fatal(err)
	}
	running.Record.Status = StatusRunning
	running.LastSequence = 2
	started := now.Add(-time.Minute)
	running.Record.StartedAt = &started
	message, err := EncodeMessage(l10n.M("execution.event.started"))
	if err != nil {
		t.Fatal(err)
	}
	event := Event{ExecutionID: running.Record.ExecutionID, AttemptID: running.Record.AttemptID, Sequence: 2, At: started, Kind: EventStarted, ActionID: running.Record.ActionID, Message: message}
	if err := store.Commit(context.Background(), running.ExecutorID, running, &event, nil); err != nil {
		t.Fatal(err)
	}
	executorID, err := NewExecutorID()
	if err != nil {
		t.Fatal(err)
	}
	claimed, err := store.Recover(context.Background(), executorID, now, 15*time.Second)
	if err != nil || len(claimed) != 0 {
		t.Fatalf("recover = (%#v, %v)", claimed, err)
	}
	for _, id := range []ExecutionID{unsafe.Record.ExecutionID, running.Record.ExecutionID} {
		item, err := store.Get(context.Background(), id)
		if err != nil {
			t.Fatal(err)
		}
		if item.Record.Status != StatusInterrupted || item.Record.FinishedAt == nil {
			t.Fatalf("recovered record = %#v", item.Record)
		}
		events, err := store.EventsAfter(context.Background(), id, 0, 10)
		if err != nil {
			t.Fatal(err)
		}
		last := events[len(events)-1]
		if last.Kind != EventInterrupted || !strings.Contains(string(last.Message.ID), "interrupted-restart") {
			t.Fatalf("last event = %#v", last)
		}
	}
}

func TestSQLiteStorePersistsExecutionSubject(t *testing.T) {
	store, _ := openTestSQLiteStore(t)
	item, queued := testStoredExecution(t, StatusQueued, true, time.Now().UTC().Add(time.Minute))
	item.Record.Subject = &Subject{Kind: "work-item", Project: "default", Key: "WI-42", Relation: "start"}
	if _, _, err := store.Create(context.Background(), item, queued); err != nil {
		t.Fatal(err)
	}
	stored, err := store.Get(context.Background(), item.Record.ExecutionID)
	if err != nil {
		t.Fatal(err)
	}
	if stored.Record.Subject == nil || *stored.Record.Subject != *item.Record.Subject {
		t.Fatalf("subject = %#v, want %#v", stored.Record.Subject, item.Record.Subject)
	}
}

func TestSQLiteStoreMigratesVersionOneSubjectColumns(t *testing.T) {
	store, path := openTestSQLiteStore(t)
	for _, statement := range []string{
		`ALTER TABLE executions DROP COLUMN subject_kind`,
		`ALTER TABLE executions DROP COLUMN subject_project`,
		`ALTER TABLE executions DROP COLUMN subject_key`,
		`ALTER TABLE executions DROP COLUMN subject_relation`,
		`PRAGMA user_version=1`,
	} {
		if _, err := store.database.Exec(statement); err != nil {
			t.Fatal(err)
		}
	}
	if err := store.Close(); err != nil {
		t.Fatal(err)
	}
	migrated, err := OpenSQLiteStore(path)
	if err != nil {
		t.Fatal(err)
	}
	defer migrated.Close()
	var version int
	if err = migrated.database.QueryRow(`PRAGMA user_version`).Scan(&version); err != nil {
		t.Fatal(err)
	}
	if version != sqliteSchemaVersion {
		t.Fatalf("schema version = %d, want %d", version, sqliteSchemaVersion)
	}
	item, queued := testStoredExecution(t, StatusQueued, true, time.Now().UTC().Add(time.Minute))
	item.Record.Subject = &Subject{Kind: "root", Key: "/tmp/root", Relation: "doctor"}
	if _, _, err = migrated.Create(context.Background(), item, queued); err != nil {
		t.Fatalf("create after migration: %v", err)
	}
}
