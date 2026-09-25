package bootstrap

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/sachahjkl/dw/internal/gitrepo"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workapp"
)

const changelogProjectsJSON = `{
  "schema": 1,
  "projects": {
    "base": {
      "displayName": "Base",
      "repositories": {
        "shared": {"url": "https://example.test/shared.git", "defaultBranch": "develop"}
      }
    },
    "app": {
      "displayName": "App",
      "repositories": {
        "front": {"url": "https://example.test/front.git", "defaultBranch": "main", "anchorName": "front-anchor.git"},
        "gone": {"url": "https://example.test/gone.git"}
      },
      "includedProjects": ["base"]
    }
  }
}`

func changelogRoot(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	for _, directory := range []string{"config", filepath.Join("projects", "app", "repositories", "front-anchor.git"), filepath.Join("projects", "base", "repositories", "shared.git")} {
		if err := os.MkdirAll(filepath.Join(root, directory), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "config", "projects.json"), []byte(changelogProjectsJSON), 0o644); err != nil {
		t.Fatal(err)
	}
	return root
}

func TestChangelogTargetsResolveAnchors(t *testing.T) {
	root := changelogRoot(t)
	targets, err := changelogTargets(workapp.ChangelogRequest{Root: root, Project: "app"}, "")
	if err != nil {
		t.Fatal(err)
	}
	byName := make(map[string]changelogTarget, len(targets))
	for _, target := range targets {
		byName[target.name] = target
	}
	if front := byName["front"]; front.err != nil || front.path != filepath.Join(root, "projects", "app", "repositories", "front-anchor.git") || front.defaultBranch != "main" {
		t.Fatalf("front = %+v", front)
	}
	if shared := byName["shared"]; shared.err != nil || shared.path != filepath.Join(root, "projects", "base", "repositories", "shared.git") || shared.defaultBranch != "develop" {
		t.Fatalf("shared = %+v", shared)
	}
	if gone := byName["gone"]; gone.err == nil || !strings.Contains(gone.err.Error(), "gone") {
		t.Fatalf("gone = %+v", gone)
	}
}

func TestChangelogTargetsRejectUnknownProjectAndRepository(t *testing.T) {
	root := changelogRoot(t)
	if _, err := changelogTargets(workapp.ChangelogRequest{Root: root, Project: "missing"}, ""); err == nil {
		t.Fatal("unknown project was accepted")
	}
	targets, err := changelogTargets(workapp.ChangelogRequest{Root: root, Project: "app", Repositories: []string{"nope", root}}, "")
	if err != nil {
		t.Fatal(err)
	}
	if targets[0].err == nil || targets[0].path != "" {
		t.Fatalf("unknown repository fell back to a path: %+v", targets[0])
	}
	if targets[1].err != nil || targets[1].path != root {
		t.Fatalf("existing explicit path was rejected: %+v", targets[1])
	}
}

type fakeChangelogGit struct {
	fetched []string
	ranges  []gitrepo.RevisionRange
	fail    bool
}

func (git *fakeChangelogGit) FetchAnchor(_ context.Context, path gitrepo.RepositoryPath) error {
	git.fetched = append(git.fetched, string(path))
	return nil
}

func (git *fakeChangelogGit) CommitMessagesInRangeAt(_ context.Context, _ gitrepo.RepositoryPath, revisions gitrepo.RevisionRange) (gitrepo.CommitMessages, error) {
	git.ranges = append(git.ranges, revisions)
	if git.fail {
		return "", errors.New("bad revision")
	}
	return "", nil
}

type noReferences struct{}

func (noReferences) Name() work.ProviderName                      { return "none" }
func (noReferences) ExtractCommitReferences(string) []work.ItemID { return nil }

func TestResolveChangelogSectionsUsesDefaultBranchAndFetchesAnchors(t *testing.T) {
	git := &fakeChangelogGit{}
	targets := []changelogTarget{{name: "anchor", path: "/a", defaultBranch: "develop"}, {name: "worktree", path: "/w", defaultBranch: "main", worktree: true}}
	if _, err := resolveChangelogSections(context.Background(), git, noReferences{}, workapp.ChangelogRequest{}, targets); err != nil {
		t.Fatal(err)
	}
	if len(git.fetched) != 1 || git.fetched[0] != "/a" {
		t.Fatalf("fetched = %v", git.fetched)
	}
	if git.ranges[0].From != "origin/develop" || git.ranges[1].From != "origin/main" {
		t.Fatalf("ranges = %+v", git.ranges)
	}
}

func TestResolveChangelogSectionsFailsWhenEveryRepositoryFails(t *testing.T) {
	git := &fakeChangelogGit{fail: true}
	targets := []changelogTarget{{name: "a", path: "/a", defaultBranch: "main"}, {name: "b", err: errors.New("anchor missing")}}
	if _, err := resolveChangelogSections(context.Background(), git, noReferences{}, workapp.ChangelogRequest{}, targets); err == nil {
		t.Fatal("all repositories failed without an error")
	}
	git.fail = false
	sections, err := resolveChangelogSections(context.Background(), git, noReferences{}, workapp.ChangelogRequest{}, targets)
	if err != nil || len(sections) != 2 || len(sections[1].Warnings) != 1 {
		t.Fatalf("sections = %+v, err = %v", sections, err)
	}
}
