package workspace

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/gitrepo"
	"github.com/sachahjkl/dw/internal/l10n"
)

func (e *Engine) PlanStart(ctx context.Context, request StartRequest) (StartPlan, error) {
	project := strings.TrimSpace(request.Project)
	if project == "" {
		project = "default"
	}
	ids := distinctCSV(request.WorkItemIDs)
	if len(ids) == 0 {
		return StartPlan{}, localized("workspace.error.work-item-required")
	}
	for _, workspace := range Filter(Discover(request.Root), project, nil) {
		for _, id := range ids {
			if id != "" && workspace.Manifest.MatchesWorkItem(id) {
				return StartPlan{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", id+" in "+workspace.Path))
			}
		}
	}
	kind := strings.ToLower(strings.TrimSpace(request.Type))
	if kind == "" {
		kind = "feat"
	}
	slug := SlugOrFallback(request.Slug, "work item "+ids[0])
	branchIDs := append([]string(nil), ids...)
	if request.TaskID != nil {
		branchIDs = appendDistinct(branchIDs, *request.TaskID)
	}
	config, found, err := e.project(ctx, request.Root, project)
	if err != nil {
		return StartPlan{}, localizedOperation("load project configuration", err)
	}
	repositories := distinctCSV(request.Repositories)
	if len(repositories) == 0 && found {
		for _, repository := range config.Repositories {
			repositories = appendDistinct(repositories, repository.Name)
		}
	}
	if len(repositories) == 0 {
		repositories = []string{"front", "back"}
	}
	subject := BuildSubjectName(kind, ids, slug)
	branch := BuildBranchName(kind, branchIDs, slug)
	workspace := filepath.Join(request.Root, "projects", project, "workspaces", subject)
	projectRoot := filepath.Join(request.Root, "projects", project)
	folders := make([]RepositoryFolder, 0, len(repositories))
	worktrees := make([]StartRepositoryPlan, 0, len(repositories))
	for _, name := range repositories {
		repository, ok := config.Repository(name)
		if !ok {
			repository = RepositoryConfig{Name: name, DefaultBranch: "main", Folder: name}
		}
		normalizeRepositoryConfig(&repository, name)
		path := filepath.Join(workspace, repository.Folder)
		folders = append(folders, RepositoryFolder{Repository: name, Path: repository.Folder})
		worktrees = append(worktrees, StartRepositoryPlan{Repository: name, ProjectRoot: projectRoot, WorktreePath: path, HTTPURL: repository.HTTPURL, SSHURL: repository.SSHURL, DefaultBranch: repository.DefaultBranch, AnchorName: repository.AnchorName, GitCredentialSecret: repository.GitCredentialSecret, BranchName: branch})
	}
	return StartPlan{WorkItemIDs: ids, PrimaryWorkItemID: ids[0], Project: project, TaskID: request.TaskID, Type: kind, Slug: slug, BranchName: branch, SubjectName: subject, Workspace: workspace, Repositories: repositories, RepositoryFolders: folders, RepositoryWorktrees: worktrees}, nil
}

func (e *Engine) PlanStartWithItems(ctx context.Context, request StartRequest, items []WorkItem) (StartPlan, error) {
	if strings.TrimSpace(request.Slug) == "" && len(items) > 0 && items[0].Title != nil {
		request.Slug = *items[0].Title
	}
	if len(items) > 0 {
		request.WorkItemIDs = request.WorkItemIDs[:0]
		for _, item := range items {
			request.WorkItemIDs = append(request.WorkItemIDs, item.ID)
		}
	}
	return e.PlanStart(ctx, request)
}

func (e *Engine) ExecuteStart(ctx context.Context, plan StartPlan, workItems []WorkItem, childTasks []ChildTask, emit func(ActionEvent)) (StartExecutionReport, error) {
	if _, err := os.Stat(plan.Workspace); err == nil {
		return StartExecutionReport{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", plan.Workspace))
	} else if !os.IsNotExist(err) {
		return StartExecutionReport{}, localizedOperation("inspect workspace", err)
	}
	if e.Git == nil && len(plan.RepositoryWorktrees) > 0 {
		return StartExecutionReport{}, ErrGitCapabilityRequired
	}
	events := make([]ActionEvent, 0)
	pushEvent := func(event ActionEvent) {
		events = append(events, event)
		if emit != nil {
			emit(event)
		}
	}
	prepared := make([]WorktreeResult, 0, len(plan.RepositoryWorktrees))
	rollback := func() {
		for index := len(prepared) - 1; index >= 0; index-- {
			item := prepared[index]
			if item.Created && item.GitDir != "" {
				_ = e.Git.WorktreeRemove(ctx, item.GitDir, item.WorktreePath)
			}
		}
		_ = os.RemoveAll(plan.Workspace)
	}
	for _, target := range plan.RepositoryWorktrees {
		credential, err := e.gitCredential(ctx, target.GitCredentialSecret)
		if err != nil {
			rollback()
			return StartExecutionReport{}, localizedDetail("workspace.error.worktree-preparation", err, l10n.A("repository", target.Repository))
		}
		pushEvent(ActionEvent{Type: "preparingWorktree", Repository: target.Repository})
		result, err := e.Git.PrepareWorktree(ctx, WorktreeRequest{ProjectRoot: target.ProjectRoot, Repository: target.Repository, HTTPURL: target.HTTPURL, SSHURL: target.SSHURL, DefaultBranch: target.DefaultBranch, AnchorName: target.AnchorName, BranchName: target.BranchName, WorktreePath: target.WorktreePath, Credential: credential})
		if err != nil {
			rollback()
			return StartExecutionReport{}, localizedDetail("workspace.error.worktree-preparation", err, l10n.A("repository", target.Repository))
		}
		prepared = append(prepared, result)
		pushEvent(ActionEvent{Type: "worktreePrepared", Repository: target.Repository})
	}
	if len(workItems) == 0 {
		for _, id := range plan.WorkItemIDs {
			workItems = append(workItems, WorkItem{ID: id})
		}
	}
	workItems = distinctWorkItems(workItems)
	if len(workItems) == 0 {
		rollback()
		return StartExecutionReport{}, ErrInvalidManifest
	}
	first := workItems[0]
	now := e.Clock
	if now == nil {
		now = realClock{}
	}
	manifest := Manifest{Schema: 1, WorkItemID: plan.PrimaryWorkItemID, TaskID: plan.TaskID, Project: plan.Project, Type: plan.Type, Slug: plan.Slug, BranchName: plan.BranchName, CreatedAt: now.Now().UTC().Format("2006-01-02T15:04:05Z07:00"), Repositories: append([]string(nil), plan.Repositories...), Status: "created", WorkItemType: cloneString(first.Type), WorkItemTitle: cloneString(first.Title), WorkItemState: cloneString(first.State), WorkItems: workItems}
	if len(childTasks) > 0 {
		manifest.ChildTasks = distinctChildTasks(childTasks)
	}
	if err := writeWorkspaceFiles(plan.Workspace, manifest, true); err != nil {
		rollback()
		return StartExecutionReport{}, localizedOperation("write workspace files", err)
	}
	pushEvent(ActionEvent{Type: "workspaceCreated"})
	return StartExecutionReport{Plan: plan, Manifest: manifest, WorkItems: workItems, ChildTasks: manifest.NormalizedChildTasks(), Events: events}, nil
}

func (e *Engine) Start(ctx context.Context, request StartRequest, preview bool) (StartPlan, *StartExecutionReport, error) {
	var items []WorkItem
	var children []ChildTask
	var stateUpdates []StartStateUpdate
	var err error
	project := strings.TrimSpace(request.Project)
	if project == "" {
		project = "default"
	}
	if e.Work != nil && (!preview || strings.TrimSpace(request.Slug) == "") {
		items, err = e.Work.GetWorkItems(ctx, project, distinctCSV(request.WorkItemIDs))
		if err != nil {
			return StartPlan{}, nil, err
		}
	}
	var plan StartPlan
	if len(items) > 0 {
		plan, err = e.PlanStartWithItems(ctx, request, items)
	} else {
		plan, err = e.PlanStart(ctx, request)
	}
	if err != nil {
		return StartPlan{}, nil, err
	}
	if preview {
		return plan, nil, nil
	}
	workflow := defaultWorkflow()
	if e.Config != nil {
		workflow, err = e.Config.Workflow(ctx, request.Root)
		if err != nil {
			return plan, nil, err
		}
	}
	if e.Work != nil {
		if workflow.TaskStart.CreateChildTasks && len(items) > 0 {
			parent := items[0]
			for _, repository := range plan.Repositories {
				title := parent.ID
				if parent.Title != nil && strings.TrimSpace(*parent.Title) != "" {
					title = *parent.Title
				}
				child, createErr := e.Work.CreateChildTask(ctx, plan.Project, parent, repository, ChildTaskTitle(repository, title))
				if createErr != nil {
					return plan, nil, createErr
				}
				child.Repository = repository
				children = append(children, child)
			}
			plan = StartPlanWithChildTasks(plan, children)
		}
		if workflow.TaskStart.UpdateWorkItemState {
			for _, item := range items {
				state := startState(item.Type, workflow.TaskStart.States)
				if state == nil {
					continue
				}
				changed := item.State == nil || !equalFold(*item.State, *state)
				if changed {
					if err = e.Work.UpdateWorkItemState(ctx, plan.Project, item.ID, *state); err != nil {
						return plan, nil, err
					}
				}
				stateUpdates = append(stateUpdates, StartStateUpdate{ID: item.ID, Label: workItemLabel(item), TargetState: *state, Changed: changed})
			}
		}
	}
	report, err := e.ExecuteStart(ctx, plan, items, children, nil)
	report.StateUpdates = stateUpdates
	return plan, &report, err
}

func StartPlanWithChildTasks(plan StartPlan, tasks []ChildTask) StartPlan {
	if (plan.TaskID == nil || strings.TrimSpace(*plan.TaskID) == "") && len(tasks) == 1 && strings.TrimSpace(tasks[0].ID) != "" {
		id := tasks[0].ID
		plan.TaskID = &id
	}
	ids := append([]string(nil), plan.WorkItemIDs...)
	if plan.TaskID != nil {
		ids = appendDistinct(ids, *plan.TaskID)
	}
	for _, task := range tasks {
		ids = appendDistinct(ids, task.ID)
	}
	plan.BranchName = BuildBranchName(plan.Type, ids, plan.Slug)
	for index := range plan.RepositoryWorktrees {
		plan.RepositoryWorktrees[index].BranchName = plan.BranchName
	}
	return plan
}

func (e *Engine) PlanRename(_ context.Context, root, workspace, slug string) (Manifest, RenamePlan, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, RenamePlan{}, err
	}
	newSlug := SlugOrFallback(slug, manifest.Slug)
	newBranch := BuildBranchName(manifest.Type, manifest.AllKnownWorkItemIDs(), newSlug)
	parentIDs := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		parentIDs = append(parentIDs, item.ID)
	}
	newWorkspace := filepath.Join(filepath.Dir(workspace), BuildSubjectName(manifest.Type, parentIDs, newSlug))
	return manifest, RenamePlan{Workspace: workspace, NewWorkspace: newWorkspace, OldSlug: manifest.Slug, NewSlug: newSlug, OldBranch: manifest.BranchName, NewBranch: newBranch}, nil
}
func (e *Engine) ExecuteRename(_ context.Context, manifest Manifest, plan RenamePlan) (RenameExecutionReport, error) {
	updated := manifest
	updated.Slug = plan.NewSlug
	updated.BranchName = plan.NewBranch
	if plan.Workspace != plan.NewWorkspace {
		if _, err := os.Stat(plan.NewWorkspace); err == nil {
			return RenameExecutionReport{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", plan.NewWorkspace))
		}
	}
	if err := WriteManifest(filepath.Join(plan.Workspace, ManifestFile), updated); err != nil {
		return RenameExecutionReport{}, localizedOperation("write renamed manifest", err)
	}
	if plan.Workspace != plan.NewWorkspace {
		if err := os.Rename(plan.Workspace, plan.NewWorkspace); err != nil {
			return RenameExecutionReport{}, localizedOperation("rename workspace", err)
		}
	}
	return RenameExecutionReport{Plan: plan, Manifest: updated}, nil
}

func PlanSync(workspace string) (SyncPlanReport, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return SyncPlanReport{}, err
	}
	ids := make([]string, 0)
	for _, item := range manifest.ParentWorkItems() {
		ids = append(ids, item.ID)
	}
	return SyncPlanReport{Workspace: workspace, RequestedIDs: ids}, nil
}

func ExecuteSync(plan SyncPlanReport, snapshots []WorkItem) (SyncReport, error) {
	manifest, err := ApplySnapshots(plan.Workspace, snapshots)
	if err != nil {
		return SyncReport{}, err
	}
	return SyncReport{Workspace: plan.Workspace, RequestedIDs: append([]string(nil), plan.RequestedIDs...), Snapshots: append([]WorkItem(nil), snapshots...), Manifest: manifest}, nil
}

func (e *Engine) Sync(ctx context.Context, workspace string) (SyncReport, error) {
	if e.Work == nil {
		return SyncReport{}, ErrWorkCapabilityRequired
	}
	plan, err := PlanSync(workspace)
	if err != nil {
		return SyncReport{}, err
	}
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return SyncReport{}, err
	}
	snapshots, err := e.Work.GetWorkItems(ctx, manifest.Project, plan.RequestedIDs)
	if err != nil {
		return SyncReport{}, err
	}
	return ExecuteSync(plan, snapshots)
}

func PlanWorkItemUpdate(root, workspace string, items []WorkItem) (Manifest, WorkItemUpdatePlan, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, WorkItemUpdatePlan{}, err
	}
	items = distinctWorkItems(items)
	if len(items) == 0 {
		return Manifest{}, WorkItemUpdatePlan{}, ErrEmptyWorkItemSet
	}
	ids := make([]string, 0)
	for _, item := range items {
		ids = append(ids, item.ID)
	}
	branchIDs := append([]string(nil), ids...)
	if manifest.TaskID != nil {
		branchIDs = appendDistinct(branchIDs, *manifest.TaskID)
	}
	for _, task := range manifest.NormalizedChildTasks() {
		branchIDs = appendDistinct(branchIDs, task.ID)
	}
	newBranch := BuildBranchName(manifest.Type, branchIDs, manifest.Slug)
	subject := BuildSubjectName(manifest.Type, ids, manifest.Slug)
	return manifest, WorkItemUpdatePlan{Workspace: workspace, NewWorkspace: filepath.Join(filepath.Dir(workspace), subject), OldBranch: manifest.BranchName, NewBranch: newBranch, WorkItems: items}, nil
}
func PlanAddWorkItems(root, workspace string, items []WorkItem) (Manifest, WorkItemUpdatePlan, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, WorkItemUpdatePlan{}, err
	}
	all := manifest.ParentWorkItems()
	added := make([]string, 0)
	for _, item := range items {
		if manifest.MatchesWorkItem(item.ID) {
			continue
		}
		found := false
		for _, existing := range all {
			if equalFold(existing.ID, item.ID) {
				found = true
				break
			}
		}
		if !found {
			all = append(all, item)
			added = append(added, item.ID)
		}
	}
	for _, candidate := range Filter(Discover(root), manifest.Project, added) {
		if !equalFold(candidate.Path, workspace) {
			return Manifest{}, WorkItemUpdatePlan{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", candidate.Path))
		}
	}
	return PlanWorkItemUpdate(root, workspace, all)
}
func PlanRemoveWorkItems(root, workspace string, ids []string) (Manifest, WorkItemUpdatePlan, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, WorkItemUpdatePlan{}, err
	}
	items := make([]WorkItem, 0)
	for _, item := range manifest.ParentWorkItems() {
		remove := false
		for _, id := range ids {
			if equalFold(id, item.ID) {
				remove = true
				break
			}
		}
		if !remove {
			items = append(items, item)
		}
	}
	return PlanWorkItemUpdate(root, workspace, items)
}
func ExecuteWorkItemUpdate(manifest Manifest, plan WorkItemUpdatePlan, action string) (WorkItemUpdateReport, error) {
	if len(plan.WorkItems) == 0 {
		return WorkItemUpdateReport{}, ErrEmptyWorkItemSet
	}
	first := plan.WorkItems[0]
	updated := manifest
	updated.WorkItemID = first.ID
	updated.WorkItemType = cloneString(first.Type)
	updated.WorkItemTitle = cloneString(first.Title)
	updated.WorkItemState = cloneString(first.State)
	updated.WorkItems = append([]WorkItem(nil), plan.WorkItems...)
	updated.BranchName = plan.NewBranch
	if plan.Workspace != plan.NewWorkspace {
		if _, err := os.Stat(plan.NewWorkspace); err == nil {
			return WorkItemUpdateReport{}, localizedCause("workspace.error.workspace-conflict", ErrWorkspaceConflict, l10n.A("detail", plan.NewWorkspace))
		}
	}
	if err := writeWorkspaceFiles(plan.Workspace, updated, false); err != nil {
		return WorkItemUpdateReport{}, localizedOperation("write workspace files", err)
	}
	if plan.Workspace != plan.NewWorkspace {
		if err := os.Rename(plan.Workspace, plan.NewWorkspace); err != nil {
			return WorkItemUpdateReport{}, localizedOperation("rename workspace", err)
		}
	}
	if err := WriteGeneratedFiles(plan.NewWorkspace, updated); err != nil {
		return WorkItemUpdateReport{}, localizedOperation("write generated agent files", err)
	}
	return WorkItemUpdateReport{Action: action, Plan: plan, Manifest: updated, Workspace: plan.NewWorkspace}, nil
}

func (e *Engine) CreateChildTask(ctx context.Context, workspace, repository, title string) (ChildTask, Manifest, error) {
	if e.Work == nil {
		return ChildTask{}, Manifest{}, ErrWorkCapabilityRequired
	}
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return ChildTask{}, Manifest{}, err
	}
	parent := manifest.ParentWorkItems()[0]
	if !RequiresChildTasks(parent.Type) {
		return ChildTask{}, Manifest{}, localized("workspace.error.child-task-type")
	}
	task, err := e.Work.CreateChildTask(ctx, manifest.Project, parent, repository, ChildTaskTitle(repository, title))
	if err != nil {
		return ChildTask{}, Manifest{}, err
	}
	task.Repository = repository
	manifest.ChildTasks = append(manifest.NormalizedChildTasks(), task)
	if err = WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		return ChildTask{}, Manifest{}, localizedOperation("write child task manifest", err)
	}
	if err = updateHandoffChildTasks(workspace, repository, manifest.NormalizedChildTasks()); err != nil {
		return ChildTask{}, Manifest{}, localizedOperation("update child task handoff", err)
	}
	return task, manifest, nil
}
func RequiresChildTasks(kind *string) bool {
	normalized := strings.ToLower(strings.TrimSpace(valueOrEmpty(kind)))
	return normalized == "user story" || normalized == "anomalie"
}
func ChildTaskTitle(repository, title string) string {
	prefix := strings.ToUpper(repository)
	switch strings.ToLower(repository) {
	case "front":
		prefix = "FRONT"
	case "back":
		prefix = "BACK"
	case "sql", "db", "database":
		prefix = "SQL"
	}
	return "[" + prefix + "] " + title
}

func writeWorkspaceFiles(workspace string, manifest Manifest, agents bool) error {
	if err := os.MkdirAll(workspace, 0o755); err != nil {
		return err
	}
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		return err
	}
	if err := writeFileAtomic(filepath.Join(workspace, PlanFile), []byte(PlanMarkdown(manifest)), 0o644); err != nil {
		return err
	}
	for _, repository := range manifest.Repositories {
		if err := writeFileAtomic(filepath.Join(workspace, HandoffPrefix+repository+".md"), []byte(HandoffMarkdown(manifest, repository)), 0o644); err != nil {
			return err
		}
	}
	if agents {
		return WriteGeneratedFiles(workspace, manifest)
	}
	return nil
}
func updateHandoffChildTasks(workspace, repository string, tasks []ChildTask) error {
	path := filepath.Join(workspace, HandoffPrefix+repository+".md")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		return err
	}
	known := make([]string, 0)
	for _, task := range tasks {
		if equalFold(task.Repository, repository) {
			value := "#" + task.ID
			if task.Title != nil {
				value += " (" + *task.Title + ")"
			}
			known = append(known, value)
		}
	}
	if len(known) == 0 {
		known = []string{"(aucune)"}
	}
	lines := strings.Split(strings.ReplaceAll(string(data), "\r\n", "\n"), "\n")
	for index, line := range lines {
		if strings.HasPrefix(line, "- Child tasks connus:") {
			lines[index] = "- Child tasks connus: " + strings.Join(known, ", ")
		}
	}
	return writeFileAtomic(path, []byte(strings.Join(lines, "\n")), 0o644)
}
func (e *Engine) project(ctx context.Context, root, key string) (ProjectConfig, bool, error) {
	if e.Config == nil {
		return ProjectConfig{Key: key}, false, nil
	}
	return e.Config.Project(ctx, root, key)
}
func normalizeRepositoryConfig(repository *RepositoryConfig, name string) {
	repository.Name = name
	if strings.TrimSpace(repository.DefaultBranch) == "" {
		repository.DefaultBranch = "main"
	}
	if strings.TrimSpace(repository.Folder) == "" {
		repository.Folder = name
	}
	if strings.TrimSpace(repository.AnchorName) == "" {
		repository.AnchorName = name + ".git"
	}
}
func (e *Engine) gitCredential(ctx context.Context, key string) (*gitrepo.Credential, error) {
	if strings.TrimSpace(key) == "" {
		return nil, nil
	}
	if e.Secrets == nil {
		return nil, ErrSecretCapabilityRequired
	}
	value, ok, err := e.Secrets.Get(ctx, contract.SecretKey(key))
	if err != nil {
		return nil, localizedDetail("workspace.error.secret-read", err, l10n.A("key", key))
	}
	if !ok {
		return nil, localized("workspace.error.secret-not-found", l10n.A("key", key))
	}
	credential := gitrepo.NewPersonalAccessToken(value)
	return &credential, nil
}
func pushEvent(events *[]ActionEvent, emit func(ActionEvent), event ActionEvent) {
	*events = append(*events, event)
	if emit != nil {
		emit(event)
	}
}
func sortStrings(values []string) { sort.Strings(values) }

// ApplySnapshots updates only task.json from provider-neutral work item data.
// Provider fetching and authentication remain in the orchestration layer.
func ApplySnapshots(workspace string, snapshots []WorkItem) (Manifest, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, err
	}
	if len(snapshots) == 0 {
		return manifest, nil
	}
	snapshots = distinctWorkItems(snapshots)
	if len(snapshots) == 0 {
		return manifest, nil
	}
	first := snapshots[0]
	manifest.WorkItemID = first.ID
	manifest.WorkItemType = cloneString(first.Type)
	manifest.WorkItemTitle = cloneString(first.Title)
	manifest.WorkItemState = cloneString(first.State)
	manifest.WorkItems = snapshots
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		return Manifest{}, localizedOperation("write synchronized manifest", err)
	}
	return manifest, nil
}

// AddChild records a created provider child task and refreshes its handoff.
func AddChild(workspace string, task ChildTask) (Manifest, error) {
	manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
	if err != nil {
		return Manifest{}, err
	}
	manifest.ChildTasks = append(manifest.NormalizedChildTasks(), task)
	manifest.ChildTasks = distinctChildTasks(manifest.ChildTasks)
	if err := WriteManifest(filepath.Join(workspace, ManifestFile), manifest); err != nil {
		return Manifest{}, localizedOperation("write child task manifest", err)
	}
	if err := updateHandoffChildTasks(workspace, task.Repository, manifest.ChildTasks); err != nil {
		return Manifest{}, localizedOperation("update child task handoff", err)
	}
	return manifest, nil
}

func startState(kind *string, states []WorkItemTypeState) *string {
	normalized := normalizeState(valueOrEmpty(kind))
	switch normalized {
	case "user story", "anomalie", "bug", "activite", "task", "tache":
	default:
		return nil
	}
	for _, item := range states {
		if normalizeState(item.Type) == normalized && strings.TrimSpace(item.State) != "" {
			value := item.State
			return &value
		}
	}
	value := "En développement"
	if normalized == "user story" || normalized == "anomalie" {
		value = "En réalisation"
	}
	return &value
}
