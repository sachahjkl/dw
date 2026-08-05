package execution

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"time"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/runtimeconfig"
	_ "modernc.org/sqlite"
)

const sqliteSchemaVersion = 2

const executionColumns = `
execution_id, attempt_id, action_id, request_schema, request_json,
request_redacted, request_hash, root, subject_kind, subject_project,
subject_key, subject_relation, principal, origin, idempotency_key, status,
executor_id, lease_expires_at, resumable, created_at, started_at,
finished_at, cancel_requested_at, result_schema, result_json,
result_redacted, error_code, error_message_json,
(SELECT COALESCE(MAX(sequence), 0) FROM events WHERE events.execution_id = executions.execution_id)`

type SQLiteStore struct {
	database *sql.DB
}

type executionColumnsValue struct {
	executionID       []byte
	attemptID         []byte
	actionID          string
	requestSchema     int64
	requestJSON       []byte
	requestRedacted   int64
	requestHash       []byte
	root              string
	subjectKind       sql.NullString
	subjectProject    sql.NullString
	subjectKey        sql.NullString
	subjectRelation   sql.NullString
	principal         string
	origin            string
	idempotencyKey    []byte
	status            string
	executorID        []byte
	leaseExpiresAt    sql.NullInt64
	resumable         int64
	createdAt         int64
	startedAt         sql.NullInt64
	finishedAt        sql.NullInt64
	cancelRequestedAt sql.NullInt64
	resultSchema      sql.NullInt64
	resultJSON        []byte
	resultRedacted    sql.NullInt64
	errorCode         sql.NullString
	errorMessageJSON  []byte
	lastSequence      int64
}

func DefaultSQLitePath() string {
	dirs := config.ResolvePlatformBaseDirs()
	base := dirs.StateDir
	if runtime.GOOS == "windows" {
		base = dirs.DataLocalDir
	}
	if base == "" {
		base = dirs.HomeDir
	}
	return filepath.Join(base, "DevWorkflow", "execution-v1.sqlite")
}

func OpenSQLiteStore(path string) (*SQLiteStore, error) {
	if path == "" {
		path = DefaultSQLitePath()
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, fmt.Errorf("execution.sqlite-directory:%w", err)
	}
	database, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, fmt.Errorf("execution.sqlite-open:%w", err)
	}
	database.SetMaxOpenConns(1)
	store := &SQLiteStore{database: database}
	if err := store.initialize(); err != nil {
		_ = database.Close()
		return nil, err
	}
	if err := os.Chmod(path, 0o600); err != nil {
		_ = database.Close()
		return nil, fmt.Errorf("execution.sqlite-permissions:%w", err)
	}
	return store, nil
}

func (store *SQLiteStore) initialize() error {
	var currentVersion int
	if err := store.database.QueryRow(`PRAGMA user_version`).Scan(&currentVersion); err != nil {
		return fmt.Errorf("execution.sqlite-version:%w", err)
	}
	if currentVersion > sqliteSchemaVersion {
		return fmt.Errorf("execution.sqlite-version:%d", currentVersion)
	}
	statements := []string{
		`PRAGMA foreign_keys=ON`,
		`PRAGMA journal_mode=WAL`,
		`PRAGMA busy_timeout=5000`,
		`CREATE TABLE IF NOT EXISTS executions (
			execution_id BLOB PRIMARY KEY CHECK(length(execution_id)=16),
			attempt_id BLOB NOT NULL UNIQUE CHECK(length(attempt_id)=16),
			action_id TEXT NOT NULL,
			request_schema INTEGER NOT NULL CHECK(request_schema>0),
			request_json BLOB,
			request_redacted INTEGER NOT NULL CHECK(request_redacted IN (0,1)),
			request_hash BLOB NOT NULL CHECK(length(request_hash)=32),
			root TEXT NOT NULL,
			subject_kind TEXT,
			subject_project TEXT,
			subject_key TEXT,
			subject_relation TEXT,
			principal TEXT NOT NULL,
			origin TEXT NOT NULL CHECK(origin IN ('cli','tui','web')),
			idempotency_key BLOB NOT NULL CHECK(length(idempotency_key)=16),
			status TEXT NOT NULL CHECK(status IN ('queued','running','waiting-input','canceling','canceled','succeeded','failed','interrupted')),
			executor_id BLOB CHECK(executor_id IS NULL OR length(executor_id)=16),
			lease_expires_at INTEGER,
			resumable INTEGER NOT NULL CHECK(resumable IN (0,1)),
			created_at INTEGER NOT NULL,
			started_at INTEGER,
			finished_at INTEGER,
			cancel_requested_at INTEGER,
			result_schema INTEGER,
			result_json BLOB,
			result_redacted INTEGER CHECK(result_redacted IS NULL OR result_redacted IN (0,1)),
			error_code TEXT,
			error_message_json BLOB,
			UNIQUE(principal, idempotency_key)
		)`,
		`CREATE TABLE IF NOT EXISTS attempts (
			attempt_id BLOB PRIMARY KEY CHECK(length(attempt_id)=16),
			execution_id BLOB NOT NULL CHECK(length(execution_id)=16),
			ordinal INTEGER NOT NULL CHECK(ordinal=1),
			status TEXT NOT NULL CHECK(status IN ('queued','running','waiting-input','canceling','canceled','succeeded','failed','interrupted')),
			started_at INTEGER,
			finished_at INTEGER,
			FOREIGN KEY(execution_id) REFERENCES executions(execution_id) ON DELETE CASCADE
		)`,
		`CREATE TABLE IF NOT EXISTS events (
			execution_id BLOB NOT NULL CHECK(length(execution_id)=16),
			attempt_id BLOB NOT NULL CHECK(length(attempt_id)=16),
			sequence INTEGER NOT NULL CHECK(sequence>0),
			at INTEGER NOT NULL,
			kind TEXT NOT NULL,
			action_id TEXT NOT NULL,
			message_json BLOB NOT NULL,
			payload_type TEXT,
			payload_schema INTEGER,
			payload_json BLOB,
			PRIMARY KEY(execution_id, sequence),
			FOREIGN KEY(execution_id) REFERENCES executions(execution_id) ON DELETE CASCADE,
			FOREIGN KEY(attempt_id) REFERENCES attempts(attempt_id) ON DELETE CASCADE
		)`,
		`CREATE TABLE IF NOT EXISTS prompts (
			execution_id BLOB NOT NULL CHECK(length(execution_id)=16),
			attempt_id BLOB NOT NULL CHECK(length(attempt_id)=16),
			prompt_id TEXT NOT NULL,
			kind TEXT NOT NULL CHECK(kind IN ('text','secret','confirm','select-one','select-many')),
			schema INTEGER NOT NULL CHECK(schema>0),
			prompt_json BLOB,
			status TEXT NOT NULL CHECK(status IN ('pending','answered')),
			created_at INTEGER NOT NULL,
			responded_at INTEGER,
			response_json BLOB,
			response_redacted INTEGER NOT NULL CHECK(response_redacted IN (0,1)),
			PRIMARY KEY(execution_id, prompt_id),
			FOREIGN KEY(execution_id) REFERENCES executions(execution_id) ON DELETE CASCADE,
			FOREIGN KEY(attempt_id) REFERENCES attempts(attempt_id) ON DELETE CASCADE,
			CHECK(kind <> 'secret' OR prompt_json IS NULL),
			CHECK(kind <> 'secret' OR response_json IS NULL),
			CHECK(kind <> 'secret' OR status <> 'answered' OR response_redacted=1)
		)`,
		`CREATE INDEX IF NOT EXISTS executions_principal_root_created ON executions(principal, root, created_at)`,
		`CREATE INDEX IF NOT EXISTS executions_status_lease ON executions(status, lease_expires_at)`,
		`CREATE INDEX IF NOT EXISTS events_execution_sequence ON events(execution_id, sequence)`,
	}
	for _, statement := range statements {
		if _, err := store.database.Exec(statement); err != nil {
			return fmt.Errorf("execution.sqlite-schema:%w", err)
		}
	}
	if currentVersion != 1 {
		if _, err := store.database.Exec(fmt.Sprintf("PRAGMA user_version=%d", sqliteSchemaVersion)); err != nil {
			return fmt.Errorf("execution.sqlite-version:%w", err)
		}
		return nil
	}
	transaction, err := store.database.Begin()
	if err != nil {
		return fmt.Errorf("execution.sqlite-migration:%w", err)
	}
	defer transaction.Rollback()
	for _, statement := range []string{
		`ALTER TABLE executions ADD COLUMN subject_kind TEXT`,
		`ALTER TABLE executions ADD COLUMN subject_project TEXT`,
		`ALTER TABLE executions ADD COLUMN subject_key TEXT`,
		`ALTER TABLE executions ADD COLUMN subject_relation TEXT`,
		fmt.Sprintf("PRAGMA user_version=%d", sqliteSchemaVersion),
	} {
		if _, err = transaction.Exec(statement); err != nil {
			return fmt.Errorf("execution.sqlite-migration:%w", err)
		}
	}
	if err = transaction.Commit(); err != nil {
		return fmt.Errorf("execution.sqlite-migration:%w", err)
	}
	return nil
}

func (store *SQLiteStore) Create(ctx context.Context, item storedExecution, queued Event) (storedExecution, bool, error) {
	transaction, err := store.database.BeginTx(ctx, nil)
	if err != nil {
		return storedExecution{}, false, fmt.Errorf("execution.sqlite-begin:%w", err)
	}
	defer transaction.Rollback()

	var existingID []byte
	var existingAction, existingRoot string
	var existingHash []byte
	err = transaction.QueryRowContext(ctx,
		`SELECT execution_id, action_id, root, request_hash FROM executions WHERE principal=? AND idempotency_key=?`,
		item.Record.Principal, identifierBytes(item.IdempotencyKey.value),
	).Scan(&existingID, &existingAction, &existingRoot, &existingHash)
	if err == nil {
		if existingAction != string(item.Record.ActionID) || existingRoot != item.Record.Root || !bytes.Equal(existingHash, item.RequestHash[:]) {
			return storedExecution{}, false, fmt.Errorf("execution.idempotency-conflict")
		}
		if err := transaction.Commit(); err != nil {
			return storedExecution{}, false, fmt.Errorf("execution.sqlite-commit:%w", err)
		}
		id, parseErr := executionIDFromBytes(existingID)
		if parseErr != nil {
			return storedExecution{}, false, parseErr
		}
		existing, getErr := store.Get(ctx, id)
		return existing, true, getErr
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return storedExecution{}, false, fmt.Errorf("execution.sqlite-idempotency:%w", err)
	}

	if _, err := transaction.ExecContext(ctx, `INSERT INTO executions (
		execution_id, attempt_id, action_id, request_schema, request_json, request_redacted,
		request_hash, root, subject_kind, subject_project, subject_key, subject_relation,
		principal, origin, idempotency_key, status, executor_id, lease_expires_at, resumable, created_at
	) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)`,
		identifierBytes(item.Record.ExecutionID.value), identifierBytes(item.Record.AttemptID.value), string(item.Record.ActionID),
		item.Record.Request.Schema, nullableJSON(item.Record.Request), boolInteger(item.Record.Request.Redacted), item.RequestHash[:],
		item.Record.Root, nullableSubjectValue(item.Record.Subject, func(subject Subject) string { return subject.Kind }),
		nullableSubjectValue(item.Record.Subject, func(subject Subject) string { return subject.Project }),
		nullableSubjectValue(item.Record.Subject, func(subject Subject) string { return subject.Key }),
		nullableSubjectValue(item.Record.Subject, func(subject Subject) string { return subject.Relation }),
		item.Record.Principal, item.Record.Origin, identifierBytes(item.IdempotencyKey.value), item.Record.Status,
		nullableIdentifier(item.ExecutorID), nullableTime(item.LeaseExpiresAt), boolInteger(item.Resumable), unixMilliseconds(item.Record.CreatedAt),
	); err != nil {
		return storedExecution{}, false, fmt.Errorf("execution.sqlite-insert:%w", err)
	}
	if _, err := transaction.ExecContext(ctx,
		`INSERT INTO attempts(attempt_id, execution_id, ordinal, status) VALUES (?,?,1,?)`,
		identifierBytes(item.Record.AttemptID.value), identifierBytes(item.Record.ExecutionID.value), item.Record.Status,
	); err != nil {
		return storedExecution{}, false, fmt.Errorf("execution.sqlite-attempt:%w", err)
	}
	if err := insertEvent(ctx, transaction, queued); err != nil {
		return storedExecution{}, false, err
	}
	if err := transaction.Commit(); err != nil {
		return storedExecution{}, false, fmt.Errorf("execution.sqlite-commit:%w", err)
	}
	return item, false, nil
}

func (store *SQLiteStore) Get(ctx context.Context, id ExecutionID) (storedExecution, error) {
	row := store.database.QueryRowContext(ctx, `SELECT `+executionColumns+` FROM executions WHERE execution_id=?`, identifierBytes(id.value))
	item, err := scanStoredRow(row)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return storedExecution{}, fmt.Errorf("execution.not-found:%s", id)
		}
		return storedExecution{}, fmt.Errorf("execution.sqlite-get:%w", err)
	}
	if err := store.loadPendingPrompt(ctx, &item); err != nil {
		return storedExecution{}, err
	}
	return item, nil
}

func (store *SQLiteStore) List(ctx context.Context, principal PrincipalID, filter ListFilter) ([]storedExecution, error) {
	rows, err := store.database.QueryContext(ctx, `SELECT `+executionColumns+` FROM executions WHERE principal=? ORDER BY created_at DESC`, principal)
	if err != nil {
		return nil, fmt.Errorf("execution.sqlite-list:%w", err)
	}
	items := make([]storedExecution, 0)
	for rows.Next() {
		item, scanErr := scanStoredRows(rows)
		if scanErr != nil {
			rows.Close()
			return nil, fmt.Errorf("execution.sqlite-list:%w", scanErr)
		}
		if filter.Root != "" && item.Record.Root != filter.Root {
			continue
		}
		if !sqliteStatusIncluded(item.Record.Status, filter.Statuses) {
			continue
		}
		items = append(items, item)
		if filter.Limit != 0 && len(items) >= int(filter.Limit) {
			break
		}
	}
	if err := rows.Close(); err != nil {
		return nil, fmt.Errorf("execution.sqlite-list:%w", err)
	}
	for index := range items {
		if err := store.loadPendingPrompt(ctx, &items[index]); err != nil {
			return nil, err
		}
	}
	return items, nil
}

func (store *SQLiteStore) Commit(ctx context.Context, owner ExecutorID, item storedExecution, event *Event, prompt *promptUpdate) error {
	transaction, err := store.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("execution.sqlite-begin:%w", err)
	}
	defer transaction.Rollback()

	var resultSchema, resultRedacted sql.NullInt64
	var resultJSON, errorMessageJSON []byte
	var errorCode sql.NullString
	if item.Record.Result != nil {
		resultSchema = sql.NullInt64{Int64: int64(item.Record.Result.Schema), Valid: true}
		if !item.Record.Result.Redacted {
			resultJSON = append([]byte(nil), item.Record.Result.JSON...)
		}
		resultRedacted = sql.NullInt64{Int64: boolInteger(item.Record.Result.Redacted), Valid: true}
	}
	if item.Record.Failure != nil {
		messageJSON, marshalErr := json.Marshal(item.Record.Failure.Message)
		if marshalErr != nil {
			return fmt.Errorf("execution.sqlite-failure:%w", marshalErr)
		}
		errorCode = sql.NullString{String: string(item.Record.Failure.Code), Valid: true}
		errorMessageJSON = messageJSON
	}
	result, err := transaction.ExecContext(ctx, `UPDATE executions SET
		status=?, executor_id=?, lease_expires_at=?, started_at=?, finished_at=?, cancel_requested_at=?,
		result_schema=?, result_json=?, result_redacted=?, error_code=?, error_message_json=?
		WHERE execution_id=? AND executor_id=?`,
		item.Record.Status, nullableIdentifier(item.ExecutorID), nullableTime(item.LeaseExpiresAt), nullableTimePointer(item.Record.StartedAt),
		nullableTimePointer(item.Record.FinishedAt), nullableTimePointer(item.CancelRequestedAt), resultSchema, resultJSON,
		resultRedacted, errorCode, errorMessageJSON, identifierBytes(item.Record.ExecutionID.value), nullableIdentifier(owner),
	)
	if err != nil {
		return fmt.Errorf("execution.sqlite-update:%w", err)
	}
	updated, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("execution.sqlite-update:%w", err)
	}
	if updated != 1 {
		return fmt.Errorf("execution.lease-lost:%s", item.Record.ExecutionID)
	}
	if _, err := transaction.ExecContext(ctx, `UPDATE attempts SET status=?, started_at=?, finished_at=? WHERE attempt_id=?`,
		item.Record.Status, nullableTimePointer(item.Record.StartedAt), nullableTimePointer(item.Record.FinishedAt), identifierBytes(item.Record.AttemptID.value)); err != nil {
		return fmt.Errorf("execution.sqlite-attempt:%w", err)
	}
	if event != nil {
		if err := insertEvent(ctx, transaction, *event); err != nil {
			return err
		}
	}
	if prompt != nil {
		if err := mutatePrompt(ctx, transaction, item, *prompt); err != nil {
			return err
		}
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("execution.sqlite-commit:%w", err)
	}
	return nil
}

func (store *SQLiteStore) EventsAfter(ctx context.Context, id ExecutionID, after EventSequence, limit uint16) ([]Event, error) {
	if limit == 0 {
		limit = runtimeconfig.Default().Execution.EventFetchLimit
	}
	rows, err := store.database.QueryContext(ctx, `SELECT attempt_id, sequence, at, kind, action_id, message_json,
		payload_type, payload_schema, payload_json FROM events WHERE execution_id=? AND sequence>? ORDER BY sequence LIMIT ?`,
		identifierBytes(id.value), after, limit)
	if err != nil {
		return nil, fmt.Errorf("execution.sqlite-events:%w", err)
	}
	defer rows.Close()
	events := make([]Event, 0)
	for rows.Next() {
		var attemptBytes, messageJSON, payloadJSON []byte
		var sequence, at int64
		var kind, actionID string
		var payloadType sql.NullString
		var payloadSchema sql.NullInt64
		if err := rows.Scan(&attemptBytes, &sequence, &at, &kind, &actionID, &messageJSON, &payloadType, &payloadSchema, &payloadJSON); err != nil {
			return nil, fmt.Errorf("execution.sqlite-events:%w", err)
		}
		attemptID, err := attemptIDFromBytes(attemptBytes)
		if err != nil {
			return nil, err
		}
		var message MessageV1
		if err := json.Unmarshal(messageJSON, &message); err != nil || message.Schema != MessageSchemaV1 {
			return nil, fmt.Errorf("execution.sqlite-message")
		}
		eventSequence, err := eventSequenceFromInt64(sequence)
		if err != nil {
			return nil, err
		}
		event := Event{ExecutionID: id, AttemptID: attemptID, Sequence: eventSequence, At: timeFromMilliseconds(at), Kind: EventKind(kind), ActionID: action.ID(actionID), Message: message}
		if payloadType.Valid {
			if !payloadSchema.Valid {
				return nil, fmt.Errorf("execution.sqlite-payload-schema")
			}
			schema, err := schemaVersionFromInt64(payloadSchema.Int64)
			if err != nil {
				return nil, err
			}
			event.Payload = &EncodedEventData{Type: action.EventDataType(payloadType.String), Schema: schema, JSON: append(json.RawMessage(nil), payloadJSON...)}
		}
		events = append(events, event)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("execution.sqlite-events:%w", err)
	}
	return events, nil
}

func (store *SQLiteStore) Renew(ctx context.Context, executorID ExecutorID, ids []ExecutionID, expiresAt time.Time) error {
	transaction, err := store.database.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("execution.sqlite-begin:%w", err)
	}
	defer transaction.Rollback()
	for _, id := range ids {
		result, err := transaction.ExecContext(ctx, `UPDATE executions SET lease_expires_at=? WHERE execution_id=? AND executor_id=? AND status IN ('queued','running','waiting-input','canceling')`,
			unixMilliseconds(expiresAt), identifierBytes(id.value), identifierBytes(executorID.value))
		if err != nil {
			return fmt.Errorf("execution.sqlite-renew:%w", err)
		}
		updated, rowsErr := result.RowsAffected()
		if rowsErr != nil {
			return fmt.Errorf("execution.sqlite-renew:%w", rowsErr)
		}
		if updated != 1 {
			return fmt.Errorf("execution.lease-lost:%s", id)
		}
	}
	if err := transaction.Commit(); err != nil {
		return fmt.Errorf("execution.sqlite-commit:%w", err)
	}
	return nil
}

func (store *SQLiteStore) Recover(ctx context.Context, executorID ExecutorID, now time.Time, leaseDuration time.Duration) ([]storedExecution, error) {
	transaction, err := store.database.BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("execution.sqlite-begin:%w", err)
	}
	defer transaction.Rollback()
	rows, err := transaction.QueryContext(ctx, `SELECT execution_id, status, resumable FROM executions
		WHERE status IN ('queued','running','waiting-input','canceling') AND (lease_expires_at IS NULL OR lease_expires_at<=?) ORDER BY created_at`, unixMilliseconds(now))
	if err != nil {
		return nil, fmt.Errorf("execution.sqlite-recover:%w", err)
	}
	type recoveryCandidate struct {
		id        ExecutionID
		status    Status
		resumable bool
	}
	candidates := make([]recoveryCandidate, 0)
	for rows.Next() {
		var idBytes []byte
		var status string
		var resumable int64
		if err := rows.Scan(&idBytes, &status, &resumable); err != nil {
			rows.Close()
			return nil, fmt.Errorf("execution.sqlite-recover:%w", err)
		}
		id, err := executionIDFromBytes(idBytes)
		if err != nil {
			rows.Close()
			return nil, err
		}
		candidates = append(candidates, recoveryCandidate{id: id, status: Status(status), resumable: resumable == 1})
	}
	if err := rows.Close(); err != nil {
		return nil, fmt.Errorf("execution.sqlite-recover:%w", err)
	}
	claimed := make([]ExecutionID, 0)
	lease := now.Add(leaseDuration)
	for _, candidate := range candidates {
		if candidate.status == StatusQueued && candidate.resumable {
			result, err := transaction.ExecContext(ctx, `UPDATE executions SET executor_id=?, lease_expires_at=?
				WHERE execution_id=? AND status='queued' AND resumable=1 AND (lease_expires_at IS NULL OR lease_expires_at<=?)`,
				identifierBytes(executorID.value), unixMilliseconds(lease), identifierBytes(candidate.id.value), unixMilliseconds(now))
			if err != nil {
				return nil, fmt.Errorf("execution.sqlite-claim:%w", err)
			}
			count, err := result.RowsAffected()
			if err != nil {
				return nil, fmt.Errorf("execution.sqlite-claim:%w", err)
			}
			if count == 1 {
				claimed = append(claimed, candidate.id)
			}
			continue
		}
		if err := interruptRestart(ctx, transaction, candidate.id, now); err != nil {
			return nil, err
		}
	}
	if err := transaction.Commit(); err != nil {
		return nil, fmt.Errorf("execution.sqlite-commit:%w", err)
	}
	items := make([]storedExecution, 0, len(claimed))
	for _, id := range claimed {
		item, err := store.Get(ctx, id)
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return items, nil
}

func (store *SQLiteStore) Prune(ctx context.Context, root string, limit uint16) error {
	if limit == 0 {
		return nil
	}
	_, err := store.database.ExecContext(ctx, `DELETE FROM executions WHERE execution_id IN (
		SELECT execution_id FROM executions WHERE root=? AND status IN ('canceled','succeeded','failed','interrupted')
		ORDER BY COALESCE(finished_at, created_at) DESC LIMIT -1 OFFSET ?
	)`, root, limit)
	if err != nil {
		return fmt.Errorf("execution.sqlite-prune:%w", err)
	}
	return nil
}

func (store *SQLiteStore) Close() error {
	return store.database.Close()
}

func (store *SQLiteStore) loadPendingPrompt(ctx context.Context, item *storedExecution) error {
	var promptID, kind string
	var schema int64
	var promptJSON []byte
	if item.Record.Status != StatusWaitingInput {
		item.Record.PendingPrompt = nil
		return nil
	}
	err := store.database.QueryRowContext(ctx, `SELECT prompt_id, kind, schema, prompt_json FROM prompts WHERE execution_id=? AND status='pending' ORDER BY created_at DESC LIMIT 1`,
		identifierBytes(item.Record.ExecutionID.value)).Scan(&promptID, &kind, &schema, &promptJSON)
	if errors.Is(err, sql.ErrNoRows) {
		return nil
	}
	if err != nil {
		return fmt.Errorf("execution.sqlite-prompt:%w", err)
	}
	promptSchema, err := schemaVersionFromInt64(schema)
	if err != nil {
		return err
	}
	item.Record.PendingPrompt = &EncodedPrompt{ID: action.PromptID(promptID), Kind: action.PromptKind(kind), Schema: promptSchema, JSON: append(json.RawMessage(nil), promptJSON...), Redacted: kind == string(action.PromptSecret)}
	return nil
}

func scanStoredRow(row *sql.Row) (storedExecution, error) {
	var value executionColumnsValue
	err := row.Scan(&value.executionID, &value.attemptID, &value.actionID, &value.requestSchema, &value.requestJSON,
		&value.requestRedacted, &value.requestHash, &value.root, &value.subjectKind, &value.subjectProject,
		&value.subjectKey, &value.subjectRelation, &value.principal, &value.origin, &value.idempotencyKey,
		&value.status, &value.executorID, &value.leaseExpiresAt, &value.resumable, &value.createdAt, &value.startedAt,
		&value.finishedAt, &value.cancelRequestedAt, &value.resultSchema, &value.resultJSON, &value.resultRedacted,
		&value.errorCode, &value.errorMessageJSON, &value.lastSequence)
	if err != nil {
		return storedExecution{}, err
	}
	return decodeStored(value)
}

func scanStoredRows(rows *sql.Rows) (storedExecution, error) {
	var value executionColumnsValue
	err := rows.Scan(&value.executionID, &value.attemptID, &value.actionID, &value.requestSchema, &value.requestJSON,
		&value.requestRedacted, &value.requestHash, &value.root, &value.subjectKind, &value.subjectProject,
		&value.subjectKey, &value.subjectRelation, &value.principal, &value.origin, &value.idempotencyKey,
		&value.status, &value.executorID, &value.leaseExpiresAt, &value.resumable, &value.createdAt, &value.startedAt,
		&value.finishedAt, &value.cancelRequestedAt, &value.resultSchema, &value.resultJSON, &value.resultRedacted,
		&value.errorCode, &value.errorMessageJSON, &value.lastSequence)
	if err != nil {
		return storedExecution{}, err
	}
	return decodeStored(value)
}

func decodeStored(value executionColumnsValue) (storedExecution, error) {
	executionID, err := executionIDFromBytes(value.executionID)
	if err != nil {
		return storedExecution{}, err
	}
	attemptID, err := attemptIDFromBytes(value.attemptID)
	if err != nil {
		return storedExecution{}, err
	}
	idempotencyKey, err := idempotencyKeyFromBytes(value.idempotencyKey)
	if err != nil {
		return storedExecution{}, err
	}
	executorID, err := executorIDFromNullableBytes(value.executorID)
	if err != nil {
		return storedExecution{}, err
	}
	if len(value.requestHash) != 32 {
		return storedExecution{}, fmt.Errorf("execution.sqlite-request-hash")
	}
	var requestHash [32]byte
	copy(requestHash[:], value.requestHash)
	requestSchema, err := schemaVersionFromInt64(value.requestSchema)
	if err != nil {
		return storedExecution{}, err
	}
	lastSequence, err := eventSequenceFromInt64(value.lastSequence)
	if err != nil {
		return storedExecution{}, err
	}
	record := Record{
		ExecutionID: executionID, AttemptID: attemptID, ActionID: action.ID(value.actionID), Status: Status(value.status), Root: value.root,
		Principal: PrincipalID(value.principal), Origin: Origin(value.origin),
		Request:   Encoded{Schema: requestSchema, JSON: append(json.RawMessage(nil), value.requestJSON...), Redacted: value.requestRedacted == 1},
		CreatedAt: timeFromMilliseconds(value.createdAt), StartedAt: timePointer(value.startedAt), FinishedAt: timePointer(value.finishedAt),
	}
	if value.subjectKind.Valid {
		record.Subject = &Subject{
			Kind: value.subjectKind.String, Project: value.subjectProject.String,
			Key: value.subjectKey.String, Relation: value.subjectRelation.String,
		}
		if !record.Subject.Valid() {
			return storedExecution{}, fmt.Errorf("execution.sqlite-subject")
		}
	}
	if value.resultSchema.Valid {
		resultSchema, err := schemaVersionFromInt64(value.resultSchema.Int64)
		if err != nil {
			return storedExecution{}, err
		}
		record.Result = &Encoded{Schema: resultSchema, JSON: append(json.RawMessage(nil), value.resultJSON...), Redacted: value.resultRedacted.Int64 == 1}
	}
	if value.errorCode.Valid {
		var message MessageV1
		if err := json.Unmarshal(value.errorMessageJSON, &message); err != nil || message.Schema != MessageSchemaV1 {
			return storedExecution{}, fmt.Errorf("execution.sqlite-failure")
		}
		record.Failure = &Failure{Code: ErrorCode(value.errorCode.String), Message: message}
	}
	return storedExecution{
		Record: record, RequestHash: requestHash, IdempotencyKey: idempotencyKey, ExecutorID: executorID,
		LeaseExpiresAt: timeValue(value.leaseExpiresAt), Resumable: value.resumable == 1,
		LastSequence: lastSequence, CancelRequestedAt: timePointer(value.cancelRequestedAt),
	}, nil
}

func schemaVersionFromInt64(value int64) (SchemaVersion, error) {
	if value < 0 || value > int64(^uint16(0)) {
		return 0, fmt.Errorf("execution.sqlite-schema-out-of-range:%d", value)
	}
	return SchemaVersion(value), nil
}

func eventSequenceFromInt64(value int64) (EventSequence, error) {
	if value < 0 {
		return 0, fmt.Errorf("execution.sqlite-sequence-out-of-range:%d", value)
	}
	return EventSequence(value), nil
}

func insertEvent(ctx context.Context, transaction *sql.Tx, event Event) error {
	messageJSON, err := json.Marshal(event.Message)
	if err != nil {
		return fmt.Errorf("execution.sqlite-message:%w", err)
	}
	var payloadType sql.NullString
	var payloadSchema sql.NullInt64
	var payloadJSON []byte
	if event.Payload != nil {
		payloadType = sql.NullString{String: string(event.Payload.Type), Valid: true}
		payloadSchema = sql.NullInt64{Int64: int64(event.Payload.Schema), Valid: true}
		payloadJSON = append([]byte(nil), event.Payload.JSON...)
	}
	_, err = transaction.ExecContext(ctx, `INSERT INTO events(execution_id, attempt_id, sequence, at, kind, action_id, message_json, payload_type, payload_schema, payload_json)
		VALUES (?,?,?,?,?,?,?,?,?,?)`, identifierBytes(event.ExecutionID.value), identifierBytes(event.AttemptID.value), event.Sequence,
		unixMilliseconds(event.At), event.Kind, event.ActionID, messageJSON, payloadType, payloadSchema, payloadJSON)
	if err != nil {
		return fmt.Errorf("execution.sqlite-event:%w", err)
	}
	return nil
}

func mutatePrompt(ctx context.Context, transaction *sql.Tx, item storedExecution, update promptUpdate) error {
	if update.Prompt != nil {
		promptJSON := []byte(nil)
		if !update.Prompt.Redacted {
			promptJSON = update.Prompt.JSON
		}
		_, err := transaction.ExecContext(ctx, `INSERT INTO prompts(execution_id, attempt_id, prompt_id, kind, schema, prompt_json, status, created_at, response_redacted)
			VALUES (?,?,?,?,?,?,?,?,?)`, identifierBytes(item.Record.ExecutionID.value), identifierBytes(item.Record.AttemptID.value), update.Prompt.ID,
			update.Prompt.Kind, update.Prompt.Schema, promptJSON, update.PromptStatus, unixMilliseconds(time.Now().UTC()), boolInteger(update.Prompt.Redacted))
		if err != nil {
			return fmt.Errorf("execution.sqlite-prompt:%w", err)
		}
		return nil
	}
	if update.PromptStatus == "discarded" {
		result, err := transaction.ExecContext(ctx, `DELETE FROM prompts WHERE execution_id=? AND prompt_id=? AND status='pending'`,
			identifierBytes(item.Record.ExecutionID.value), update.PromptID)
		if err != nil {
			return fmt.Errorf("execution.sqlite-prompt:%w", err)
		}
		count, err := result.RowsAffected()
		if err != nil || count != 1 {
			return fmt.Errorf("execution.prompt-not-pending:%s", update.PromptID)
		}
		return nil
	}
	responseJSON := []byte(nil)
	if !update.Redacted {
		responseJSON = update.ResponseJSON
	}
	result, err := transaction.ExecContext(ctx, `UPDATE prompts SET status=?, responded_at=?, response_json=?, response_redacted=? WHERE execution_id=? AND prompt_id=? AND status='pending'`,
		update.PromptStatus, nullableTimePointer(update.RespondedAt), responseJSON, boolInteger(update.Redacted), identifierBytes(item.Record.ExecutionID.value), update.PromptID)
	if err != nil {
		return fmt.Errorf("execution.sqlite-prompt:%w", err)
	}
	count, err := result.RowsAffected()
	if err != nil || count != 1 {
		return fmt.Errorf("execution.prompt-not-pending:%s", update.PromptID)
	}
	return nil
}

func interruptRestart(ctx context.Context, transaction *sql.Tx, id ExecutionID, now time.Time) error {
	var attemptBytes []byte
	var actionID string
	var sequence int64
	err := transaction.QueryRowContext(ctx, `SELECT attempt_id, action_id, (SELECT COALESCE(MAX(sequence),0) FROM events WHERE execution_id=executions.execution_id)
		FROM executions WHERE execution_id=?`, identifierBytes(id.value)).Scan(&attemptBytes, &actionID, &sequence)
	if err != nil {
		return fmt.Errorf("execution.sqlite-recover:%w", err)
	}
	result, err := transaction.ExecContext(ctx, `UPDATE executions SET status='interrupted', finished_at=?, executor_id=NULL, lease_expires_at=NULL
		WHERE execution_id=? AND status IN ('queued','running','waiting-input','canceling') AND (lease_expires_at IS NULL OR lease_expires_at<=?)`,
		unixMilliseconds(now), identifierBytes(id.value), unixMilliseconds(now))
	if err != nil {
		return fmt.Errorf("execution.sqlite-interrupt:%w", err)
	}
	count, err := result.RowsAffected()
	if err != nil || count == 0 {
		return nil
	}
	attemptID, err := attemptIDFromBytes(attemptBytes)
	if err != nil {
		return err
	}
	message, err := EncodeMessage(l10n.M("execution.interrupted-restart"))
	if err != nil {
		return err
	}
	if sequence == int64(^uint64(0)>>1) {
		return fmt.Errorf("execution.sqlite-sequence-exhausted")
	}
	eventSequence, err := eventSequenceFromInt64(sequence + 1)
	if err != nil {
		return err
	}
	event := Event{ExecutionID: id, AttemptID: attemptID, Sequence: eventSequence, At: now, Kind: EventInterrupted, ActionID: action.ID(actionID), Message: message}
	if err := insertEvent(ctx, transaction, event); err != nil {
		return err
	}
	if _, err = transaction.ExecContext(ctx, `DELETE FROM prompts WHERE execution_id=? AND status='pending'`, identifierBytes(id.value)); err != nil {
		return fmt.Errorf("execution.sqlite-prompt:%w", err)
	}
	_, err = transaction.ExecContext(ctx, `UPDATE attempts SET status='interrupted', finished_at=? WHERE attempt_id=?`, unixMilliseconds(now), attemptBytes)
	if err != nil {
		return fmt.Errorf("execution.sqlite-attempt:%w", err)
	}
	return nil
}

func sqliteStatusIncluded(status Status, statuses []Status) bool {
	if len(statuses) == 0 {
		return true
	}
	for _, candidate := range statuses {
		if candidate == status {
			return true
		}
	}
	return false
}

func identifierBytes(value [identifierSize]byte) []byte {
	result := make([]byte, identifierSize)
	copy(result, value[:])
	return result
}

func executionIDFromBytes(value []byte) (ExecutionID, error) {
	if len(value) != identifierSize {
		return ExecutionID{}, fmt.Errorf("execution.sqlite-execution-id")
	}
	var raw [identifierSize]byte
	copy(raw[:], value)
	return ExecutionID{value: raw}, nil
}

func attemptIDFromBytes(value []byte) (AttemptID, error) {
	if len(value) != identifierSize {
		return AttemptID{}, fmt.Errorf("execution.sqlite-attempt-id")
	}
	var raw [identifierSize]byte
	copy(raw[:], value)
	return AttemptID{value: raw}, nil
}

func idempotencyKeyFromBytes(value []byte) (IdempotencyKey, error) {
	if len(value) != identifierSize {
		return IdempotencyKey{}, fmt.Errorf("execution.sqlite-idempotency-key")
	}
	var raw [identifierSize]byte
	copy(raw[:], value)
	return IdempotencyKey{value: raw}, nil
}

func executorIDFromNullableBytes(value []byte) (ExecutorID, error) {
	if len(value) == 0 {
		return ExecutorID{}, nil
	}
	if len(value) != identifierSize {
		return ExecutorID{}, fmt.Errorf("execution.sqlite-executor-id")
	}
	var raw [identifierSize]byte
	copy(raw[:], value)
	return ExecutorID{value: raw}, nil
}

func nullableIdentifier(id ExecutorID) []byte {
	if id.IsZero() {
		return nil
	}
	return identifierBytes(id.value)
}

func nullableSubjectValue(subject *Subject, selectValue func(Subject) string) any {
	if subject == nil {
		return nil
	}
	return selectValue(*subject)
}

func unixMilliseconds(value time.Time) int64 {
	return value.UTC().UnixMilli()
}

func nullableTime(value time.Time) *int64 {
	if value.IsZero() {
		return nil
	}
	milliseconds := unixMilliseconds(value)
	return &milliseconds
}

func nullableTimePointer(value *time.Time) *int64 {
	if value == nil {
		return nil
	}
	milliseconds := unixMilliseconds(*value)
	return &milliseconds
}

func timeFromMilliseconds(value int64) time.Time {
	return time.UnixMilli(value).UTC()
}

func timePointer(value sql.NullInt64) *time.Time {
	if !value.Valid {
		return nil
	}
	result := timeFromMilliseconds(value.Int64)
	return &result
}

func timeValue(value sql.NullInt64) time.Time {
	if !value.Valid {
		return time.Time{}
	}
	return timeFromMilliseconds(value.Int64)
}

func nullableJSON(value Encoded) []byte {
	if value.Redacted {
		return nil
	}
	return value.JSON
}

func boolInteger(value bool) int64 {
	if value {
		return 1
	}
	return 0
}
