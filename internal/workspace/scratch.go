package workspace

import (
	"context"
	"crypto/rand"
	"encoding/binary"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/l10n"
)

type randomULID struct{}

func (randomULID) NewID(now time.Time) (string, error) {
	var data [16]byte
	milliseconds := uint64(now.UnixMilli())
	data[0], data[1], data[2] = byte(milliseconds>>40), byte(milliseconds>>32), byte(milliseconds>>24)
	data[3], data[4], data[5] = byte(milliseconds>>16), byte(milliseconds>>8), byte(milliseconds)
	if _, err := rand.Read(data[6:]); err != nil {
		return "", err
	}
	const alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
	var output [26]byte
	hi := binary.BigEndian.Uint64(data[:8])
	lo := binary.BigEndian.Uint64(data[8:])
	for i := 25; i >= 0; i-- {
		output[i] = alphabet[lo&31]
		lo = lo>>5 | hi<<59
		hi >>= 5
	}
	return string(output[:]), nil
}

func ShortWorkspaceID(id string) string {
	id = strings.TrimSpace(id)
	if len(id) > 8 {
		return strings.ToLower(id[len(id)-8:])
	}
	return strings.ToLower(id)
}

func (e *Engine) PlanScratchStart(ctx context.Context, request ScratchStartRequest) (ScratchStartPlan, error) {
	project, title := strings.TrimSpace(request.Project), strings.TrimSpace(request.Title)
	if project == "" || title == "" {
		return ScratchStartPlan{}, fmt.Errorf("workspace scratch start requires --project and --title")
	}
	if err := validatePathComponent("project", project); err != nil {
		return ScratchStartPlan{}, err
	}
	configured, found, err := e.project(ctx, request.Root, project)
	if err != nil {
		return ScratchStartPlan{}, localizedOperation("load project configuration", err)
	}
	if !found {
		return ScratchStartPlan{}, fmt.Errorf("workspace scratch start requires a configured project: %s", project)
	}
	repositories := distinctCSV(request.Repositories)
	if len(repositories) == 0 {
		for _, repository := range configured.Repositories {
			repositories = append(repositories, repository.Name)
		}
	}
	if len(repositories) == 0 {
		return ScratchStartPlan{}, fmt.Errorf("workspace scratch start requires at least one configured repository")
	}
	clock := e.Clock
	if clock == nil {
		clock = realClock{}
	}
	ids := e.IDs
	if ids == nil {
		ids = randomULID{}
	}
	workspaceID, err := ids.NewID(clock.Now())
	if err != nil {
		return ScratchStartPlan{}, localizedOperation("generate workspace ID", err)
	}
	slug := SlugOrFallback(request.Slug, title)
	branch, subject := "spike/"+slug, "scratch-spike-"+slug
	workspace := filepath.Join(request.Root, "projects", project, "workspaces", subject)
	if _, statErr := os.Stat(workspace); statErr == nil {
		suffix := ShortWorkspaceID(workspaceID)
		branch, subject = branch+"-"+suffix, subject+"-"+suffix
		workspace = filepath.Join(request.Root, "projects", project, "workspaces", subject)
	}
	projectRoot := filepath.Join(request.Root, "projects", project)
	folders := make([]RepositoryFolder, 0, len(repositories))
	worktrees := make([]StartRepositoryPlan, 0, len(repositories))
	for _, name := range repositories {
		repository, ok := configured.Repository(name)
		if !ok {
			return ScratchStartPlan{}, localizedCause("workspace.error.missing-repository", ErrMissingRepository, l10n.A("repository", name))
		}
		normalizeRepositoryConfig(&repository, name)
		path := filepath.Join(workspace, repository.Folder)
		folders = append(folders, RepositoryFolder{Repository: name, Path: repository.Folder})
		worktrees = append(worktrees, StartRepositoryPlan{Repository: name, ProjectRoot: projectRoot, WorktreePath: path, HTTPURL: repository.HTTPURL, SSHURL: repository.SSHURL, DefaultBranch: repository.DefaultBranch, AnchorName: repository.AnchorName, GitCredentialSecret: repository.GitCredentialSecret, BranchName: branch})
	}
	if checker, ok := e.Git.(interface {
		BranchExists(context.Context, string, string) (bool, error)
	}); ok {
		collision := false
		for _, target := range worktrees {
			exists, checkErr := checker.BranchExists(ctx, filepath.Join(projectRoot, "repositories", target.AnchorName), branch)
			if checkErr != nil && !os.IsNotExist(checkErr) {
				return ScratchStartPlan{}, checkErr
			}
			collision = collision || exists
		}
		if collision && !strings.HasSuffix(subject, "-"+ShortWorkspaceID(workspaceID)) {
			suffix := ShortWorkspaceID(workspaceID)
			branch, subject = branch+"-"+suffix, subject+"-"+suffix
			workspace = filepath.Join(request.Root, "projects", project, "workspaces", subject)
			for i := range worktrees {
				worktrees[i].BranchName, worktrees[i].WorktreePath = branch, filepath.Join(workspace, folders[i].Path)
			}
		}
	}
	return ScratchStartPlan{WorkspaceID: workspaceID, Project: project, Title: title, Type: "spike", Slug: slug, BranchName: branch, SubjectName: subject, Workspace: workspace, Repositories: repositories, RepositoryFolders: folders, RepositoryWorktrees: worktrees}, nil
}

func (e *Engine) ExecuteScratchStart(ctx context.Context, plan ScratchStartPlan, emit func(ActionEvent)) (ScratchStartExecutionReport, error) {
	start := StartPlan{Project: plan.Project, Type: plan.Type, Slug: plan.Slug, BranchName: plan.BranchName, SubjectName: plan.SubjectName, Workspace: plan.Workspace, Repositories: plan.Repositories, RepositoryFolders: plan.RepositoryFolders, RepositoryWorktrees: plan.RepositoryWorktrees}
	if _, err := os.Stat(plan.Workspace); err == nil {
		return ScratchStartExecutionReport{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", plan.Workspace))
	}
	if e.Git == nil {
		return ScratchStartExecutionReport{}, ErrGitCapabilityRequired
	}
	prepared := make([]WorktreeResult, 0, len(start.RepositoryWorktrees))
	rollback := func() {
		for i := len(prepared) - 1; i >= 0; i-- {
			if prepared[i].Created && prepared[i].GitDir != "" {
				_ = e.Git.WorktreeRemove(ctx, prepared[i].GitDir, prepared[i].WorktreePath)
			}
		}
		_ = os.RemoveAll(plan.Workspace)
	}
	events := make([]ActionEvent, 0)
	for _, target := range start.RepositoryWorktrees {
		credential, err := e.gitCredential(ctx, target.GitCredentialSecret)
		if err != nil {
			rollback()
			return ScratchStartExecutionReport{}, err
		}
		pushEvent(&events, emit, ActionEvent{Type: "preparingWorktree", Repository: target.Repository})
		result, err := e.Git.PrepareWorktree(ctx, WorktreeRequest{ProjectRoot: target.ProjectRoot, Repository: target.Repository, HTTPURL: target.HTTPURL, SSHURL: target.SSHURL, DefaultBranch: target.DefaultBranch, AnchorName: target.AnchorName, BranchName: target.BranchName, WorktreePath: target.WorktreePath, Credential: credential})
		if err != nil {
			rollback()
			return ScratchStartExecutionReport{}, localizedDetail("workspace.error.worktree-preparation", err, l10n.A("repository", target.Repository))
		}
		prepared = append(prepared, result)
	}
	clock := e.Clock
	if clock == nil {
		clock = realClock{}
	}
	manifest := Manifest{Schema: 2, Kind: KindScratch, WorkspaceID: plan.WorkspaceID, Title: plan.Title, Project: plan.Project, Type: plan.Type, Slug: plan.Slug, BranchName: plan.BranchName, CreatedAt: clock.Now().UTC().Format(time.RFC3339), Repositories: append([]string(nil), plan.Repositories...), Status: "created"}
	if err := writeWorkspaceFiles(plan.Workspace, manifest, true); err != nil {
		rollback()
		return ScratchStartExecutionReport{}, localizedOperation("write scratch workspace files", err)
	}
	pushEvent(&events, emit, ActionEvent{Type: "workspaceCreated"})
	return ScratchStartExecutionReport{Plan: plan, Manifest: manifest, Events: events}, nil
}

func (e *Engine) PlanScratchPromotion(ctx context.Context, workspace string, target WorkItem, kind, slug string, createChildren bool, states []string) (Manifest, ScratchPromotionPlan, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, ScratchPromotionPlan{}, err
	}
	if manifest.Kind != KindScratch || strings.TrimSpace(target.ID) == "" {
		return Manifest{}, ScratchPromotionPlan{}, fmt.Errorf("workspace scratch promote requires a scratch workspace and target work item")
	}
	if strings.TrimSpace(kind) == "" {
		kind = "feat"
	}
	if strings.TrimSpace(slug) == "" && target.Title != nil {
		slug = *target.Title
	}
	slug = SlugOrFallback(slug, manifest.Slug)
	newBranch := BuildBranchName(kind, []string{target.ID}, slug)
	newWorkspace := filepath.Join(filepath.Dir(workspace), BuildSubjectName(kind, []string{target.ID}, slug))
	if newWorkspace != workspace {
		if _, err := os.Stat(newWorkspace); err == nil {
			return Manifest{}, ScratchPromotionPlan{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", newWorkspace))
		}
	}
	if checker, ok := e.Git.(interface {
		BranchExists(context.Context, string, string) (bool, error)
		CurrentBranch(context.Context, string) (string, error)
	}); ok {
		root, valid := Root(workspace)
		if !valid {
			return Manifest{}, ScratchPromotionPlan{}, fmt.Errorf("invalid workspace location: %s", workspace)
		}
		configured, _, configErr := e.project(ctx, root, manifest.Project)
		if configErr != nil {
			return Manifest{}, ScratchPromotionPlan{}, configErr
		}
		for _, name := range manifest.Repositories {
			repository, found := configured.Repository(name)
			if !found {
				return Manifest{}, ScratchPromotionPlan{}, fmt.Errorf("repository is not configured: %s", name)
			}
			normalizeRepositoryConfig(&repository, name)
			path := filepath.Join(workspace, repository.Folder)
			current, currentErr := checker.CurrentBranch(ctx, path)
			if currentErr != nil || current != manifest.BranchName {
				return Manifest{}, ScratchPromotionPlan{}, fmt.Errorf("repository %s must be on branch %s", name, manifest.BranchName)
			}
			exists, existsErr := checker.BranchExists(ctx, path, newBranch)
			if existsErr != nil {
				return Manifest{}, ScratchPromotionPlan{}, existsErr
			}
			if exists {
				return Manifest{}, ScratchPromotionPlan{}, fmt.Errorf("target branch already exists in %s: %s", name, newBranch)
			}
		}
	}
	return manifest, ScratchPromotionPlan{WorkspaceID: manifest.WorkspaceID, Workspace: workspace, NewWorkspace: newWorkspace, Target: target, Type: kind, Slug: slug, OldBranch: manifest.BranchName, NewBranch: newBranch, Repositories: append([]string(nil), manifest.Repositories...), CreateChildTasks: createChildren, StateUpdates: states}, nil
}

func (e *Engine) ExecuteScratchPromotionLocal(ctx context.Context, manifest Manifest, plan ScratchPromotionPlan) (ScratchPromotionExecutionReport, error) {
	brancher, ok := e.Git.(interface {
		RenameBranch(context.Context, string, string, string) error
	})
	if !ok && len(plan.Repositories) > 0 {
		return ScratchPromotionExecutionReport{Plan: plan}, fmt.Errorf("git branch rename capability is required for scratch promotion")
	}
	root, ok := Root(plan.Workspace)
	if !ok {
		return ScratchPromotionExecutionReport{Plan: plan}, fmt.Errorf("invalid workspace location: %s", plan.Workspace)
	}
	project, _, err := e.project(ctx, root, manifest.Project)
	if err != nil {
		return ScratchPromotionExecutionReport{Plan: plan}, err
	}
	renamed := make([]string, 0)
	backups := make(map[string][]byte)
	paths := []string{ManifestFile, PlanFile}
	for _, name := range manifest.Repositories {
		paths = append(paths, HandoffPrefix+name+".md")
	}
	for _, file := range AgentFiles(manifest) {
		paths = append(paths, file.RelativePath)
	}
	for _, relative := range paths {
		if data, readErr := os.ReadFile(filepath.Join(plan.Workspace, relative)); readErr == nil {
			backups[relative] = data
		}
	}
	rollback := func() {
		base := plan.Workspace
		if _, err := os.Stat(plan.NewWorkspace); err == nil && plan.NewWorkspace != plan.Workspace {
			_ = os.Rename(plan.NewWorkspace, plan.Workspace)
		}
		for i := len(renamed) - 1; i >= 0; i-- {
			repository, _ := project.Repository(renamed[i])
			normalizeRepositoryConfig(&repository, renamed[i])
			_ = brancher.RenameBranch(ctx, filepath.Join(base, repository.Folder), plan.NewBranch, plan.OldBranch)
		}
		for relative, data := range backups {
			_ = writeFileAtomic(filepath.Join(plan.Workspace, relative), data, 0o644)
		}
	}
	for _, name := range plan.Repositories {
		repository, _ := project.Repository(name)
		normalizeRepositoryConfig(&repository, name)
		if err := brancher.RenameBranch(ctx, filepath.Join(plan.Workspace, repository.Folder), plan.OldBranch, plan.NewBranch); err != nil {
			rollback()
			return ScratchPromotionExecutionReport{Plan: plan, LocalEffects: renamed}, err
		}
		renamed = append(renamed, name)
	}
	if plan.NewWorkspace != plan.Workspace {
		if err := os.Rename(plan.Workspace, plan.NewWorkspace); err != nil {
			rollback()
			return ScratchPromotionExecutionReport{Plan: plan, LocalEffects: renamed}, err
		}
	}
	updated := manifest
	updated.Kind, updated.WorkItemID, updated.Type, updated.Slug, updated.BranchName = KindTracked, plan.Target.ID, plan.Type, plan.Slug, plan.NewBranch
	updated.WorkItemType, updated.WorkItemTitle, updated.WorkItemState = cloneString(plan.Target.Type), cloneString(plan.Target.Title), cloneString(plan.Target.State)
	updated.WorkItems = []WorkItem{plan.Target}
	if err := writeWorkspaceFiles(plan.NewWorkspace, updated, true); err != nil {
		rollback()
		return ScratchPromotionExecutionReport{Plan: plan, LocalEffects: renamed}, err
	}
	effects := append([]string(nil), renamed...)
	effects = append(effects, "workspace", "manifest", "generated-files")
	return ScratchPromotionExecutionReport{Plan: plan, Manifest: updated, LocalEffects: effects}, nil
}

func ScratchNotApplicable(operation, workspace string) error {
	return fmt.Errorf("%s is not applicable to an unpromoted scratch workspace %s; run: dw workspace scratch promote <WORK_ITEM_ID> --workspace %s --execute", operation, workspace, workspace)
}

func (e *Engine) ScratchPruneCandidates(ctx context.Context, root, project string, cutoff time.Time) ([]Summary, error) {
	kind := KindScratch
	items := FilterKind(Discover(root), project, nil, &kind)
	result := make([]Summary, 0)
	for _, item := range items {
		latest := time.Time{}
		err := filepath.WalkDir(item.Path, func(path string, entry os.DirEntry, err error) error {
			if err != nil {
				return err
			}
			if entry.IsDir() && entry.Name() == ".git" {
				return filepath.SkipDir
			}
			if entry.IsDir() {
				return nil
			}
			info, err := entry.Info()
			if err == nil && info.ModTime().After(latest) {
				latest = info.ModTime()
			}
			return err
		})
		if err != nil {
			return nil, err
		}
		if commits, ok := e.Git.(interface {
			LastCommitTime(context.Context, string) (time.Time, bool, error)
		}); ok {
			configured, _, configErr := e.project(ctx, root, item.Manifest.Project)
			if configErr != nil {
				return nil, configErr
			}
			for _, name := range item.Manifest.Repositories {
				repository, _ := configured.Repository(name)
				normalizeRepositoryConfig(&repository, name)
				value, found, commitErr := commits.LastCommitTime(ctx, filepath.Join(item.Path, repository.Folder))
				if commitErr != nil {
					return nil, commitErr
				}
				if found && value.After(latest) {
					latest = value
				}
			}
		}
		item.ActivityAt = latest.UTC().Format(time.RFC3339)
		if latest.Before(cutoff) {
			result = append(result, item)
		}
	}
	return result, nil
}
