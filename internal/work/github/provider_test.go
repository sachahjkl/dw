package github

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
)

func TestProviderProjectsIssuesStatesPullRequestsAndAuth(t *testing.T) {
	t.Setenv("DW_TEST_GITHUB_TOKEN", "test-token")
	var statePatch, commentPosted, pullPatched, childLinked bool
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Authorization") != "Bearer test-token" {
			t.Errorf("authorization = %q", request.Header.Get("Authorization"))
		}
		writer.Header().Set("Content-Type", "application/json")
		switch request.Method + " " + request.URL.Path {
		case "GET /user":
			json.NewEncoder(writer).Encode(map[string]any{"login": "octocat"})
		case "GET /repos/acme/app/issues/7":
			json.NewEncoder(writer).Encode(map[string]any{"number": 7, "title": "Ship it", "body": "Details", "state": "open", "html_url": "https://github.test/acme/app/issues/7", "user": map[string]any{"login": "ada"}, "assignee": map[string]any{"login": "grace"}, "labels": []any{map[string]any{"name": "feature"}}})
		case "GET /repos/acme/app/issues":
			json.NewEncoder(writer).Encode([]any{
				map[string]any{"number": 7, "title": "Ship it", "state": "open"},
				map[string]any{"number": 8, "title": "A PR", "state": "open", "pull_request": map[string]any{}},
			})
		case "POST /repos/acme/app/issues":
			json.NewEncoder(writer).Encode(map[string]any{"id": 9007, "number": 9, "title": "Child", "html_url": "https://github.test/acme/app/issues/9"})
		case "POST /repos/acme/app/issues/7/sub_issues":
			var body map[string]any
			json.NewDecoder(request.Body).Decode(&body)
			childLinked = body["sub_issue_id"] == float64(9007)
			writer.WriteHeader(http.StatusCreated)
			writer.Write([]byte(`{}`))
		case "PATCH /repos/acme/app/issues/7":
			var body map[string]any
			json.NewDecoder(request.Body).Decode(&body)
			statePatch = body["state"] == "closed"
			json.NewEncoder(writer).Encode(map[string]any{"number": 7, "title": "Ship it", "state": "closed"})
		case "POST /repos/acme/app/issues/7/comments":
			commentPosted = true
			writer.WriteHeader(http.StatusCreated)
			writer.Write([]byte(`{}`))
		case "GET /repos/acme/app/pulls":
			json.NewEncoder(writer).Encode([]any{map[string]any{"number": 12, "title": "Fix #7", "body": "Closes #7", "state": "open", "html_url": "https://github.test/pr/12", "url": "https://api.github.test/pr/12", "head": map[string]any{"ref": "feature"}, "base": map[string]any{"ref": "main"}}})
		case "POST /repos/acme/app/pulls":
			json.NewEncoder(writer).Encode(map[string]any{"number": 12, "title": "Fix", "body": "", "state": "open", "html_url": "https://github.test/pr/12", "url": "https://api.github.test/pr/12"})
		case "GET /repos/acme/app/pulls/12":
			json.NewEncoder(writer).Encode(map[string]any{"number": 12, "title": "Fix", "body": ""})
		case "PATCH /repos/acme/app/pulls/12":
			var body map[string]string
			json.NewDecoder(request.Body).Decode(&body)
			pullPatched = strings.Contains(body["body"], "Closes #7")
			json.NewEncoder(writer).Encode(map[string]any{"number": 12})
		default:
			http.Error(writer, request.Method+" "+request.URL.Path, http.StatusNotFound)
		}
	}))
	defer server.Close()
	provider := New(Options{Owner: "acme", Repository: "app", APIURL: server.URL, Client: server.Client(), TokenEnvironmentVariable: "DW_TEST_GITHUB_TOKEN"}, nil)
	ctx := context.Background()

	status, err := provider.AuthStatus(ctx, work.ProjectRef{})
	if err != nil || !status.Authenticated || status.Principal != "octocat" {
		t.Fatalf("auth status = %#v, err=%v", status, err)
	}
	items, err := provider.ReadItems(ctx, work.ProjectRef{}, []work.ItemID{"7"}, work.ReadOptions{})
	if err != nil || len(items) != 1 || items[0].AssignedTo != "grace" || items[0].Tags[0] != "feature" {
		t.Fatalf("items = %#v, err=%v", items, err)
	}
	assigned, err := provider.QueryAssigned(ctx, work.ProjectRef{}, work.AssignedQuery{Top: 10, ExcludeFinalStates: true})
	if err != nil || len(assigned) != 1 || assigned[0].ID != "7" {
		t.Fatalf("assigned = %#v, err=%v", assigned, err)
	}
	changes, err := provider.UpdateStates(ctx, work.ProjectRef{}, []work.StateChange{{ID: "7", State: "done", Comment: "Completed"}})
	if err != nil || len(changes) != 1 || !changes[0].Changed || !statePatch || !commentPosted {
		t.Fatalf("changes = %#v, patch=%v comment=%v err=%v", changes, statePatch, commentPosted, err)
	}
	pulls, err := provider.ListPullRequests(ctx, work.ProjectRef{}, work.PullRequestQuery{Status: "active"})
	if err != nil || len(pulls) != 1 || pulls[0].SourceRef != "refs/heads/feature" || len(pulls[0].WorkItemIDs) != 1 || pulls[0].WorkItemIDs[0] != "7" {
		t.Fatalf("pulls = %#v, err=%v", pulls, err)
	}
	child, err := provider.CreateChild(ctx, work.ProjectRef{}, work.ChildCreate{ParentID: "7", Title: "Child"})
	if err != nil || child.ID != "9" || !childLinked {
		t.Fatalf("child = %#v, linked=%v err=%v", child, childLinked, err)
	}
	created, err := provider.CreatePullRequest(ctx, work.ProjectRef{}, work.PullRequestCreate{Repository: "app", SourceRef: "refs/heads/feature", TargetRef: "refs/heads/main", Title: "Fix", WorkItemIDs: []work.ItemID{"7"}})
	if err != nil || created.ID != "12" || !pullPatched {
		t.Fatalf("created = %#v, patched=%v err=%v", created, pullPatched, err)
	}
	if got := provider.ExtractCommitReferences("Fix #7 and #7, refs #9"); len(got) != 2 || got[0] != "7" || got[1] != "9" {
		t.Fatalf("commit references = %#v", got)
	}
}
