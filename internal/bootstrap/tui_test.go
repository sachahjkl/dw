package bootstrap

import (
	"context"
	"testing"

	"github.com/sachahjkl/dw/internal/action"
	"github.com/sachahjkl/dw/internal/cli/controller"
	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/console"
	"github.com/sachahjkl/dw/internal/tui"
)

func TestTUIRequestBuilderBuildsValidFormRequest(t *testing.T) {
	routes := controller.NewRegistry()
	if err := routes.Register(controller.Route{
		Key: "work.item.list",
		Build: func(*parse.Result) (action.Request, error) {
			return guideRequest{}, nil
		},
		Project: func(action.ResultEnvelope, *parse.Result) (console.OutputFormat, *console.JSONProjection, error) {
			return console.FormatHuman, nil, nil
		},
	}); err != nil {
		t.Fatal(err)
	}

	builder := tuiRequestBuilder{routes: routes, grammar: spec.Root(nil), root: t.TempDir()}
	request, err := builder.Build(context.Background(), tui.FormRequest{
		Action: "work.item.list",
		Parameters: []tui.Parameter{
			{Name: "project", Value: "project"},
			{Name: "top", Value: 20},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := request.(guideRequest); !ok {
		t.Fatalf("request = %T, want guideRequest", request)
	}
}

func TestOpenURLHandlerRejectsNonHTTPURL(t *testing.T) {
	dispatcher := action.NewDispatcher()
	for _, handler := range bootstrapHandlers() {
		if err := dispatcher.Register(handler); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := dispatcher.Dispatch(context.Background(), openURLRequest{URL: "file:///tmp/secret"}, action.Runtime{}); err == nil {
		t.Fatal("non-HTTP URL was accepted")
	}
}
