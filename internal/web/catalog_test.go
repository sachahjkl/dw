package web

import (
	"context"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/l10n"
)

type catalogRequest struct{ id action.ID }

func (request catalogRequest) ActionID() action.ID { return request.id }

func TestCommandCatalogueIncludesEveryWebLeaf(t *testing.T) {
	localizer, err := l10n.NewEnglish().Extend(spec.EnglishEntries()...)
	if err != nil {
		t.Fatal(err)
	}
	grammar := spec.Root(localizer)
	routes := controller.NewRegistry()
	leaves := catalogueLeaves(grammar)
	for _, command := range leaves {
		key := command.Key
		excluded := strings.HasPrefix(key, "completion.") || key == "tui" || strings.HasPrefix(key, "web.")
		route := controller.Route{Key: key}
		if excluded {
			route.Direct = func(_ context.Context, _ controller.Execution, _ *parse.Result) (controller.Outcome, error) {
				return controller.Outcome{}, nil
			}
		} else {
			route.Build = func(_ *parse.Result) (action.Request, error) { return catalogRequest{id: action.ID(key)}, nil }
			route.Project = func(action.ResultEnvelope, *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
				return console.FormatHuman, nil, nil
			}
		}
		if err := routes.Register(route); err != nil {
			t.Fatal(err)
		}
	}
	server := &Server{deps: Dependencies{Grammar: grammar, Routes: routes}}
	catalogue := server.commandCatalog("csrf")
	byKey := make(map[string]commandView, len(catalogue))
	for _, command := range catalogue {
		byKey[command.Key] = command
	}
	for _, leaf := range leaves {
		excluded := strings.HasPrefix(leaf.Key, "completion.") || leaf.Key == "tui" || strings.HasPrefix(leaf.Key, "web.")
		command, found := byKey[leaf.Key]
		if excluded && found {
			t.Errorf("excluded leaf %q is present", leaf.Key)
		}
		if !excluded && !found {
			t.Errorf("web leaf %q is absent", leaf.Key)
		}
		if found && command.CLI == "" {
			t.Errorf("leaf %q has no exact CLI command", leaf.Key)
		}
	}
	for _, key := range []string{"agent.open", "workspace.open", "workspace.start"} {
		if command, found := byKey[key]; !found || !command.Disabled || command.DisabledReason == "" {
			t.Errorf("external command %q is not disabled with a reason", key)
		}
	}
}

func catalogueLeaves(root *spec.Command) []*spec.Command {
	var leaves []*spec.Command
	var visit func(*spec.Command)
	visit = func(command *spec.Command) {
		if len(command.Children) == 0 {
			leaves = append(leaves, command)
			return
		}
		for _, child := range command.Children {
			visit(child)
		}
	}
	for _, child := range root.Children {
		visit(child)
	}
	return leaves
}
