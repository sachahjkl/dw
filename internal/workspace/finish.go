package workspace

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

func (e *Engine) PlanFinish(ctx context.Context, root, workspace, message string, createPR, ready bool) (FinishPlanReport, error) {
	manifest, targets, err := e.commitTargets(ctx, root, workspace)
	if err != nil {
		return FinishPlanReport{}, err
	}
	handoff, err := ValidateHandoffs(workspace)
	if err != nil {
		return FinishPlanReport{}, err
	}
	project, _, err := e.project(ctx, root, manifest.Project)
	if err != nil {
		return FinishPlanReport{}, err
	}
	statuses := make([]TargetStatus, 0, len(targets))
	changed := make([]string, 0)
	unpushed := make([]string, 0)
	for _, target := range targets {
		status := RepositoryStatus{}
		if e.Git != nil {
			status, err = e.Git.Status(ctx, target.Path)
			if err != nil {
				return FinishPlanReport{}, err
			}
		}
		statuses = append(statuses, TargetStatus{Target: target, Status: status})
		if status.IsGitRepository && status.HasChanges {
			changed = append(changed, target.Repository)
		}
		if status.IsGitRepository && status.HasUnpushed {
			unpushed = append(unpushed, target.Repository)
		}
	}
	actionable := unpushed
	if len(changed) > 0 {
		actionable = changed
	}
	summaries := make([]HandoffSummary, 0)
	for _, target := range targets {
		data, readErr := os.ReadFile(filepath.Join(workspace, HandoffPrefix+target.Repository+".md"))
		if readErr == nil {
			if summary, parseErr := ParseHandoff(string(data), target.Repository); parseErr == nil {
				summaries = append(summaries, summary)
			}
		}
	}
	candidates := make([]PullRequestCandidate, 0)
	if createPR {
		for _, name := range actionable {
			if candidate, ok := candidateFor(name, targets, project); ok {
				candidates = append(candidates, candidate)
			}
		}
		if len(candidates) == 0 && e.Git != nil {
			for index, target := range targets {
				if !statuses[index].Status.IsGitRepository {
					continue
				}
				candidate, _ := candidateFor(target.Repository, targets, project)
				ahead, aheadErr := e.Git.HasCommitsAhead(ctx, target.Path, "origin/"+candidate.TargetBranch)
				if aheadErr == nil && ahead {
					candidates = append(candidates, candidate)
				}
			}
		}
	}
	return FinishPlanReport{Root: root, Workspace: workspace, Manifest: manifest, Targets: statuses, Handoff: handoff, HandoffSummaries: summaries, CommitMessage: BuildCommitMessage(manifest, message), CreatePR: createPR, Ready: ready, ChangedRepositories: changed, UnpushedRepositories: unpushed, ActionableRepositories: append([]string(nil), actionable...), PullRequestCandidates: candidates}, nil
}
func candidateFor(name string, targets []RepositoryTarget, project ProjectConfig) (PullRequestCandidate, bool) {
	var target RepositoryTarget
	found := false
	for _, item := range targets {
		if equalFold(item.Repository, name) {
			target = item
			found = true
			break
		}
	}
	if !found {
		return PullRequestCandidate{}, false
	}
	repository, ok := project.Repository(name)
	targetBranch := "main"
	providerRepository := ""
	if ok {
		if strings.TrimSpace(repository.PullRequestTargetBranch) != "" {
			targetBranch = repository.PullRequestTargetBranch
		} else if strings.TrimSpace(repository.DefaultBranch) != "" {
			targetBranch = repository.DefaultBranch
		}
		providerRepository = repository.ProviderRepository
	}
	return PullRequestCandidate{Repository: name, Path: target.Path, ProviderRepository: providerRepository, TargetBranch: targetBranch}, true
}
func (e *Engine) ExecuteFinish(ctx context.Context, plan FinishPlanReport, options FinishExecuteOptions, emit func(ActionEvent)) (FinishExecutionReport, error) {
	if !plan.Handoff.IsValid {
		return FinishExecutionReport{}, ErrInvalidHandoff
	}
	if e.Git == nil {
		return FinishExecutionReport{}, ErrGitCapabilityRequired
	}
	events := make([]ActionEvent, 0)
	verification := make([]VerificationResult, 0)
	gitActions := make([]GitAction, 0)
	pullRequests := make([]PullRequestResult, 0)
	stateUpdates := make([]WorkItemStateUpdate, 0)
	workflow := defaultWorkflow()
	if e.Config != nil {
		configured, err := e.Config.Workflow(ctx, plan.Root)
		if err != nil {
			return FinishExecutionReport{}, err
		}
		workflow = configured
	}
	if !options.SkipVerification && workflow.TaskFinish.RunVerification {
		pushEvent(&events, emit, ActionEvent{Type: "verifyingFinish", RepositoryCount: len(plan.PullRequestCandidates)})
		verification = RunVerification(ctx, e.verifier(), workflow.TaskFinish.VerificationCommands, plan.PullRequestCandidates)
		for _, result := range verification {
			if result.ExitCode != 0 {
				return FinishExecutionReport{}, ErrVerificationFailed
			}
		}
		pushEvent(&events, emit, ActionEvent{Type: "finishVerificationCompleted"})
	}
	changed := changedTargets(plan.Targets)
	unpushed := unpushedTargets(plan.Targets)
	if len(changed) > 0 {
		pushEvent(&events, emit, ActionEvent{Type: "runningGitOperation", Operation: "commitAndPush", RepositoryCount: len(changed)})
		for _, target := range changed {
			pushEvent(&events, emit, ActionEvent{Type: "runningRepositoryGitOperation", Repository: target.Target.Repository, Operation: "commitAndPush"})
			if err := e.Git.Commit(ctx, target.Target.Path, plan.CommitMessage); err != nil {
				return FinishExecutionReport{}, err
			}
			if err := e.Git.Push(ctx, target.Target.Path, plan.Manifest.BranchName, options.ForceWithLease); err != nil {
				return FinishExecutionReport{}, pushError(err, options.ForceWithLease)
			}
			gitActions = append(gitActions, GitAction{Repository: target.Target.Repository, Operation: "commitAndPush", Path: target.Target.Path})
		}
		pushEvent(&events, emit, ActionEvent{Type: "gitOperationCompleted", Operation: "commitAndPush"})
	} else if len(unpushed) > 0 {
		pushEvent(&events, emit, ActionEvent{Type: "runningGitOperation", Operation: "push", RepositoryCount: len(unpushed)})
		for _, target := range unpushed {
			pushEvent(&events, emit, ActionEvent{Type: "runningRepositoryGitOperation", Repository: target.Target.Repository, Operation: "push"})
			if err := e.Git.Push(ctx, target.Target.Path, plan.Manifest.BranchName, options.ForceWithLease); err != nil {
				return FinishExecutionReport{}, pushError(err, options.ForceWithLease)
			}
			gitActions = append(gitActions, GitAction{Repository: target.Target.Repository, Operation: "push", Path: target.Target.Path})
		}
		pushEvent(&events, emit, ActionEvent{Type: "gitOperationCompleted", Operation: "push"})
	}
	if !plan.CreatePR {
		pushEvent(&events, emit, ActionEvent{Type: "skippingPullRequestCreation"})
		return FinishExecutionReport{Plan: plan, Events: events, VerificationResults: verification, GitActions: gitActions, PullRequests: pullRequests, WorkItemUpdates: stateUpdates}, nil
	}
	if e.Work == nil {
		return FinishExecutionReport{}, ErrWorkCapabilityRequired
	}
	sourceRef := "refs/heads/" + plan.Manifest.BranchName
	taskPlan, _ := os.ReadFile(filepath.Join(plan.Workspace, PlanFile))
	for _, candidate := range plan.PullRequestCandidates {
		if strings.TrimSpace(candidate.ProviderRepository) == "" {
			pullRequests = append(pullRequests, PullRequestResult{Repository: candidate.Repository, Action: "skipped", SkipReason: "missingProviderRepository"})
			continue
		}
		pushEvent(&events, emit, ActionEvent{Type: "checkingActivePullRequest", Repository: candidate.Repository})
		existing, err := e.Work.FindActivePullRequest(ctx, plan.Manifest.Project, candidate.ProviderRepository, sourceRef)
		if err != nil {
			return FinishExecutionReport{}, err
		}
		if existing != nil {
			pullRequests = append(pullRequests, PullRequestResult{Repository: candidate.Repository, Action: "existing", URL: existing.URL, PullRequestID: &existing.ID})
			continue
		}
		summary, err := readHandoffSummary(plan.Workspace, candidate.Repository)
		if err != nil {
			return FinishExecutionReport{}, err
		}
		input := PullRequestInput{ProviderRepository: candidate.ProviderRepository, SourceRefName: sourceRef, TargetRefName: "refs/heads/" + candidate.TargetBranch, Title: BuildCommitMessage(plan.Manifest, ""), Description: PullRequestDescription(plan.Manifest, candidate, string(taskPlan), verification, summary), IsDraft: !plan.Ready, WorkItemIDs: plan.Manifest.AllKnownWorkItemIDs()}
		pushEvent(&events, emit, ActionEvent{Type: "creatingPullRequest", Repository: candidate.Repository})
		created, err := e.Work.CreatePullRequest(ctx, plan.Manifest.Project, input)
		if err != nil {
			return FinishExecutionReport{}, err
		}
		for _, id := range plan.Manifest.AllKnownWorkItemIDs() {
			if linkErr := e.Work.LinkWorkItemToPullRequest(ctx, plan.Manifest.Project, candidate.ProviderRepository, created.ID, id); linkErr != nil {
				pushEvent(&events, emit, ActionEvent{Type: "pullRequestWorkItemLinkSkipped", WorkItemID: id, Error: linkErr.Error()})
			}
		}
		pullRequests = append(pullRequests, PullRequestResult{Repository: candidate.Repository, Action: "created", URL: created.URL, PullRequestID: &created.ID})
	}
	if workflow.TaskFinish.UpdateWorkItemState {
		items, err := e.Work.GetWorkItems(ctx, plan.Manifest.Project, plan.Manifest.AllKnownWorkItemIDs())
		if err != nil {
			return FinishExecutionReport{}, err
		}
		for _, item := range items {
			target := finishState(item.Type, workflow.TaskFinish.States)
			label := workItemLabel(item)
			if target == nil {
				stateUpdates = append(stateUpdates, WorkItemStateUpdate{ID: item.ID, Label: label, Type: item.Type, CurrentState: item.State, Changed: false, Outcome: "unsupportedWorkItemType"})
				continue
			}
			if item.State != nil && equalFold(*item.State, *target) {
				stateUpdates = append(stateUpdates, WorkItemStateUpdate{ID: item.ID, Label: label, Type: item.Type, CurrentState: item.State, TargetState: target, Changed: false, Outcome: "alreadyInTargetState"})
				continue
			}
			if err = e.Work.UpdateWorkItemState(ctx, plan.Manifest.Project, item.ID, *target); err != nil {
				return FinishExecutionReport{}, err
			}
			stateUpdates = append(stateUpdates, WorkItemStateUpdate{ID: item.ID, Label: label, Type: item.Type, CurrentState: item.State, TargetState: target, Changed: true, Outcome: "updated"})
		}
	}
	return FinishExecutionReport{Plan: plan, Events: events, VerificationResults: verification, GitActions: gitActions, PullRequests: pullRequests, WorkItemUpdates: stateUpdates}, nil
}

func RunVerification(ctx context.Context, runner VerificationPort, configured []RepositoryCommands, candidates []PullRequestCandidate) []VerificationResult {
	if runner == nil {
		return []VerificationResult{}
	}
	result := make([]VerificationResult, 0)
	for _, candidate := range candidates {
		for _, configuration := range configured {
			if !equalFold(configuration.Repository, candidate.Repository) {
				continue
			}
			for _, command := range configuration.Commands {
				command = strings.TrimSpace(command)
				if command == "" {
					continue
				}
				exit, stdout, stderr := runner.Run(ctx, candidate.Path, command)
				result = append(result, VerificationResult{Repository: candidate.Repository, Command: command, ExitCode: exit, StandardOutput: stdout, StandardError: stderr})
			}
		}
	}
	return result
}

type ShellVerification struct{}

func (ShellVerification) Run(ctx context.Context, directory, command string) (int, string, string) {
	var process *exec.Cmd
	if runtime.GOOS == "windows" {
		process = exec.CommandContext(ctx, "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command)
	} else {
		process = exec.CommandContext(ctx, "sh", "-lc", command)
	}
	process.Dir = directory
	var stdout, stderr strings.Builder
	process.Stdout = &stdout
	process.Stderr = &stderr
	err := process.Run()
	exit := 0
	if err != nil {
		exit = 1
		if value, ok := err.(*exec.ExitError); ok {
			exit = value.ExitCode()
		} else if stderr.Len() == 0 {
			stderr.WriteString(err.Error())
		}
	}
	return exit, stdout.String(), stderr.String()
}
func (e *Engine) verifier() VerificationPort {
	if e.Verify != nil {
		return e.Verify
	}
	return ShellVerification{}
}
func PullRequestDescription(manifest Manifest, candidate PullRequestCandidate, plan string, verification []VerificationResult, handoff HandoffSummary) string {
	if strings.TrimSpace(plan) == "" {
		plan = "_Plan not found._"
	} else {
		plan = strings.TrimSpace(plan)
	}
	ids := make([]string, 0)
	for _, id := range manifest.AllKnownWorkItemIDs() {
		ids = append(ids, "#"+id)
	}
	return fmt.Sprintf("## Summary\n- Work completed for `%s`\n- Repository: `%s`\n- Work items: `%s`\n\n## Plan\n%s\n\n## Handoff\n%s\n\n## Verification\n%s\n", manifest.Slug, candidate.Repository, strings.Join(ids, ", "), plan, StructuredHandoff(handoff), renderVerification(candidate.Repository, verification))
}
func StructuredHandoff(summary HandoffSummary) string {
	return fmt.Sprintf("### Status\n- `%s`\n\n### Work Completed\n%s\n\n### Decisions\n%s\n\n### Risks\n%s\n\n### Blockers\n%s\n\n### Follow-up\n%s\n", summary.Status, renderList(summary.Done), renderList(summary.Decisions), renderList(summary.Risks), renderList(summary.Blockers), renderList(summary.FollowUp))
}
func renderList(items []string) string {
	if len(items) == 0 {
		return "- (none)"
	}
	lines := make([]string, len(items))
	for index, item := range items {
		lines[index] = "- " + item
	}
	return strings.Join(lines, "\n")
}
func renderVerification(repository string, results []VerificationResult) string {
	lines := make([]string, 0)
	for _, result := range results {
		if equalFold(result.Repository, repository) {
			status := "passed"
			if result.ExitCode != 0 {
				status = "failed"
			}
			lines = append(lines, fmt.Sprintf("- `%s`: %s", result.Command, status))
		}
	}
	if len(lines) == 0 {
		return "- No command configured in `taskFinish.verificationCommands`."
	}
	return strings.Join(lines, "\n")
}
func readHandoffSummary(workspace, repository string) (HandoffSummary, error) {
	data, err := os.ReadFile(filepath.Join(workspace, HandoffPrefix+repository+".md"))
	if err != nil {
		return HandoffSummary{}, err
	}
	return ParseHandoff(string(data), repository)
}
func changedTargets(targets []TargetStatus) []TargetStatus {
	result := make([]TargetStatus, 0)
	for _, target := range targets {
		if target.Status.IsGitRepository && target.Status.HasChanges {
			result = append(result, target)
		}
	}
	return result
}
func unpushedTargets(targets []TargetStatus) []TargetStatus {
	result := make([]TargetStatus, 0)
	for _, target := range targets {
		if target.Status.IsGitRepository && target.Status.HasUnpushed {
			result = append(result, target)
		}
	}
	return result
}
func pushError(err error, force bool) error {
	detail := strings.ToLower(err.Error())
	if !force && (strings.Contains(detail, "non-fast-forward") || strings.Contains(detail, "fetch first") || strings.Contains(detail, "stale info") || strings.Contains(detail, "remote branch changed")) {
		return localizedCause("workspace.error.remote-diverged", err)
	}
	return err
}
func defaultWorkflow() WorkflowConfig {
	return WorkflowConfig{
		TaskStart: StartOptions{UpdateWorkItemState: true, CreateChildTasks: false, States: []WorkItemTypeState{
			{Type: "user story", State: "En réalisation"}, {Type: "anomalie", State: "En réalisation"},
			{Type: "bug", State: "En développement"}, {Type: "activite", State: "En développement"},
			{Type: "task", State: "En développement"}, {Type: "tache", State: "En développement"},
		}},
		TaskFinish: FinishOptions{RunVerification: true, UpdateWorkItemState: true, States: []WorkItemTypeState{
			{Type: "bug", State: "PR en attente"}, {Type: "activite", State: "PR en attente"},
			{Type: "task", State: "PR en attente"}, {Type: "tache", State: "PR en attente"},
		}},
	}
}
func finishState(kind *string, states []WorkItemTypeState) *string {
	normalized := normalizeState(valueOrEmpty(kind))
	if normalized != "bug" && normalized != "activite" && normalized != "task" && normalized != "tache" {
		return nil
	}
	for _, state := range states {
		if normalizeState(state.Type) == normalized && strings.TrimSpace(state.State) != "" {
			value := state.State
			return &value
		}
	}
	value := "PR en attente"
	return &value
}
func workItemLabel(item WorkItem) string {
	label := "#" + item.ID
	if item.Type != nil {
		label += " [" + *item.Type + "]"
	}
	if item.Title != nil {
		label += " " + *item.Title
	}
	return label
}

// ExecuteLocalFinish performs the complete local finish stage (handoff gate,
// verification, commit and push) without invoking a work provider. It lets
// orchestration layers own PR and work-item state sequencing explicitly.
func (e *Engine) ExecuteLocalFinish(ctx context.Context, plan FinishPlanReport, options FinishExecuteOptions, emit func(ActionEvent)) (FinishExecutionReport, error) {
	local := plan
	local.CreatePR = false
	report, err := e.ExecuteFinish(ctx, local, options, emit)
	if err != nil {
		return FinishExecutionReport{}, err
	}
	report.Plan = plan
	if count := len(report.Events); count > 0 && report.Events[count-1].Type == "skippingPullRequestCreation" {
		report.Events = report.Events[:count-1]
	}
	return report, nil
}
