package atlassian

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
)

func TestProviderProjectsJiraAndBitbucketCapabilities(t *testing.T) {
	t.Setenv("DW_TEST_JIRA_TOKEN", "jira-token")
	t.Setenv("DW_TEST_BITBUCKET_TOKEN", "bitbucket-token")
	var transitioned, commented, pullUpdated bool
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		writer.Header().Set("Content-Type", "application/json")
		switch request.Method + " " + request.URL.Path {
		case "GET /rest/api/3/myself":
			if username, password, ok := request.BasicAuth(); !ok || username != "ada@example.com" || password != "jira-token" {
				t.Errorf("Jira auth = %q/%q/%v", username, password, ok)
			}
			json.NewEncoder(writer).Encode(map[string]any{"displayName": "Ada"})
		case "GET /user":
			if request.Header.Get("Authorization") != "Bearer bitbucket-token" {
				t.Errorf("Bitbucket auth = %q", request.Header.Get("Authorization"))
			}
			json.NewEncoder(writer).Encode(map[string]any{"display_name": "Grace"})
		case "GET /rest/api/3/issue/APP-7":
			json.NewEncoder(writer).Encode(jiraFixture())
		case "GET /rest/api/3/search/jql":
			if !strings.Contains(request.URL.Query().Get("jql"), "assignee = currentUser()") {
				t.Errorf("JQL = %q", request.URL.Query().Get("jql"))
			}
			json.NewEncoder(writer).Encode(map[string]any{"issues": []any{jiraFixture()}})
		case "GET /rest/api/3/issue/APP-7/transitions":
			json.NewEncoder(writer).Encode(map[string]any{"transitions": []any{map[string]any{"id": "31", "name": "Done", "to": map[string]any{"name": "Done"}}}})
		case "POST /rest/api/3/issue/APP-7/transitions":
			transitioned = true
			writer.WriteHeader(http.StatusNoContent)
		case "POST /rest/api/3/issue/APP-7/comment":
			commented = true
			writer.WriteHeader(http.StatusCreated)
			writer.Write([]byte(`{}`))
		case "GET /repositories/ws/repo/pullrequests":
			json.NewEncoder(writer).Encode(map[string]any{"values": []any{bitbucketFixture()}})
		case "POST /repositories/ws/repo/pullrequests":
			created := bitbucketFixture()
			created["description"] = ""
			json.NewEncoder(writer).Encode(created)
		case "GET /repositories/ws/repo/pullrequests/12":
			created := bitbucketFixture()
			created["description"] = ""
			json.NewEncoder(writer).Encode(created)
		case "PUT /repositories/ws/repo/pullrequests/12":
			var body map[string]any
			json.NewDecoder(request.Body).Decode(&body)
			pullUpdated = strings.Contains(body["description"].(string), "APP-7")
			json.NewEncoder(writer).Encode(bitbucketFixture())
		default:
			http.Error(writer, request.Method+" "+request.URL.Path, http.StatusNotFound)
		}
	}))
	defer server.Close()
	provider := New(Options{
		Jira: JiraOptions{
			URL:                      server.URL,
			Project:                  "APP",
			Email:                    "ada@example.com",
			TokenEnvironmentVariable: "DW_TEST_JIRA_TOKEN",
		},
		Bitbucket: BitbucketOptions{
			URL:                      server.URL,
			Workspace:                "ws",
			Repository:               "repo",
			TokenEnvironmentVariable: "DW_TEST_BITBUCKET_TOKEN",
		},
		Client: server.Client(),
	}, nil)
	ctx := context.Background()

	status, err := provider.AuthStatus(ctx, work.ProjectRef{})
	if err != nil || !status.Authenticated || status.Principal != "Ada / Grace" {
		t.Fatalf("status = %#v, err=%v", status, err)
	}
	items, err := provider.ReadItems(ctx, work.ProjectRef{}, []work.ItemID{"APP-7"}, work.ReadOptions{})
	if err != nil || len(items) != 1 || items[0].Title != "Ship it" || items[0].ParentID.Or("") != "APP-1" {
		t.Fatalf("items = %#v, err=%v", items, err)
	}
	assigned, err := provider.QueryAssigned(ctx, work.ProjectRef{}, work.AssignedQuery{ExcludeFinalStates: true})
	if err != nil || len(assigned) != 1 {
		t.Fatalf("assigned = %#v, err=%v", assigned, err)
	}
	changes, err := provider.UpdateStates(ctx, work.ProjectRef{}, []work.StateChange{{ID: "APP-7", State: "Done", Comment: "shipped"}})
	if err != nil || len(changes) != 1 || !changes[0].Changed || !transitioned || !commented {
		t.Fatalf("changes = %#v transition=%v comment=%v err=%v", changes, transitioned, commented, err)
	}
	pulls, err := provider.ListPullRequests(ctx, work.ProjectRef{}, work.PullRequestQuery{Status: "active"})
	if err != nil || len(pulls) != 1 || pulls[0].WorkItemIDs[0] != "APP-7" {
		t.Fatalf("pulls = %#v, err=%v", pulls, err)
	}
	created, err := provider.CreatePullRequest(ctx, work.ProjectRef{}, work.PullRequestCreate{Repository: "repo", SourceRef: "refs/heads/feature", TargetRef: "refs/heads/main", Title: "Ship", WorkItemIDs: []work.ItemID{"APP-7"}})
	if err != nil || created.ID != "12" || !pullUpdated {
		t.Fatalf("created = %#v updated=%v err=%v", created, pullUpdated, err)
	}
	if got := provider.ExtractCommitReferences("APP-7 app-8 APP-7"); len(got) != 2 || got[0] != "APP-7" || got[1] != "APP-8" {
		t.Fatalf("references = %#v", got)
	}
}

func TestBitbucketPaginationKeepsConfiguredBasePath(t *testing.T) {
	var server *httptest.Server
	server = httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.URL.Path != "/2.0/repositories/ws/repo/pullrequests" {
			t.Errorf("request path = %q", request.URL.Path)
		}
		writer.Header().Set("Content-Type", "application/json")
		if request.URL.Query().Get("page") == "2" {
			second := bitbucketFixture()
			second["id"] = 13
			json.NewEncoder(writer).Encode(map[string]any{"values": []any{second}})
			return
		}
		json.NewEncoder(writer).Encode(map[string]any{
			"values": []any{bitbucketFixture()},
			"next":   server.URL + "/2.0/repositories/ws/repo/pullrequests?page=2",
		})
	}))
	defer server.Close()
	provider := New(Options{
		Bitbucket: BitbucketOptions{
			URL:        server.URL + "/2.0",
			Workspace:  "ws",
			Repository: "repo",
		},
		Client: server.Client(),
	}, nil)

	pulls, err := provider.ListPullRequests(context.Background(), work.ProjectRef{}, work.PullRequestQuery{Status: "active"})
	if err != nil || len(pulls) != 2 || pulls[1].ID != "13" {
		t.Fatalf("pulls = %#v, err=%v", pulls, err)
	}
}

func TestOptionsDecodeNestedProviderConfiguration(t *testing.T) {
	var options Options
	err := json.Unmarshal([]byte(`{
		"jira": {
			"url": "https://jira.example.test",
			"project": "APP",
			"email": "ada@example.test",
			"tokenEnvironmentVariable": "JIRA_TOKEN",
			"credentialKey": "atlassian/jira-token"
		},
		"bitbucket": {
			"url": "https://api.bitbucket.example.test/2.0",
			"workspace": "acme",
			"repository": "app",
			"username": "ada",
			"tokenEnvironmentVariable": "BITBUCKET_TOKEN",
			"credentialKey": "atlassian/bitbucket-token"
		}
	}`), &options)
	if err != nil {
		t.Fatal(err)
	}
	if options.Jira.URL != "https://jira.example.test" || options.Jira.Project != "APP" || options.Jira.Email != "ada@example.test" || options.Jira.TokenEnvironmentVariable != "JIRA_TOKEN" || options.Jira.CredentialKey != "atlassian/jira-token" {
		t.Fatalf("Jira options = %#v", options.Jira)
	}
	if options.Bitbucket.URL != "https://api.bitbucket.example.test/2.0" || options.Bitbucket.Workspace != "acme" || options.Bitbucket.Repository != "app" || options.Bitbucket.Username != "ada" || options.Bitbucket.TokenEnvironmentVariable != "BITBUCKET_TOKEN" || options.Bitbucket.CredentialKey != "atlassian/bitbucket-token" {
		t.Fatalf("Bitbucket options = %#v", options.Bitbucket)
	}
}

func jiraFixture() map[string]any {
	return map[string]any{
		"key": "APP-7",
		"fields": map[string]any{
			"summary": "Ship it",
			"description": map[string]any{
				"type": "doc",
				"content": []any{map[string]any{
					"type":    "paragraph",
					"content": []any{map[string]any{"type": "text", "text": "Details"}},
				}},
			},
			"status":    map[string]any{"name": "In Progress", "statusCategory": map[string]any{"key": "indeterminate"}},
			"issuetype": map[string]any{"name": "Task"},
			"assignee":  map[string]any{"displayName": "Grace"},
			"parent":    map[string]any{"key": "APP-1", "fields": map[string]any{"summary": "Parent"}},
			"labels":    []string{"release"},
		},
	}
}

func bitbucketFixture() map[string]any {
	return map[string]any{"id": 12, "title": "Ship APP-7", "description": "Implements APP-7", "state": "OPEN", "source": map[string]any{"branch": map[string]any{"name": "feature"}}, "destination": map[string]any{"branch": map[string]any{"name": "main"}}, "author": map[string]any{"display_name": "Ada"}, "links": map[string]any{"html": map[string]any{"href": "https://bitbucket.test/pr/12"}, "self": map[string]any{"href": "https://api.bitbucket.test/pr/12"}}}
}
