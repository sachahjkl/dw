package console

import (
	"errors"
	"strings"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/data"
)

type Engine struct {
	Results *Registry
	Events  *EventRegistry
}

func NewEngine(results *Registry, events *EventRegistry) Engine {
	if results == nil {
		results = NewRegistry()
	}
	if events == nil {
		events = NewEventRegistry()
	}
	return Engine{Results: results, Events: events}
}

// RenderResult requires an explicit ordered projection for JSON. It never
// serializes a domain result or derives machine output from human text.
func (e Engine) RenderResult(context RenderContext, envelope action.ResultEnvelope, format OutputFormat, projection *JSONProjection) (Output, error) {
	return e.RenderResultKind(context, envelope, envelope.Action, format, projection)
}

func (e Engine) RenderResultKind(context RenderContext, envelope action.ResultEnvelope, kind ResultKind, format OutputFormat, projection *JSONProjection) (Output, error) {
	if format == FormatJSON {
		if projection == nil {
			return Output{}, errors.New("console.json-projection-required")
		}
		return RenderJSON(*projection)
	}
	context.Policy.Machine = false
	return e.Results.Render(context, kind, envelope.Result)
}

// RenderEvent formats registered event DTOs. JSON commands suppress all event
// text so stdout remains a single valid document and stderr remains quiet.
func (e Engine) RenderEvent(context RenderContext, envelope action.EventEnvelope) (string, bool, error) {
	if !context.Policy.EventsEnabled() {
		return "", false, nil
	}
	event, line, err := e.Events.FormatEnvelope(envelope)
	if err != nil {
		var missing RendererNotFoundError
		if !errors.As(err, &missing) {
			return "", false, err
		}
		if envelope.Message.ID == "" {
			return "", false, nil
		}
		return context.Localizer.Render(envelope.Message), false, nil
	}
	if envelope.Message.ID != "" {
		line = AppendEventFields(context.Localizer.Render(envelope.Message), event)
	}
	return line, event.Transient, nil
}

func (e Engine) WriteEvent(context RenderContext, envelope action.EventEnvelope) error {
	line, _, err := e.RenderEvent(context, envelope)
	if err != nil || line == "" {
		return err
	}
	return WriteDiagnostic(context.Policy.Streams.Stderr, context.EventTheme.Muted(line))
}

func RegisterGuideRenderer[T any](registry *Registry, kind ResultKind, project func(T) GuideResult) error {
	return RegisterResult(registry, kind, func(context RenderContext, value T) (Output, error) {
		return TextOutput(FormatHuman, RenderGuide(project(value), context.Localizer, context.Theme)), nil
	})
}

func RegisterQueryRenderer[T any](registry *Registry, kind ResultKind, project func(T) data.Table) error {
	return RegisterResult(registry, kind, func(context RenderContext, value T) (Output, error) {
		return RenderQuery(project(value), context.Policy, context.Localizer, context.Theme), nil
	})
}

func RegisterChangelogRenderer[T any](registry *Registry, kind ResultKind, project func(T) ChangelogReport) error {
	return RegisterResult(registry, kind, func(context RenderContext, value T) (Output, error) {
		report := project(value)
		format := FormatRaw
		switch report.Format {
		case ChangelogMarkdown:
			format = FormatMarkdown
		case ChangelogHTML:
			format = FormatHTML
		}
		return TextOutput(format, RenderChangelog(report, context.Localizer)), nil
	})
}

type DocumentResult struct {
	Format OutputFormat
	Text   string
}

func RegisterDocumentRenderer[T any](registry *Registry, kind ResultKind, project func(T) DocumentResult) error {
	return RegisterResult(registry, kind, func(_ RenderContext, value T) (Output, error) {
		document := project(value)
		return TextOutput(document.Format, document.Text), nil
	})
}

// Lines exposes the same preprojected human text to TUI adapters without
// coupling TUI state to provider or console DTOs.
func Lines(output Output) []string {
	if len(output.Body) == 0 {
		return nil
	}
	return strings.Split(strings.TrimSuffix(string(output.Body), "\n"), "\n")
}
