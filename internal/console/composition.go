package console

import "github.com/sachahjkl/dw/internal/action"

// Registration is the static composition seam for one action. Bootstrap owns
// concrete DTO imports; console owns rendering and completeness validation.
type Registration struct {
	Action action.ID
	Result ResultRenderer
	Event  EventProjector
}

// RegisterAll installs built-in renderers, then application registrations, and
// rejects incomplete result coverage before command execution begins.
func RegisterAll(results *Registry, events *EventRegistry, registrations ...Registration) error {
	if results == nil || events == nil {
		return &MissingRenderersError{Kinds: append([]ResultKind(nil), RequiredResultKinds...)}
	}
	if err := RegisterCoreRenderers(results); err != nil {
		return err
	}
	if err := RegisterUpdateRenderers(results, events); err != nil {
		return err
	}
	for _, registration := range registrations {
		if registration.Result != nil {
			if err := results.Register(registration.Action, registration.Result); err != nil {
				return err
			}
		}
		if registration.Event != nil {
			if err := events.Register(registration.Action, registration.Event); err != nil {
				return err
			}
		}
	}
	return results.ValidateComplete(RequiredResultKinds)
}

func PageRenderer[T any](project func(T) Page) ResultRenderer {
	return func(context RenderContext, payload any) (Output, error) {
		value, ok := payload.(T)
		if !ok {
			return Output{}, PayloadTypeError{}
		}
		return TextOutput(FormatHuman, RenderPage(project(value), context.Localizer, context.Theme)), nil
	}
}

func EventRenderer[T any](project func(T) EventProjection) EventProjector {
	return func(payload any) (EventProjection, error) {
		value, ok := payload.(T)
		if !ok {
			return EventProjection{}, PayloadTypeError{}
		}
		return project(value), nil
	}
}
