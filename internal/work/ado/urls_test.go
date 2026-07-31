package ado

import (
	"net/url"
	"testing"
)

func TestADOURLsPreserveStandardOutputs(t *testing.T) {
	options := Options{Organization: "https://dev.azure.com/acme/", Project: "Project", APIVersion: "7.1"}
	tests := []struct {
		name string
		got  string
		want string
	}{
		{"expanded item", ExpandedWorkItemURL(options, "42"), "https://dev.azure.com/acme/Project/_apis/wit/workitems/42?$expand=all&api-version=7.1"},
		{"comments", WorkItemCommentsURL(options, "42", 10), "https://dev.azure.com/acme/Project/_apis/wit/workItems/42/comments?$top=10&api-version=7.1"},
		{"item", WorkItemURL(options, "42"), "https://dev.azure.com/acme/Project/_apis/wit/workitems/42?api-version=7.1"},
		{"batch", WorkItemsBatchURL(options), "https://dev.azure.com/acme/Project/_apis/wit/workitemsbatch?api-version=7.1"},
		{"wiql", WIQLURL(options, 20), "https://dev.azure.com/acme/Project/_apis/wit/wiql?$top=20&api-version=7.1"},
		{"item API", WorkItemAPIURL(options, "42"), "https://dev.azure.com/acme/Project/_apis/wit/workItems/42"},
		{"item web", WorkItemWebURL(options, "42"), "https://dev.azure.com/acme/Project/_workitems/edit/42"},
		{"create item", CreateWorkItemURL(options, "Bug"), "https://dev.azure.com/acme/Project/_apis/wit/workitems/$Bug?api-version=7.1"},
		{"pull requests", PullRequestsURL(options, "repo"), "https://dev.azure.com/acme/Project/_apis/git/repositories/repo/pullrequests?api-version=7.1"},
		{"pull request web", PullRequestWebURL(options, "repo", 12), "https://dev.azure.com/acme/Project/_git/repo/pullrequest/12"},
		{"active pull requests", ActivePullRequestsURL(options, "repo", "refs/heads/topic"), "https://dev.azure.com/acme/Project/_apis/git/repositories/repo/pullrequests?searchCriteria.status=active&searchCriteria.sourceRefName=refs%2Fheads%2Ftopic&api-version=7.1"},
		{"repository pull requests", ActivePullRequestsForRepositoryURL(options, "repo"), "https://dev.azure.com/acme/Project/_apis/git/repositories/repo/pullrequests?searchCriteria.status=active&api-version=7.1"},
		{"pull request items", PullRequestWorkItemsURL(options, "repo", 12), "https://dev.azure.com/acme/Project/_apis/git/repositories/repo/pullRequests/12/workitems?api-version=7.1"},
		{"connection data", ConnectionDataURL(options), "https://dev.azure.com/acme/_apis/connectionData?connectOptions=1&lastChangeId=-1&lastChangeId64=-1"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if test.got != test.want {
				t.Fatalf("URL = %q, want %q", test.got, test.want)
			}
		})
	}
}

func TestADOURLSeparatesAndEscapesSegmentsAndQuery(t *testing.T) {
	options := Options{
		Organization: "https://dev.azure.com/acme%20org/?ignored=yes#fragment",
		Project:      "Team / One",
		APIVersion:   "7.1-preview&x",
	}
	value := ActivePullRequestsURL(options, "repo/name #1", "refs/heads/a&b")
	parsed, err := url.Parse(value)
	if err != nil {
		t.Fatal(err)
	}
	wantPath := "/acme%20org/Team%20%2F%20One/_apis/git/repositories/repo%2Fname%20%231/pullrequests"
	if parsed.EscapedPath() != wantPath {
		t.Fatalf("escaped path = %q, want %q", parsed.EscapedPath(), wantPath)
	}
	query := parsed.Query()
	if query.Get("api-version") != options.APIVersion || query.Get("searchCriteria.sourceRefName") != "refs/heads/a&b" || query.Get("searchCriteria.status") != "active" {
		t.Fatalf("query = %#v", query)
	}
	if parsed.Fragment != "" || query.Has("ignored") {
		t.Fatalf("base URL query or fragment leaked into generated URL: %q", value)
	}
}

func TestOrganizationNameUsesURLPath(t *testing.T) {
	if got := OrganizationName("https://dev.azure.com/acme%20org/?ignored=yes"); got != "acme org" {
		t.Fatalf("OrganizationName = %q", got)
	}
}
