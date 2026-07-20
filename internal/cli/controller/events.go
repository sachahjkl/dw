package controller

import (
	"context"
	"fmt"
	"sync"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
)

// EventSink routes human diagnostics to stderr and never contaminates machine stdout.
type EventSink struct {
	engine    console.Engine
	context   console.RenderContext
	verbosity uint8
	mu        sync.Mutex
	last      uint64
}

func NewEventSink(engine console.Engine, policy console.Policy, localizer l10n.Localizer, verbosity uint8) *EventSink {
	return &EventSink{engine: engine, context: console.NewRenderContext(policy, localizer), verbosity: verbosity}
}

func (sink *EventSink) Emit(ctx context.Context, event action.EventEnvelope) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if sink.context.Policy.Machine || (!sink.context.Policy.Streams.StderrTTY && sink.verbosity == 0) {
		return nil
	}
	if event.Kind == action.EventLog && sink.verbosity == 0 {
		return nil
	}
	sink.mu.Lock()
	defer sink.mu.Unlock()
	if event.Sequence != 0 && event.Sequence <= sink.last {
		return fmt.Errorf("cli.event-out-of-order:%s:%d", event.Action, event.Sequence)
	}
	if event.Sequence != 0 {
		sink.last = event.Sequence
	}
	return sink.engine.WriteEvent(sink.context, event)
}
