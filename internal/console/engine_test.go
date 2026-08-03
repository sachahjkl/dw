package console

import (
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/l10n"
)

type engineEventData struct{}

func (engineEventData) EventDataType() action.EventDataType { return "test.event" }
func (engineEventData) EventDataSchema() uint16             { return 1 }

func TestRenderEventFallsBackForUnknownPersistedMessage(t *testing.T) {
	events := NewEventRegistry()
	if err := RegisterEvent(events, action.ID("test.run"), func(engineEventData) EventProjection {
		return EventProjection{ActionID: "test.run"}
	}); err != nil {
		t.Fatal(err)
	}
	engine := NewEngine(NewRegistry(), events)
	context := NewRenderContext(Policy{ShowEvents: true}, NewEnglishLocalizer())
	line, _, err := engine.RenderEvent(context, action.EventEnvelope{
		Action: "test.run", Data: engineEventData{}, Message: l10n.M("plugin.removed-message"),
	})
	if err != nil {
		t.Fatal(err)
	}
	if line != "Plugin Removed Message" {
		t.Fatalf("unknown persisted message = %q", line)
	}
}

func TestRenderEventLocalizesKnownPersistedMessage(t *testing.T) {
	engine := NewEngine(NewRegistry(), NewEventRegistry())
	context := NewRenderContext(Policy{ShowEvents: true}, NewEnglishLocalizer())
	line, _, err := engine.RenderEvent(context, action.EventEnvelope{
		Action: "workspace.start", Message: l10n.M("work.event.planning-start"),
	})
	if err != nil {
		t.Fatal(err)
	}
	if line != "Planning workspace start..." {
		t.Fatalf("known persisted message = %q", line)
	}
}

func TestWorkLifecycleEventMessagesAreComplete(t *testing.T) {
	localizer := NewEnglishLocalizer()
	for _, id := range []l10n.ID{
		"work.event.planning-start",
		"work.event.loading-start-work-items",
		"work.event.building-start-plan",
		"work.event.executing-start",
	} {
		if !localizer.Has(id) {
			t.Errorf("missing work lifecycle event message %q", id)
		}
	}
}
