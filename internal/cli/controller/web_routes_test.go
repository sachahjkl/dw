package controller

import (
	"context"
	"testing"

	"github.com/sachahjkl/dw/internal/cli/parse"
	"github.com/sachahjkl/dw/internal/cli/spec"
	"github.com/sachahjkl/dw/internal/webservice"
)

type webLifecycleStub struct {
	opened      webservice.OpenResult
	startedWith *webservice.StartOptions
	openedWith  *webservice.OpenOptions
}

func (stub webLifecycleStub) Start(_ context.Context, options webservice.StartOptions) (webservice.StartResult, error) {
	if stub.startedWith != nil {
		*stub.startedWith = options
	}
	return webservice.StartResult{}, nil
}

func (stub webLifecycleStub) Stop(context.Context) error { return nil }

func (stub webLifecycleStub) Status(context.Context) (webservice.StatusV1, error) {
	return webservice.StatusV1{}, nil
}

func (stub webLifecycleStub) Open(_ context.Context, options webservice.OpenOptions) (webservice.OpenResult, error) {
	if stub.openedWith != nil {
		*stub.openedWith = options
	}
	return stub.opened, nil
}

func (stub webLifecycleStub) Register(context.Context, webservice.RegisterOptions) (webservice.StatusV1, error) {
	return webservice.StatusV1{}, nil
}

func (stub webLifecycleStub) Unregister(context.Context) error { return nil }

func TestWebOpenPrintsTicketURLWhenBrowserIsUnavailable(t *testing.T) {
	lifecycle := webLifecycleStub{opened: webservice.OpenResult{
		Location: "http://127.0.0.1:7331/?ticket=single-use",
	}}
	route := webRoutes(lifecycle, nil)["web.open"]

	outcome, err := route.Direct(context.Background(), Execution{}, nil)
	if err != nil {
		t.Fatal(err)
	}
	want := "URL: http://127.0.0.1:7331/?ticket=single-use\nBrowser: not opened; use the URL above\n"
	if got := string(outcome.Output.Body); got != want {
		t.Fatalf("output = %q, want %q", got, want)
	}
}

func TestWebRoutesMapAccessOptions(t *testing.T) {
	token := "chosen-token"
	startInvocation, err := parse.Parse(spec.Root(nil), []string{"web", "start", "--open", "--no-expiry", "--token", token})
	if err != nil {
		t.Fatal(err)
	}
	var startOptions webservice.StartOptions
	lifecycle := webLifecycleStub{startedWith: &startOptions}
	if _, routeErr := webRoutes(lifecycle, nil)["web.start"].Direct(context.Background(), Execution{}, startInvocation); routeErr != nil {
		t.Fatal(routeErr)
	}
	if !startOptions.Open || !startOptions.NoExpiry || startOptions.Token == nil || *startOptions.Token != token {
		t.Fatalf("start options = %#v", startOptions)
	}

	openInvocation, err := parse.Parse(spec.Root(nil), []string{"web", "open", "--no-expiry"})
	if err != nil {
		t.Fatal(err)
	}
	var openOptions webservice.OpenOptions
	lifecycle = webLifecycleStub{openedWith: &openOptions}
	if _, routeErr := webRoutes(lifecycle, nil)["web.open"].Direct(context.Background(), Execution{}, openInvocation); routeErr != nil {
		t.Fatal(routeErr)
	}
	if !openOptions.NoExpiry {
		t.Fatalf("open options = %#v", openOptions)
	}
}
