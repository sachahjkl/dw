package workspace

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"
)

type scratchTestConfig struct{ project ProjectConfig }

func (config scratchTestConfig) Project(context.Context, string, string) (ProjectConfig, bool, error) {
	return config.project, true, nil
}
func (scratchTestConfig) Workflow(context.Context, string) (WorkflowConfig, error) {
	return WorkflowConfig{}, nil
}

type scratchTestIDs string

func (ids scratchTestIDs) NewID(time.Time) (string, error) { return string(ids), nil }

func TestScratchManifestRoundTripHasNoFakeWorkItem(t *testing.T) {
	path := filepath.Join(t.TempDir(), ManifestFile)
	original := Manifest{Kind: KindScratch, WorkspaceID: "01K2ABCDEFGHJKMNPQRSTVWXYZ", Title: "Cache strategy", Project: "ha", Type: "spike", Slug: "cache-strategy", BranchName: "spike/cache-strategy", Repositories: []string{"front"}}
	if err := WriteManifest(path, original); err != nil {
		t.Fatal(err)
	}
	loaded, err := ReadManifest(path)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Schema != 2 || loaded.Kind != KindScratch || loaded.WorkspaceID != original.WorkspaceID {
		t.Fatalf("loaded manifest = %#v", loaded)
	}
	if loaded.PrimaryWorkItemID() != "" || len(loaded.ParentWorkItems()) != 0 || len(loaded.AllKnownWorkItemIDs()) != 0 {
		t.Fatalf("scratch has fabricated work items: %#v", loaded.ParentWorkItems())
	}
}

func TestLegacyManifestDefaultsToTracked(t *testing.T) {
	path := filepath.Join(t.TempDir(), ManifestFile)
	if err := os.WriteFile(path, []byte(`{"schema":1,"workItemId":"42","project":"ha","repositories":[]}`), 0o644); err != nil {
		t.Fatal(err)
	}
	loaded, err := ReadManifest(path)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Kind != KindTracked || loaded.Schema != 1 {
		t.Fatalf("legacy manifest = %#v", loaded)
	}
}

func TestPlanScratchStartUsesReadableNamesAndConfiguredRepositories(t *testing.T) {
	root := t.TempDir()
	engine := NewEngine(scratchTestConfig{project: ProjectConfig{Key: "ha", Repositories: []RepositoryConfig{{Name: "front", Folder: "ui", DefaultBranch: "main"}}}}, nil, nil, nil)
	engine.IDs = scratchTestIDs("01K2ABCDEFGHJKMNPQRSTVWXYZ")
	plan, err := engine.PlanScratchStart(context.Background(), ScratchStartRequest{Root: root, Project: "ha", Title: "Test a cache strategy"})
	if err != nil {
		t.Fatal(err)
	}
	if plan.Slug != "test-a-cache-strategy" || plan.BranchName != "spike/test-a-cache-strategy" || plan.SubjectName != "scratch-spike-test-a-cache-strategy" {
		t.Fatalf("plan naming = %#v", plan)
	}
	if len(plan.Repositories) != 1 || plan.Repositories[0] != "front" || plan.RepositoryFolders[0].Path != "ui" {
		t.Fatalf("plan repositories = %#v", plan)
	}
}

func TestScratchPreflightDoesNotRequireProviderContext(t *testing.T) {
	root := t.TempDir()
	workspace := filepath.Join(root, "projects", "ha", "workspaces", "scratch-spike-cache")
	manifest := Manifest{Kind: KindScratch, WorkspaceID: "01K2ABCDEFGHJKMNPQRSTVWXYZ", Title: "Cache", Project: "ha", Type: "spike", Slug: "cache", BranchName: "spike/cache", Repositories: []string{"front"}}
	if err := writeWorkspaceFiles(workspace, manifest, false); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(workspace, "front"), 0o755); err != nil {
		t.Fatal(err)
	}
	engine := NewEngine(scratchTestConfig{project: ProjectConfig{Key: "ha", Repositories: []RepositoryConfig{{Name: "front", Folder: "front"}}}}, nil, nil, nil)
	report, err := engine.BuildScratchPreflight(context.Background(), root, workspace)
	if err != nil {
		t.Fatal(err)
	}
	if report.HasBlockingIssues {
		t.Fatalf("scratch preflight issues = %#v", report.Issues)
	}
}
