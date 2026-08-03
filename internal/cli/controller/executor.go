package controller

import (
	"context"
	"errors"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/l10n"
)

func executeRequest(ctx context.Context, adapter Execution, invocation *parse.Result, request action.Request) (action.ResultEnvelope, error) {
	key, err := execution.NewIdempotencyKey()
	if err != nil {
		return action.ResultEnvelope{}, err
	}
	root := resolvedRoot(invocation.Values)
	id, err := adapter.Executor.Submit(ctx, execution.Submission{Request: request, Root: root, Actor: adapter.Actor, IdempotencyKey: key})
	if err != nil {
		return action.ResultEnvelope{}, err
	}
	record, streamErr := streamExecution(ctx, adapter, invocation, id)
	envelope := action.ResultEnvelope{Action: record.ActionID, Result: record.TypedResult}
	if streamErr != nil {
		return envelope, streamErr
	}
	if record.Status == execution.StatusCanceled {
		return envelope, context.Canceled
	}
	if record.Failure != nil {
		return envelope, execution.NewFailureError(*record.Failure)
	}
	if record.Status != execution.StatusSucceeded {
		return envelope, fmt.Errorf("execution.unexpected-terminal-status:%s", record.Status)
	}
	return envelope, nil
}

func streamExecution(ctx context.Context, adapter Execution, invocation *parse.Result, id execution.ExecutionID) (execution.Record, error) {
	lastSequence := execution.EventSequence(0)
	eventSink := NewEventSink(adapter.Console, adapter.Policy, adapter.Localizer, invocation.Verbosity)
	input := NewTerminalInput(adapter.Policy.Streams, adapter.Localizer)
	for {
		subscription, err := adapter.Executor.Subscribe(ctx, adapter.Actor, id, lastSequence)
		if err != nil {
			if ctx.Err() != nil {
				cancelExecution(adapter, id)
			}
			return execution.Record{}, err
		}
		for event := range subscription.Events {
			if event.Sequence > lastSequence {
				lastSequence = event.Sequence
			}
			if err := handleExecutionEvent(ctx, adapter, id, event, eventSink, input); err != nil {
				cancelExecution(adapter, id)
				return waitAfterCancel(adapter, id, err)
			}
		}
		var streamErr error
		for deliveredErr := range subscription.Errors {
			if deliveredErr != nil {
				streamErr = deliveredErr
				break
			}
		}
		if streamErr != nil && streamErr.Error() == "execution.slow-subscriber" {
			continue
		}
		if streamErr != nil {
			return execution.Record{}, streamErr
		}
		break
	}
	if ctx.Err() != nil {
		cancelExecution(adapter, id)
		return waitAfterCancel(adapter, id, ctx.Err())
	}
	return adapter.Executor.Wait(ctx, adapter.Actor, id)
}

func handleExecutionEvent(ctx context.Context, adapter Execution, id execution.ExecutionID, event execution.Event, sink *EventSink, input *TerminalInput) error {
	switch event.Kind {
	case execution.EventProgress, execution.EventWarning, execution.EventLog:
		message, err := execution.DecodeMessage(event.Message)
		if err != nil {
			return err
		}
		kind := action.EventProgress
		if event.Kind == execution.EventWarning {
			kind = action.EventWarning
		} else if event.Kind == execution.EventLog {
			kind = action.EventLog
		}
		return sink.Emit(ctx, action.EventEnvelope{Action: event.ActionID, Kind: kind, Message: message, Data: event.TypedData})
	case execution.EventInputRequired:
		record, err := adapter.Executor.Get(ctx, adapter.Actor, id)
		if err != nil {
			return err
		}
		if record.PendingPrompt == nil {
			return nil
		}
		prompt, err := execution.DecodePrompt(*record.PendingPrompt)
		if err != nil {
			return err
		}
		if adapter.Policy.Machine || !adapter.Policy.Interactive() {
			if prompt.PromptKind() == action.PromptConfirm {
				return l10n.NewError("cli.error.confirmation-required")
			}
			return l10n.NewError("cli.error.input-requires-terminal", l10n.A("prompt", prompt.PromptID()))
		}
		response, err := input.Request(ctx, prompt)
		if err != nil {
			return err
		}
		return adapter.Executor.Respond(ctx, adapter.Actor, id, prompt.PromptID(), response)
	}
	return nil
}

func cancelExecution(adapter Execution, id execution.ExecutionID) {
	_ = adapter.Executor.Cancel(context.Background(), adapter.Actor, id)
}

func waitAfterCancel(adapter Execution, id execution.ExecutionID, cause error) (execution.Record, error) {
	record, err := adapter.Executor.Wait(context.Background(), adapter.Actor, id)
	if err != nil && !errors.Is(err, context.Canceled) {
		return execution.Record{}, err
	}
	return record, cause
}
