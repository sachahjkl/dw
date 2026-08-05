package ado

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/sachahjkl/dw/internal/work"
)

func TestLinkPullRequestWorkItemAddsArtifactRelation(t *testing.T) {
	t.Setenv("DW_ADO_TOKEN", "token")
	artifactID := "vstfs:///Git/PullRequestId/project-id%2Frepository-id%2F12"
	requests := 0
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		requests++
		writer.Header().Set("Content-Type", "application/json")
		switch requests {
		case 1:
			if request.Method != http.MethodGet || request.URL.Path != "/project/_apis/git/repositories/repo/pullRequests/12/workitems" {
				t.Fatalf("work item request = %s %s", request.Method, request.URL.Path)
			}
			json.NewEncoder(writer).Encode(map[string]any{"value": []any{}})
		case 2:
			if request.Method != http.MethodGet || request.URL.Path != "/project/_apis/git/repositories/repo/pullRequests/12" {
				t.Fatalf("pull request request = %s %s", request.Method, request.URL.Path)
			}
			json.NewEncoder(writer).Encode(map[string]any{"artifactId": artifactID})
		case 3:
			if request.Method != http.MethodPatch || request.URL.Path != "/project/_apis/wit/workitems/42" {
				t.Fatalf("link request = %s %s", request.Method, request.URL.Path)
			}
			if got := request.Header.Get("Content-Type"); got != "application/json-patch+json" {
				t.Fatalf("content type = %q", got)
			}
			var patch []jsonPatchOperation
			if err := json.NewDecoder(request.Body).Decode(&patch); err != nil {
				t.Fatal(err)
			}
			if len(patch) != 1 || patch[0].Op != "add" || patch[0].Path != "/relations/-" {
				t.Fatalf("patch = %#v", patch)
			}
			relation := patch[0].Value.(map[string]any)
			attributes := relation["attributes"].(map[string]any)
			if relation["rel"] != "ArtifactLink" || relation["url"] != artifactID || attributes["name"] != "Pull Request" {
				t.Fatalf("relation = %#v", relation)
			}
			json.NewEncoder(writer).Encode(map[string]any{})
		default:
			t.Fatalf("unexpected request: %s %s", request.Method, request.URL.Path)
		}
	}))
	defer server.Close()
	provider := New(Options{Organization: server.URL, Project: "project"}, nil)
	provider.Transport.NewClient = func() HTTPDoer { return server.Client() }

	err := provider.LinkPullRequestWorkItem(context.Background(), work.ProjectRef{}, "repo", "12", "42")
	if err != nil {
		t.Fatal(err)
	}
	if requests != 3 {
		t.Fatalf("request count = %d", requests)
	}
}

func TestLinkPullRequestWorkItemSkipsExistingRelation(t *testing.T) {
	t.Setenv("DW_ADO_TOKEN", "token")
	requests := 0
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		requests++
		writer.Header().Set("Content-Type", "application/json")
		json.NewEncoder(writer).Encode(map[string]any{"value": []any{map[string]any{"id": 42}}})
	}))
	defer server.Close()
	provider := New(Options{Organization: server.URL, Project: "project"}, nil)
	provider.Transport.NewClient = func() HTTPDoer { return server.Client() }

	if err := provider.LinkPullRequestWorkItem(context.Background(), work.ProjectRef{}, "repo", "12", "42"); err != nil {
		t.Fatal(err)
	}
	if requests != 1 {
		t.Fatalf("request count = %d", requests)
	}
}
