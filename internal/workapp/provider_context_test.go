package workapp

import (
	"context"
	"errors"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workspace"
)

type contextTestProvider struct {
	options work.ReadOptions
}

func (*contextTestProvider) Name() work.ProviderName { return "context-test" }
func (provider *contextTestProvider) ReadRichContext(_ context.Context, _ work.ProjectRef, _ []work.ItemID, options work.ReadOptions) ([]work.RichContext, error) {
	provider.options = options
	return []work.RichContext{{Item: work.Item{ID: "42"}}}, nil
}

func TestContextAppliesSummaryAndCommentOptions(t *testing.T) {
	provider := &contextTestProvider{}
	registry := work.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	service := New(registry)
	report, err := service.Context(context.Background(), ContextRequest{
		Provider: "context-test", Project: "project", IDs: []string{"42"}, Summary: true,
		IncludeComments: true, Comments: 7, Mode: ContextRich,
	}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if provider.options.IncludeRelations || !provider.options.IncludeComments || provider.options.CommentLimit != 7 {
		t.Fatalf("read options = %#v", provider.options)
	}
	if len(report.Items) != 1 || len(report.Expanded) != 0 {
		t.Fatalf("report = %#v", report)
	}
}

type childPartialProvider struct{}

func (childPartialProvider) Name() work.ProviderName { return "child-partial" }
func (childPartialProvider) CreateChild(context.Context, work.ProjectRef, work.ChildCreate) (work.ChildCreateResult, error) {
	return work.ChildCreateResult{ID: "9", Title: "Child"}, errors.New("link failed")
}

type childLookup struct{}

func (childLookup) Resolve(context.Context, string, *string, string, []string, bool) (string, error) {
	return "/workspace", nil
}
func (childLookup) Manifest(context.Context, string) (workspace.Manifest, error) {
	return workspace.Manifest{Project: "project", WorkItemID: "7"}, nil
}

type childWriter struct{ called bool }

func (writer *childWriter) AddChild(context.Context, string, workspace.ChildTask) (workspace.Manifest, error) {
	writer.called = true
	return workspace.Manifest{}, nil
}

func TestCreateChildReportsRemoteChildWhenLinkFails(t *testing.T) {
	registry := work.NewRegistry()
	if err := registry.Register(childPartialProvider{}); err != nil {
		t.Fatal(err)
	}
	writer := &childWriter{}
	service := &Service{Providers: registry, Lookup: childLookup{}, Children: writer}
	report, err := service.CreateChild(context.Background(), ChildRequest{Provider: "child-partial", Repository: "repo"}, nil)
	if err == nil || report.Created.ID != "9" || writer.called {
		t.Fatalf("report = %#v, writer called = %t, err = %v", report, writer.called, err)
	}
}
