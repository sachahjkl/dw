package workapp

import (
	"context"
	"fmt"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/work"
)

type changelogProvider struct {
	items   map[work.ItemID]work.Item
	reads   []work.ReadOptions
	pullIDs map[work.RepositoryName][]work.ItemID
}

func (*changelogProvider) Name() work.ProviderName { return "changelog" }

func (provider *changelogProvider) ReadItems(_ context.Context, _ work.ProjectRef, ids []work.ItemID, options work.ReadOptions) ([]work.Item, error) {
	provider.reads = append(provider.reads, options)
	result := make([]work.Item, 0, len(ids))
	for _, id := range ids {
		if item, found := provider.items[id]; found {
			if !options.IncludeRelations {
				item.ParentID = contract.Optional[work.ItemID]{}
			}
			result = append(result, item)
		}
	}
	return result, nil
}

func (*changelogProvider) ListPullRequests(context.Context, work.ProjectRef, work.PullRequestQuery) ([]work.PullRequest, error) {
	return nil, nil
}

func (*changelogProvider) ActivePullRequest(context.Context, work.ProjectRef, work.RepositoryName, string) (*work.PullRequest, error) {
	return nil, nil
}

func (provider *changelogProvider) PullRequestWorkItemIDs(_ context.Context, _ work.ProjectRef, repository work.RepositoryName, _ work.PullRequestID) ([]work.ItemID, error) {
	ids, found := provider.pullIDs[repository]
	if !found {
		return nil, fmt.Errorf("repository %s: %w", repository, work.ErrPullRequestNotFound)
	}
	return ids, nil
}

func changelogService(t *testing.T, provider *changelogProvider) *Service {
	t.Helper()
	registry := work.NewRegistry()
	if err := registry.Register(provider); err != nil {
		t.Fatal(err)
	}
	return &Service{Providers: registry}
}

func TestChangelogGroupsByParentWithRelationsAndReportsMissingItems(t *testing.T) {
	provider := &changelogProvider{items: map[work.ItemID]work.Item{
		"1":  {ID: "1", Title: "Parent"},
		"10": {ID: "10", Title: "Child", ParentID: contract.Some(work.ItemID("1"))},
		"11": {ID: "11", Title: "Orphan"},
	}}
	report, err := changelogService(t, provider).Changelog(context.Background(), ChangelogRequest{Provider: "changelog", Project: "p", Source: ChangelogWorkItems, WorkItemIDs: []string{"10", "11", "404"}, GroupByParent: true}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if !provider.reads[0].IncludeRelations {
		t.Fatal("grouped changelog did not request relations")
	}
	section := report.Sections[0]
	if len(section.Groups) != 2 || section.Groups[0].Parent.ID != "1" || len(section.Groups[0].Items) != 1 || section.Groups[1].Parent.ID != "" || section.Groups[1].Items[0].ID != "11" {
		t.Fatalf("groups = %+v", section.Groups)
	}
	if len(section.Warnings) != 1 || !strings.Contains(section.Warnings[0].Detail, "404") {
		t.Fatalf("warnings = %+v", section.Warnings)
	}
}

func TestChangelogFromPullRequestToleratesRepositoriesWithoutThePullRequest(t *testing.T) {
	provider := &changelogProvider{items: map[work.ItemID]work.Item{"5": {ID: "5"}}, pullIDs: map[work.RepositoryName][]work.ItemID{"b": {"5"}}}
	service := changelogService(t, provider)
	report, err := service.Changelog(context.Background(), ChangelogRequest{Provider: "changelog", Project: "p", Source: ChangelogPullRequests, Repositories: []string{"a", "b"}, PullRequestIDs: []int64{7}}, nil)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Join(report.WorkItemIDs, ",") != "5" {
		t.Fatalf("ids = %v", report.WorkItemIDs)
	}
	if _, err := service.Changelog(context.Background(), ChangelogRequest{Provider: "changelog", Project: "p", Source: ChangelogPullRequests, Repositories: []string{"a"}, PullRequestIDs: []int64{7}}, nil); err == nil {
		t.Fatal("pull request absent from every repository was accepted")
	}
}
