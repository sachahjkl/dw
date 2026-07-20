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
	if err := RegisterGrammarAliases(results); err != nil {
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

func RegisterGrammarAliases(results *Registry) error {
	aliases := [][2]ResultKind{
		{"auth.login", "ado.auth.login"}, {"auth.status", "ado.auth.status"}, {"auth.logout", "ado.auth.logout"},
		{"ado.item.show", "ado.workitem"}, {"ado.context.show", "ado.context"}, {"ado.context.ai", "ado.ai.context"},
		{"upgrade", "upgrade.run"}, {"work.start", "task.start"}, {"work.pr.start", "task.start.pr"},
		{"agent.open", "task.open"}, {"work.open", "task.open"}, {"work.sync", "task.sync"}, {"work.task.child.create", "task.child.create"},
		{"work.prune", "task.prune"}, {"work.finish", "task.finish"},
	}
	for _, alias := range aliases {
		if err := results.Alias(alias[0], alias[1]); err != nil {
			return err
		}
	}
	return results.Union("work.item.doing", "task.doing.plan", "task.doing.execute")
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
