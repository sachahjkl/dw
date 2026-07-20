package workspace

import (
	"context"
	"github.com/sachahjkl/dw/internal/l10n"
	"os"
	"path/filepath"
	"strings"
)

func (e *Engine) PlanAddRepository(ctx context.Context, root, workspace, name string) (Manifest, AddRepositoryPlan, error) {
	name = strings.TrimSpace(name)
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, AddRepositoryPlan{}, err
	}
	for _, existing := range manifest.Repositories {
		if equalFold(existing, name) {
			return manifest, AddRepositoryPlan{Workspace: workspace, Repository: name, ProjectRoot: filepath.Join(root, "projects", manifest.Project), WorktreePath: filepath.Join(workspace, name), DefaultBranch: "main", AnchorName: name + ".git", BranchName: manifest.BranchName, Repositories: append([]string(nil), manifest.Repositories...)}, nil
		}
	}
	project, found, err := e.project(ctx, root, manifest.Project)
	if err != nil {
		return Manifest{}, AddRepositoryPlan{}, localizedOperation("load project configuration", err)
	}
	if !found {
		return Manifest{}, AddRepositoryPlan{}, localizedCause("workspace.error.missing-repository", ErrMissingRepository, l10n.A("repository", name))
	}
	repository, ok := project.Repository(name)
	if !ok {
		return Manifest{}, AddRepositoryPlan{}, localizedCause("workspace.error.missing-repository", ErrMissingRepository, l10n.A("repository", name))
	}
	normalizeRepositoryConfig(&repository, name)
	repositories := appendDistinct(append([]string(nil), manifest.Repositories...), name)
	return manifest, AddRepositoryPlan{Workspace: workspace, Repository: name, ProjectRoot: filepath.Join(root, "projects", manifest.Project), WorktreePath: filepath.Join(workspace, repository.Folder), HTTPURL: repository.HTTPURL, SSHURL: repository.SSHURL, DefaultBranch: repository.DefaultBranch, AnchorName: repository.AnchorName, GitCredentialSecret: repository.GitCredentialSecret, BranchName: manifest.BranchName, Repositories: repositories}, nil
}
func (e *Engine) ExecuteAddRepository(ctx context.Context, manifest Manifest, plan AddRepositoryPlan) (AddRepositoryReport, error) {
	if e.Git == nil {
		return AddRepositoryReport{}, ErrGitCapabilityRequired
	}
	credential, err := e.gitCredential(ctx, plan.GitCredentialSecret)
	if err != nil {
		return AddRepositoryReport{}, err
	}
	worktree, err := e.Git.PrepareWorktree(ctx, WorktreeRequest{ProjectRoot: plan.ProjectRoot, Repository: plan.Repository, HTTPURL: plan.HTTPURL, SSHURL: plan.SSHURL, DefaultBranch: plan.DefaultBranch, AnchorName: plan.AnchorName, BranchName: plan.BranchName, WorktreePath: plan.WorktreePath, Credential: credential})
	if err != nil {
		return AddRepositoryReport{}, localizedOperation("prepare repository worktree", err)
	}
	updated := manifest
	updated.Repositories = append([]string(nil), plan.Repositories...)
	if err = WriteManifest(filepath.Join(plan.Workspace, ManifestFile), updated); err != nil {
		return AddRepositoryReport{}, localizedOperation("write repository manifest", err)
	}
	if err = writeFileAtomic(filepath.Join(plan.Workspace, HandoffPrefix+plan.Repository+".md"), []byte(HandoffMarkdown(updated, plan.Repository)), 0o644); err != nil {
		return AddRepositoryReport{}, localizedOperation("write repository handoff", err)
	}
	if err = WriteGeneratedFiles(plan.Workspace, updated); err != nil {
		return AddRepositoryReport{}, localizedOperation("write generated agent files", err)
	}
	return AddRepositoryReport{Plan: plan, Worktree: worktree, Manifest: updated}, nil
}
func (e *Engine) AddRepositoryChoices(ctx context.Context, root, workspace string) ([]string, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return nil, err
	}
	project, found, err := e.project(ctx, root, manifest.Project)
	if err != nil || !found {
		return []string{}, err
	}
	result := make([]string, 0)
	for _, repository := range project.Repositories {
		exists := false
		for _, name := range manifest.Repositories {
			if equalFold(name, repository.Name) {
				exists = true
				break
			}
		}
		if !exists {
			result = append(result, repository.Name)
		}
	}
	return result, nil
}

func (e *Engine) PlanRepositoryLatest(ctx context.Context, root, workspace string, requested []string) (Manifest, []RepositoryTarget, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, nil, err
	}
	names := requested
	if len(names) == 0 {
		names = manifest.Repositories
	} else {
		for _, name := range names {
			if !containsFold(manifest.Repositories, name) {
				return Manifest{}, nil, localizedCause("workspace.error.missing-repository", ErrMissingRepository, l10n.A("repository", name))
			}
		}
	}
	project, _, err := e.project(ctx, root, manifest.Project)
	if err != nil {
		return Manifest{}, nil, localizedOperation("load project configuration", err)
	}
	targets := make([]RepositoryTarget, 0, len(names))
	for _, name := range names {
		repository, ok := project.Repository(name)
		if !ok {
			repository = RepositoryConfig{Name: name}
		}
		normalizeRepositoryConfig(&repository, name)
		targets = append(targets, RepositoryTarget{Repository: name, Path: filepath.Join(workspace, repository.Folder), DefaultBranch: repository.DefaultBranch, SSHURL: repository.SSHURL, GitCredentialSecret: repository.GitCredentialSecret})
	}
	return manifest, targets, nil
}
func (e *Engine) ExecuteRepositoryLatest(ctx context.Context, targets []RepositoryTarget) ([]RepositoryTarget, error) {
	if e.Git == nil {
		return nil, ErrGitCapabilityRequired
	}
	updated := make([]RepositoryTarget, 0, len(targets))
	for _, target := range targets {
		credential, err := e.gitCredential(ctx, target.GitCredentialSecret)
		if err != nil {
			return nil, err
		}
		if err = e.Git.Update(ctx, target.Path, target.DefaultBranch, credential, target.SSHURL); err != nil {
			return nil, localizedOperation("update repository", err)
		}
		updated = append(updated, target)
	}
	return updated, nil
}

func (e *Engine) PlanCommit(ctx context.Context, root, workspace, message string) (CommitPlanReport, error) {
	manifest, targets, err := e.commitTargets(ctx, root, workspace)
	if err != nil {
		return CommitPlanReport{}, err
	}
	statuses := make([]TargetStatus, 0, len(targets))
	for _, target := range targets {
		status := RepositoryStatus{}
		if e.Git != nil {
			status, err = e.Git.Status(ctx, target.Path)
			if err != nil {
				return CommitPlanReport{}, localizedOperation("read repository status", err)
			}
		}
		statuses = append(statuses, TargetStatus{Target: target, Status: status})
	}
	return CommitPlanReport{Workspace: workspace, BranchName: manifest.BranchName, Message: BuildCommitMessage(manifest, message), Targets: statuses}, nil
}
func (e *Engine) ExecuteCommit(ctx context.Context, plan CommitPlanReport) (CommitExecutionReport, error) {
	if e.Git == nil {
		return CommitExecutionReport{}, ErrGitCapabilityRequired
	}
	committed := make([]string, 0)
	for _, item := range plan.Targets {
		if !item.Status.IsGitRepository || !item.Status.HasChanges {
			continue
		}
		if err := e.Git.Commit(ctx, item.Target.Path, plan.Message); err != nil {
			return CommitExecutionReport{}, localizedOperation("commit repository", err)
		}
		committed = append(committed, item.Target.Repository)
	}
	return CommitExecutionReport{Workspace: plan.Workspace, BranchName: plan.BranchName, Message: plan.Message, Committed: committed}, nil
}
func (e *Engine) commitTargets(ctx context.Context, root, workspace string) (Manifest, []RepositoryTarget, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, nil, err
	}
	project, _, err := e.project(ctx, root, manifest.Project)
	if err != nil {
		return Manifest{}, nil, localizedOperation("load project configuration", err)
	}
	targets := make([]RepositoryTarget, 0, len(manifest.Repositories))
	for _, name := range manifest.Repositories {
		repository, ok := project.Repository(name)
		if !ok {
			repository = RepositoryConfig{Name: name}
		}
		normalizeRepositoryConfig(&repository, name)
		targets = append(targets, RepositoryTarget{Repository: name, Path: filepath.Join(workspace, repository.Folder)})
	}
	return manifest, targets, nil
}
func BuildCommitMessage(manifest Manifest, override string) string {
	override = strings.TrimSpace(override)
	if override != "" {
		return EnsureWorkItemReference(override, manifest)
	}
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = appendDistinct(ids, "#"+item.ID)
	}
	if manifest.TaskID != nil {
		ids = appendDistinct(ids, "#"+*manifest.TaskID)
	}
	for _, task := range manifest.NormalizedChildTasks() {
		ids = appendDistinct(ids, "#"+task.ID)
	}
	return strings.ToLower(manifest.Type) + "(" + strings.Join(ids, " ") + "): " + manifest.Slug
}
func EnsureWorkItemReference(message string, manifest Manifest) string {
	ids := make([]string, 0)
	if manifest.TaskID != nil {
		ids = appendDistinct(ids, *manifest.TaskID)
	}
	ids = appendDistinct(ids, manifest.WorkItemID)
	for _, item := range manifest.ParentWorkItems() {
		ids = appendDistinct(ids, item.ID)
	}
	for _, task := range manifest.NormalizedChildTasks() {
		ids = appendDistinct(ids, task.ID)
	}
	for _, id := range ids {
		if strings.Contains(message, "#"+id) {
			return message
		}
	}
	if len(ids) > 0 {
		return message + " #" + ids[0]
	}
	return message
}

func (e *Engine) PlanTeardown(ctx context.Context, root, workspace string) (Manifest, TeardownPlanReport, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, TeardownPlanReport{}, err
	}
	project, _, err := e.project(ctx, root, manifest.Project)
	if err != nil {
		return Manifest{}, TeardownPlanReport{}, localizedOperation("load project configuration", err)
	}
	steps := make([]TeardownStep, 0)
	projectRoot := filepath.Join(root, "projects", manifest.Project)
	for _, name := range manifest.Repositories {
		repository, ok := project.Repository(name)
		if !ok {
			repository = RepositoryConfig{Name: name}
		}
		normalizeRepositoryConfig(&repository, name)
		gitDir := filepath.Join(projectRoot, "repositories", repository.AnchorName)
		subject := TeardownSubject{Type: "repository", Repository: name}
		steps = append(steps, TeardownStep{Subject: subject, Action: TeardownAction{Type: "worktreeRemove", WorktreePath: filepath.Join(workspace, repository.Folder), GitDir: gitDir}})
		if strings.TrimSpace(repository.HTTPURL) != "" {
			steps = append(steps, TeardownStep{Subject: subject, Action: TeardownAction{Type: "worktreePrune", GitDir: gitDir}})
		}
	}
	steps = append(steps, TeardownStep{Subject: TeardownSubject{Type: "workspace"}, Action: TeardownAction{Type: "deleteWorkspace", Workspace: workspace}})
	workspaceCopy := workspace
	return manifest, TeardownPlanReport{Workspace: &workspaceCopy, Steps: steps}, nil
}
func (e *Engine) ExecuteTeardown(ctx context.Context, plan TeardownPlanReport, approved bool) (TeardownExecutionReport, error) {
	if !approved {
		return TeardownExecutionReport{}, ErrApprovalRequired
	}
	if plan.Workspace == nil || strings.TrimSpace(*plan.Workspace) == "" {
		return TeardownExecutionReport{}, ErrNoWorkspace
	}
	workspace := filepath.Clean(*plan.Workspace)
	if _, err := ReadManifest(filepath.Join(workspace, ManifestFile)); err != nil {
		return TeardownExecutionReport{}, err
	}
	if e.Git == nil {
		return TeardownExecutionReport{}, ErrGitCapabilityRequired
	}
	for _, step := range plan.Steps {
		if step.Action.Type != "worktreeRemove" {
			continue
		}
		if strings.TrimSpace(step.Action.GitDir) == "" {
			return TeardownExecutionReport{}, localized("workspace.error.teardown-missing-gitdir", l10n.A("repository", step.Subject.Repository))
		}
		if _, err := os.Stat(step.Action.GitDir); err != nil {
			return TeardownExecutionReport{}, localizedCause("workspace.error.teardown-gitdir-not-found", err, l10n.A("repository", step.Subject.Repository), l10n.A("path", step.Action.GitDir))
		}
		if err := e.Git.WorktreeRemove(ctx, step.Action.GitDir, step.Action.WorktreePath); err != nil {
			return TeardownExecutionReport{}, localizedDetail("workspace.error.teardown-operation", err, l10n.A("repository", step.Subject.Repository))
		}
	}
	for _, step := range plan.Steps {
		if step.Action.Type != "worktreePrune" {
			continue
		}
		if _, err := os.Stat(step.Action.GitDir); os.IsNotExist(err) {
			continue
		}
		if err := e.Git.WorktreePrune(ctx, step.Action.GitDir); err != nil {
			return TeardownExecutionReport{}, localizedDetail("workspace.error.teardown-operation", err, l10n.A("repository", step.Subject.Repository))
		}
	}
	if err := os.RemoveAll(workspace); err != nil {
		return TeardownExecutionReport{}, localizedOperation("delete workspace", err)
	}
	return TeardownExecutionReport{Workspace: workspace, Steps: append([]TeardownStep(nil), plan.Steps...)}, nil
}

func (e *Engine) PlanPrune(ctx context.Context, root string, project *string, ids []string, sync bool) (PrunePlanReport, error) {
	filterProject := ""
	if project != nil {
		filterProject = *project
	}
	syncReports := make([]PruneSyncReport, 0)
	if sync {
		for _, workspace := range Filter(Discover(root), filterProject, ids) {
			if e.Work == nil {
				syncReports = append(syncReports, PruneSyncReport{Workspace: workspace.Path, Status: "skipped", Detail: PruneSyncDetail{Kind: "auth-unavailable", Error: ErrWorkCapabilityRequired.Error()}})
				continue
			}
			report, err := e.Sync(ctx, workspace.Path)
			if err != nil {
				syncReports = append(syncReports, PruneSyncReport{Workspace: workspace.Path, Status: "skipped", Detail: PruneSyncDetail{Kind: "sync-failed", Error: err.Error()}})
			} else {
				syncReports = append(syncReports, PruneSyncReport{Workspace: workspace.Path, Status: "synced", Detail: PruneSyncDetail{Kind: "synced", WorkItems: report.Manifest.ParentWorkItems()}})
			}
		}
	}
	return PrunePlanReport{Root: root, Project: project, WorkItemIDs: append([]string(nil), ids...), Sync: syncReports, Candidates: PruneCandidates(root, filterProject, ids)}, nil
}
func (e *Engine) ExecutePrune(ctx context.Context, plan PrunePlanReport, selected []string, approved bool) (PruneExecutionReport, error) {
	if !approved {
		return PruneExecutionReport{}, ErrApprovalRequired
	}
	allowed := make(map[string]Summary, len(plan.Candidates))
	for _, candidate := range plan.Candidates {
		allowed[filepath.Clean(candidate.Path)] = candidate
	}
	if len(selected) == 0 {
		for path := range allowed {
			selected = append(selected, path)
		}
	}
	deleted := make([]string, 0, len(selected))
	for _, path := range selected {
		candidate, ok := allowed[filepath.Clean(path)]
		if !ok {
			return PruneExecutionReport{}, localized("workspace.error.prune-invalid-selection", l10n.A("path", path))
		}
		_, teardown, err := e.PlanTeardown(ctx, plan.Root, candidate.Path)
		if err != nil {
			return PruneExecutionReport{}, err
		}
		if _, err = e.ExecuteTeardown(ctx, teardown, true); err != nil {
			return PruneExecutionReport{}, err
		}
		deleted = append(deleted, candidate.Path)
	}
	return PruneExecutionReport{Root: plan.Root, Deleted: deleted}, nil
}

func containsFold(values []string, value string) bool {
	for _, item := range values {
		if equalFold(item, value) {
			return true
		}
	}
	return false
}

func (e *Engine) PlanRepositoryLatestReport(ctx context.Context, root, workspace string, requested []string) (RepositoryLatestPlanReport, error) {
	manifest, targets, err := e.PlanRepositoryLatest(ctx, root, workspace, requested)
	if err != nil {
		return RepositoryLatestPlanReport{}, err
	}
	reportTargets := make([]RepositoryLatestTarget, 0, len(targets))
	for _, target := range targets {
		var secret *string
		if target.GitCredentialSecret != "" {
			value := target.GitCredentialSecret
			secret = &value
		}
		reportTargets = append(reportTargets, RepositoryLatestTarget{Repository: target.Repository, RepositoryPath: target.Path, DefaultBranch: target.DefaultBranch, SSHURL: target.SSHURL, GitCredentialSecret: secret})
	}
	return RepositoryLatestPlanReport{Workspace: workspace, BranchName: manifest.BranchName, Targets: reportTargets}, nil
}

func (e *Engine) ExecuteRepositoryLatestReport(ctx context.Context, plan RepositoryLatestPlanReport) (RepositoryLatestExecutionReport, error) {
	targets := make([]RepositoryTarget, 0, len(plan.Targets))
	for _, target := range plan.Targets {
		secret := ""
		if target.GitCredentialSecret != nil {
			secret = *target.GitCredentialSecret
		}
		targets = append(targets, RepositoryTarget{Repository: target.Repository, Path: target.RepositoryPath, DefaultBranch: target.DefaultBranch, SSHURL: target.SSHURL, GitCredentialSecret: secret})
	}
	updated, err := e.ExecuteRepositoryLatest(ctx, targets)
	if err != nil {
		return RepositoryLatestExecutionReport{}, err
	}
	report := RepositoryLatestExecutionReport{Workspace: plan.Workspace, BranchName: plan.BranchName, Updated: make([]RepositoryLatestUpdate, 0, len(updated))}
	for _, target := range updated {
		report.Updated = append(report.Updated, RepositoryLatestUpdate{Repository: target.Repository, Path: target.Path, DefaultBranch: target.DefaultBranch})
	}
	return report, nil
}
