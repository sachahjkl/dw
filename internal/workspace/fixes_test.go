package workspace

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/sachahjkl/dw/internal/agent"
)

type recordingGit struct {
	GitPort
	failAt      int
	prepared    int
	removed     []string
	removeCtxOK bool
}

func (git *recordingGit) PrepareWorktree(_ context.Context, request WorktreeRequest) (WorktreeResult, error) {
	git.prepared++
	if git.prepared == git.failAt {
		return WorktreeResult{}, errors.New("boom")
	}
	if err := os.MkdirAll(request.WorktreePath, 0o755); err != nil {
		return WorktreeResult{}, err
	}
	return WorktreeResult{Repository: request.Repository, WorktreePath: request.WorktreePath, GitDir: filepath.Join(request.ProjectRoot, "repositories", request.AnchorName), Created: true}, nil
}

func (git *recordingGit) WorktreeRemove(ctx context.Context, _ string, path string) error {
	git.removeCtxOK = ctx.Err() == nil
	git.removed = append(git.removed, path)
	return os.RemoveAll(path)
}

func TestValidatePathComponentRejectsWindowsInvalidNames(t *testing.T) {
	for _, value := range []string{"CON", "nul", "Com1", "lpt9.txt", "aux.md", "a:b", "a<b", "a|b", "a?b", "a*b", `a"b`, "name.", "name ", "tab\tname"} {
		if err := validatePathComponent("test", value); err == nil {
			t.Errorf("validatePathComponent(%q) accepted", value)
		}
	}
	for _, value := range []string{"front", "COM", "COM10", "console", "a.b", "feat-42"} {
		if err := validatePathComponent("test", value); err != nil {
			t.Errorf("validatePathComponent(%q) = %v", value, err)
		}
	}
}

func TestEnsurePathWithinRejectsSymlinkEscape(t *testing.T) {
	root, outside := t.TempDir(), t.TempDir()
	link := filepath.Join(root, "link")
	if err := os.Symlink(outside, link); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	if err := ensurePathWithin(root, filepath.Join(link, "child")); err == nil {
		t.Fatal("symlink escape accepted")
	}
	if err := ensureNotLink(link); err == nil {
		t.Fatal("ensureNotLink accepted a symlink")
	}
	if err := ensurePathWithin(root, filepath.Join(root, "missing", "child")); err != nil {
		t.Fatal(err)
	}
}

func TestBuildNamesWithoutIDsOrSlug(t *testing.T) {
	if got := BuildBranchName("feat", nil, "slug"); got != "feat/slug" {
		t.Errorf("branch = %q", got)
	}
	if got := BuildSubjectName("feat", []string{"42"}, ""); got != "feat-42" {
		t.Errorf("subject = %q", got)
	}
	if got := BuildBranchName("feat", []string{"42", "7"}, "x y"); got != "feat/42-7-x-y" {
		t.Errorf("branch = %q", got)
	}
}

func TestEnsureWorkItemReferenceUsesBoundary(t *testing.T) {
	manifest := Manifest{WorkItemID: "12"}
	if got := EnsureWorkItemReference("fix #123", manifest); got != "fix #123 #12" {
		t.Errorf("got %q", got)
	}
	if got := EnsureWorkItemReference("fix #12: done", manifest); got != "fix #12: done" {
		t.Errorf("got %q", got)
	}
}

func TestPlanStartWithItemsDoesNotAliasCallerSlice(t *testing.T) {
	ids := []string{"1", "2"}
	engine := NewEngine(nil, nil, nil, nil)
	_, _ = engine.PlanStartWithItems(context.Background(), StartRequest{Root: t.TempDir(), WorkItemIDs: ids[:1]}, []WorkItem{{ID: "9"}, {ID: "8"}})
	if ids[0] != "1" || ids[1] != "2" {
		t.Fatalf("caller slice mutated: %v", ids)
	}
}

func TestPlanScratchStartRejectsEscapingFolder(t *testing.T) {
	engine := NewEngine(scratchTestConfig{project: ProjectConfig{Key: "ha", Repositories: []RepositoryConfig{{Name: "front", Folder: "../escape"}}}}, nil, nil, nil)
	engine.IDs = scratchTestIDs("01K2ABCDEFGHJKMNPQRSTVWXYZ")
	if _, err := engine.PlanScratchStart(context.Background(), ScratchStartRequest{Root: t.TempDir(), Project: "ha", Title: "x"}); err == nil {
		t.Fatal("escaping folder accepted")
	}
}

func TestExecuteStartRejectsExistingWorkspaceAndRollsBackWithLiveContext(t *testing.T) {
	root := t.TempDir()
	git := &recordingGit{failAt: 2}
	engine := NewEngine(nil, git, nil, nil)
	plan, err := engine.PlanStart(context.Background(), StartRequest{Root: root, WorkItemIDs: []string{"42"}, Slug: "x", Repositories: []string{"front", "back"}})
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	emit := func(event ActionEvent) {
		if event.Type == "worktreePrepared" {
			cancel()
		}
	}
	if _, err := engine.ExecuteStart(ctx, plan, nil, nil, emit); err == nil {
		t.Fatal("expected failure")
	}
	if len(git.removed) != 1 || !git.removeCtxOK {
		t.Fatalf("rollback removed %v with live context %v", git.removed, git.removeCtxOK)
	}
	if _, err := os.Stat(plan.Workspace); !os.IsNotExist(err) {
		t.Fatalf("workspace left behind: %v", err)
	}
	if err := os.MkdirAll(plan.Workspace, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(plan.Workspace, "keep"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := engine.ExecuteStart(context.Background(), plan, nil, nil, nil); !errors.Is(err, ErrWorkspaceConflict) {
		t.Fatalf("err = %v", err)
	}
	if _, err := os.Stat(filepath.Join(plan.Workspace, "keep")); err != nil {
		t.Fatal("existing workspace was modified")
	}
}

func TestAddRepositoryAlreadyPresentIsNoOp(t *testing.T) {
	workspace := t.TempDir()
	manifest := Manifest{WorkItemID: "42", Project: "ha", Type: "feat", Slug: "x", BranchName: "feat/42-x", Repositories: []string{"front"}}
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		t.Fatal(err)
	}
	git := &recordingGit{}
	engine := NewEngine(nil, git, nil, nil)
	loaded, plan, err := engine.PlanAddRepository(context.Background(), t.TempDir(), workspace, "FRONT")
	if err != nil || !plan.AlreadyPresent {
		t.Fatalf("plan = %#v, err = %v", plan, err)
	}
	if _, err := engine.ExecuteAddRepository(context.Background(), loaded, plan); err != nil || git.prepared != 0 {
		t.Fatalf("err = %v, prepared = %d", err, git.prepared)
	}
}

func TestExecuteAddRepositoryRollsBackWorktree(t *testing.T) {
	workspace := t.TempDir()
	manifest := Manifest{WorkItemID: "42", Project: "ha", Type: "feat", Slug: "x", BranchName: "feat/42-x", Repositories: []string{"front"}}
	manifestPath := filepath.Join(workspace, ManifestFile)
	if err := WriteManifest(manifestPath, manifest); err != nil {
		t.Fatal(err)
	}
	original, _ := os.ReadFile(manifestPath)
	if err := os.WriteFile(filepath.Join(workspace, ".claude"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	git := &recordingGit{}
	engine := NewEngine(nil, git, nil, nil)
	plan := AddRepositoryPlan{Workspace: workspace, Repository: "back", ProjectRoot: t.TempDir(), WorktreePath: filepath.Join(workspace, "back"), AnchorName: "back.git", Repositories: []string{"front", "back"}}
	if _, err := engine.ExecuteAddRepository(context.Background(), manifest, plan); err == nil {
		t.Fatal("expected failure")
	}
	if len(git.removed) != 1 {
		t.Fatalf("removed = %v", git.removed)
	}
	if current, _ := os.ReadFile(manifestPath); string(current) != string(original) {
		t.Fatalf("manifest not restored: %s", current)
	}
}

func TestExecuteTeardownRejectsUnsafePlans(t *testing.T) {
	root := t.TempDir()
	workspace := filepath.Join(root, "projects", "ha", "workspaces", "feat-42-x")
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), Manifest{WorkItemID: "42", Project: "ha"}); err != nil {
		t.Fatal(err)
	}
	engine := NewEngine(nil, &recordingGit{}, nil, nil)
	valid := func() TeardownPlanReport {
		_, plan, err := engine.PlanTeardown(context.Background(), root, workspace)
		if err != nil {
			t.Fatal(err)
		}
		return plan
	}
	nested := valid()
	nestedPath := filepath.Join(root, "projects", "ha", "workspaces", "feat-42-x", "sub")
	if err := WriteManifest(filepath.Join(nestedPath, ManifestFile), Manifest{WorkItemID: "42", Project: "ha"}); err != nil {
		t.Fatal(err)
	}
	nested.Workspace = &nestedPath
	escaping := valid()
	escaping.Steps = append([]TeardownStep{{Action: TeardownAction{Type: "worktreeRemove", WorktreePath: root, GitDir: filepath.Join(root, "projects", "ha", "repositories", "x.git")}}}, escaping.Steps...)
	gitDir := valid()
	gitDir.Steps = append([]TeardownStep{{Action: TeardownAction{Type: "worktreeRemove", WorktreePath: filepath.Join(workspace, "front"), GitDir: root}}}, gitDir.Steps...)
	for name, plan := range map[string]TeardownPlanReport{"nested": nested, "worktree": escaping, "gitdir": gitDir} {
		if _, err := engine.ExecuteTeardown(context.Background(), plan, true); err == nil {
			t.Errorf("%s: unsafe teardown accepted", name)
		}
	}
	if _, err := os.Stat(workspace); err != nil {
		t.Fatalf("workspace removed: %v", err)
	}
}

func TestScratchPruneSkipsRepositoryWorktrees(t *testing.T) {
	root := t.TempDir()
	workspace := filepath.Join(root, "projects", "ha", "workspaces", "scratch-spike-x")
	manifest := Manifest{Kind: KindScratch, WorkspaceID: "01K2ABCDEFGHJKMNPQRSTVWXYZ", Title: "x", Project: "ha", Type: "spike", Slug: "x", BranchName: "spike/x", Repositories: []string{"front"}}
	if err := writeWorkspaceFiles(workspace, manifest, false); err != nil {
		t.Fatal(err)
	}
	old := time.Now().Add(-100 * 24 * time.Hour)
	fresh := filepath.Join(workspace, "front", "node_modules", "fresh.js")
	if err := os.MkdirAll(filepath.Dir(fresh), 0o755); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{filepath.Join(workspace, "front", ".git"), fresh} {
		if err := os.WriteFile(path, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	entries, _ := os.ReadDir(workspace)
	for _, entry := range entries {
		_ = os.Chtimes(filepath.Join(workspace, entry.Name()), old, old)
	}
	engine := NewEngine(scratchTestConfig{project: ProjectConfig{Key: "ha"}}, nil, nil, nil)
	candidates, err := engine.ScratchPruneCandidates(context.Background(), root, "ha", time.Now().Add(-30*24*time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	if len(candidates) != 1 {
		t.Fatalf("candidates = %#v", candidates)
	}
}

func TestWriteGeneratedFilesKeepsUserContent(t *testing.T) {
	workspace := t.TempDir()
	manifest := Manifest{WorkItemID: "42", Project: "ha"}
	path := filepath.Join(workspace, "AGENTS.md")
	if err := os.WriteFile(path, []byte("# Notes\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := WriteGeneratedFiles(workspace, manifest); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(path)
	if !strings.HasPrefix(string(data), "# Notes\n") || !strings.Contains(string(data), agent.BlockBegin) {
		t.Fatalf("AGENTS.md = %q", data)
	}
}
