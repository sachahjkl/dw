package controller

import (
	"context"
	"testing"

	"github.com/sachahjkl/dw/internal/webservice"
)

type webLifecycleStub struct {
	opened webservice.OpenResult
}

func (stub webLifecycleStub) Start(context.Context, webservice.StartOptions) (webservice.StartResult, error) {
	return webservice.StartResult{}, nil
}

func (stub webLifecycleStub) Stop(context.Context) error { return nil }

func (stub webLifecycleStub) Status(context.Context) (webservice.StatusV1, error) {
	return webservice.StatusV1{}, nil
}

func (stub webLifecycleStub) Open(context.Context) (webservice.OpenResult, error) {
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
