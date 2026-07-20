package workspace

import (
	"context"
	"encoding/json"
	"time"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/gitrepo"
)

const (
	ManifestFile             = "task.json"
	PlanFile                 = "plan.md"
	HandoffPrefix            = "handoff-"
	HandoffValidationVersion = "dw.task.handoff-validation.v1"
	PreflightVersion         = "dw.task.preflight.v1"
)

type Manifest struct {
	Schema        int64                      `json:"schema"`
	WorkItemID    string                     `json:"workItemId"`
	TaskID        *string                    `json:"taskId"`
	Project       string                     `json:"project"`
	Type          string                     `json:"type"`
	Slug          string                     `json:"slug"`
	BranchName    string                     `json:"branchName"`
	CreatedAt     string                     `json:"createdAt"`
	Repositories  []string                   `json:"repositories"`
	Status        string                     `json:"status"`
	WorkItemType  *string                    `json:"workItemType,omitempty"`
	WorkItemTitle *string                    `json:"workItemTitle,omitempty"`
	WorkItemState *string                    `json:"workItemState,omitempty"`
	ChildTaskIDs  map[string]string          `json:"childTaskIds,omitempty"`
	ChildTasks    []ChildTask                `json:"childTasks,omitempty"`
	WorkItems     []WorkItem                 `json:"workItems,omitempty"`
	Unknown       map[string]json.RawMessage `json:"-"`
}

type WorkItem struct {
	ID    string  `json:"id"`
	Type  *string `json:"type"`
	Title *string `json:"title"`
	State *string `json:"state"`
	URL   *string `json:"url,omitempty"`
}

type ChildTask struct {
	Repository string  `json:"repository"`
	ID         string  `json:"id"`
	Title      *string `json:"title"`
}

type Summary struct {
	Path     string   `json:"path"`
	Manifest Manifest `json:"manifest"`
}

type ListItem struct {
	Path                string     `json:"path"`
	Project             string     `json:"project"`
	WorkItemID          string     `json:"workItemId"`
	WorkItems           []WorkItem `json:"workItems"`
	TaskID              *string    `json:"taskId"`
	AllKnownWorkItemIDs []string   `json:"allKnownWorkItemIds"`
	Type                string     `json:"type"`
	Slug                string     `json:"slug"`
	BranchName          string     `json:"branchName"`
	CreatedAt           string     `json:"createdAt"`
	WorkItemType        *string    `json:"workItemType"`
	WorkItemTitle       *string    `json:"workItemTitle"`
	WorkItemState       *string    `json:"workItemState"`
	Repositories        []string   `json:"repositories"`
}

type CurrentItem struct {
	Workspace         string            `json:"workspace"`
	Project           string            `json:"project"`
	PrimaryWorkItemID string            `json:"primaryWorkItemId"`
	WorkItems         []WorkItem        `json:"workItems"`
	TaskID            *string           `json:"taskId"`
	ChildTaskIDs      map[string]string `json:"childTaskIds"`
	ChildTasks        []ChildTask       `json:"childTasks"`
	Branch            string            `json:"branch"`
	Repositories      []string          `json:"repositories"`
}

type RepositoryConfig struct {
	Name                    string  `json:"name"`
	HTTPURL                 string  `json:"httpUrl"`
	SSHURL                  *string `json:"sshUrl,omitempty"`
	DefaultBranch           string  `json:"defaultBranch"`
	PullRequestTargetBranch string  `json:"pullRequestTargetBranch,omitempty"`
	ProviderRepository      string  `json:"providerRepository,omitempty"`
	AnchorName              string  `json:"anchorName,omitempty"`
	GitCredentialSecret     string  `json:"gitCredentialSecret,omitempty"`
	Folder                  string  `json:"folder,omitempty"`
}

type ProjectConfig struct {
	Key          string             `json:"key"`
	WorkProvider string             `json:"workProvider,omitempty"`
	Repositories []RepositoryConfig `json:"repositories"`
}

func (p ProjectConfig) Repository(name string) (RepositoryConfig, bool) {
	for _, repository := range p.Repositories {
		if equalFold(repository.Name, name) {
			return repository, true
		}
	}
	return RepositoryConfig{}, false
}

type WorkflowConfig struct {
	TaskStart  StartOptions  `json:"taskStart"`
	TaskFinish FinishOptions `json:"taskFinish"`
}

type StartOptions struct {
	UpdateWorkItemState bool                `json:"updateWorkItemState"`
	CreateChildTasks    bool                `json:"createChildTasks"`
	States              []WorkItemTypeState `json:"states"`
}

type WorkItemTypeState struct{ Type, State string }

type FinishOptions struct {
	RunVerification      bool                 `json:"runVerification"`
	UpdateWorkItemState  bool                 `json:"updateWorkItemState"`
	States               []WorkItemTypeState  `json:"states"`
	VerificationCommands []RepositoryCommands `json:"verificationCommands"`
}

type RepositoryCommands struct {
	Repository string   `json:"repository"`
	Commands   []string `json:"commands"`
}

type StartRequest struct {
	Root         string   `json:"root"`
	WorkItemIDs  []string `json:"workItemIds"`
	Project      string   `json:"project,omitempty"`
	TaskID       *string  `json:"taskId,omitempty"`
	Type         string   `json:"type,omitempty"`
	Repositories []string `json:"repositories,omitempty"`
	Slug         string   `json:"slug,omitempty"`
}

type StartPlan struct {
	WorkItemIDs         []string              `json:"workItemIds"`
	PrimaryWorkItemID   string                `json:"primaryWorkItemId"`
	Project             string                `json:"project"`
	TaskID              *string               `json:"taskId"`
	Type                string                `json:"type"`
	Slug                string                `json:"slug"`
	BranchName          string                `json:"branchName"`
	SubjectName         string                `json:"subjectName"`
	Workspace           string                `json:"workspace"`
	Repositories        []string              `json:"repositories"`
	RepositoryFolders   []RepositoryFolder    `json:"repositoryFolders"`
	RepositoryWorktrees []StartRepositoryPlan `json:"repositoryWorktrees"`
}

type RepositoryFolder struct {
	Repository string `json:"repository"`
	Path       string `json:"path"`
}

type StartRepositoryPlan struct {
	Repository          string  `json:"repository"`
	ProjectRoot         string  `json:"projectRoot"`
	WorktreePath        string  `json:"worktreePath"`
	HTTPURL             string  `json:"httpUrl"`
	SSHURL              *string `json:"sshUrl"`
	DefaultBranch       string  `json:"defaultBranch"`
	AnchorName          string  `json:"anchorName"`
	GitCredentialSecret string  `json:"gitCredentialSecret,omitempty"`
	BranchName          string  `json:"branchName"`
}

func (p StartRepositoryPlan) MarshalJSON() ([]byte, error) {
	var secret *string
	if p.GitCredentialSecret != "" {
		value := p.GitCredentialSecret
		secret = &value
	}
	return json.Marshal(struct {
		Repository          string  `json:"repository"`
		ProjectRoot         string  `json:"projectRoot"`
		WorktreePath        string  `json:"worktreePath"`
		HTTPURL             string  `json:"httpUrl"`
		SSHURL              *string `json:"sshUrl"`
		DefaultBranch       string  `json:"defaultBranch"`
		AnchorName          string  `json:"anchorName"`
		GitCredentialSecret *string `json:"gitCredentialSecret"`
		BranchName          string  `json:"branchName"`
	}{p.Repository, p.ProjectRoot, p.WorktreePath, p.HTTPURL, p.SSHURL, p.DefaultBranch, p.AnchorName, secret, p.BranchName})
}

type RenamePlan struct {
	Workspace, NewWorkspace, OldSlug, NewSlug, OldBranch, NewBranch string
}

func (p RenamePlan) MarshalJSON() ([]byte, error) {
	return json.Marshal(struct {
		Workspace    string `json:"workspace"`
		NewWorkspace string `json:"newWorkspace"`
		OldSlug      string `json:"oldSlug"`
		NewSlug      string `json:"newSlug"`
		OldBranch    string `json:"oldBranch"`
		NewBranch    string `json:"newBranch"`
	}{p.Workspace, p.NewWorkspace, p.OldSlug, p.NewSlug, p.OldBranch, p.NewBranch})
}

type WorkItemUpdatePlan struct {
	Workspace    string     `json:"workspace"`
	NewWorkspace string     `json:"newWorkspace"`
	OldBranch    string     `json:"oldBranch"`
	NewBranch    string     `json:"newBranch"`
	WorkItems    []WorkItem `json:"workItems"`
}

type AddRepositoryPlan struct {
	Workspace, Repository, ProjectRoot, WorktreePath, HTTPURL  string
	SSHURL                                                     *string
	DefaultBranch, AnchorName, GitCredentialSecret, BranchName string
	Repositories                                               []string
}

func (p AddRepositoryPlan) MarshalJSON() ([]byte, error) {
	var secret *string
	if p.GitCredentialSecret != "" {
		value := p.GitCredentialSecret
		secret = &value
	}
	return json.Marshal(struct {
		Workspace           string   `json:"workspace"`
		Repository          string   `json:"repository"`
		ProjectRoot         string   `json:"projectRoot"`
		WorktreePath        string   `json:"worktreePath"`
		HTTPURL             string   `json:"httpUrl"`
		SSHURL              *string  `json:"sshUrl"`
		DefaultBranch       string   `json:"defaultBranch"`
		AnchorName          string   `json:"anchorName"`
		GitCredentialSecret *string  `json:"gitCredentialSecret"`
		BranchName          string   `json:"branchName"`
		Repositories        []string `json:"repositories"`
	}{p.Workspace, p.Repository, p.ProjectRoot, p.WorktreePath, p.HTTPURL, p.SSHURL, p.DefaultBranch, p.AnchorName, secret, p.BranchName, p.Repositories})
}

type RepositoryTarget struct {
	Repository          string  `json:"repository"`
	Path                string  `json:"path"`
	DefaultBranch       string  `json:"defaultBranch,omitempty"`
	SSHURL              *string `json:"sshUrl,omitempty"`
	GitCredentialSecret string  `json:"gitCredentialSecret,omitempty"`
}

type RepositoryStatus = gitrepo.RepositoryStatus

type TargetStatus struct {
	Target RepositoryTarget `json:"target"`
	Status RepositoryStatus `json:"status"`
}

type TeardownStep struct {
	Subject TeardownSubject `json:"subject"`
	Action  TeardownAction  `json:"action"`
}

type TeardownSubject struct {
	Type       string `json:"type"`
	Repository string `json:"repository,omitempty"`
}
type TeardownAction struct {
	Type         string `json:"type"`
	WorktreePath string `json:"worktreePath,omitempty"`
	GitDir       string `json:"gitDir,omitempty"`
	Workspace    string `json:"workspace,omitempty"`
}

type HandoffStatus string

const (
	HandoffTodo       HandoffStatus = "todo"
	HandoffInProgress HandoffStatus = "in_progress"
	HandoffDone       HandoffStatus = "done"
	HandoffBlocked    HandoffStatus = "blocked"
)

type HandoffSummary struct {
	Repository string        `json:"repository"`
	Status     HandoffStatus `json:"status"`
	Done       []string      `json:"done"`
	Decisions  []string      `json:"decisions"`
	Risks      []string      `json:"risks"`
	Blockers   []string      `json:"blockers"`
	FollowUp   []string      `json:"follow_up"`
}

type HandoffValidationItem struct {
	Repository    string                  `json:"repository"`
	Path          string                  `json:"path"`
	Status        string                  `json:"status"`
	Valid         bool                    `json:"valid"`
	Detail        HandoffValidationDetail `json:"detail"`
	DoneCount     int                     `json:"doneCount"`
	DecisionCount int                     `json:"decisionCount"`
	RiskCount     int                     `json:"riskCount"`
	BlockerCount  int                     `json:"blockerCount"`
	FollowUpCount int                     `json:"followUpCount"`
}
type HandoffValidationDetail struct {
	Kind   string `json:"kind"`
	Reason string `json:"reason,omitempty"`
}
type HandoffValidationReport struct {
	SchemaVersion string                  `json:"schemaVersion"`
	Workspace     string                  `json:"workspace"`
	Project       string                  `json:"project"`
	Items         []HandoffValidationItem `json:"items"`
	IsValid       bool                    `json:"isValid"`
}

type PreflightIssue struct {
	Code       string          `json:"code"`
	Severity   string          `json:"severity"`
	WorkItemID string          `json:"workItemId"`
	Detail     json.RawMessage `json:"detail"`
	RelatedIDs []string        `json:"relatedIds"`
}
type PreflightReport struct {
	SchemaVersion     string           `json:"schemaVersion"`
	Workspace         string           `json:"workspace"`
	Project           string           `json:"project"`
	WorkItemIDs       []string         `json:"workItemIds"`
	Issues            []PreflightIssue `json:"issues"`
	HasBlockingIssues bool             `json:"hasBlockingIssues"`
}

type VerificationResult struct {
	Repository     string `json:"repository"`
	Command        string `json:"command"`
	ExitCode       int    `json:"exitCode"`
	StandardOutput string `json:"standardOutput"`
	StandardError  string `json:"standardError"`
}

type PullRequestCandidate struct {
	Repository         string `json:"repository"`
	Path               string `json:"path"`
	ProviderRepository string `json:"providerRepository,omitempty"`
	TargetBranch       string `json:"targetBranch"`
}

func (p PullRequestCandidate) MarshalJSON() ([]byte, error) {
	var repository *string
	if p.ProviderRepository != "" {
		value := p.ProviderRepository
		repository = &value
	}
	return json.Marshal(struct {
		Repository         string  `json:"repository"`
		Path               string  `json:"path"`
		ProviderRepository *string `json:"providerRepository"`
		TargetBranch       string  `json:"targetBranch"`
	}{p.Repository, p.Path, repository, p.TargetBranch})
}

type PullRequestInput struct {
	ProviderRepository, SourceRefName, TargetRefName, Title, Description string
	IsDraft                                                              bool
	WorkItemIDs                                                          []string
}
type PullRequestResult struct {
	Repository    string  `json:"repository"`
	Action        string  `json:"action"`
	URL           *string `json:"url,omitempty"`
	PullRequestID *int64  `json:"pullRequestId,omitempty"`
	SkipReason    string  `json:"skipReason,omitempty"`
}

func (p PullRequestResult) MarshalJSON() ([]byte, error) {
	reason := p.SkipReason
	var skipReason *string
	if reason != "" {
		skipReason = &reason
	}
	return json.Marshal(struct {
		Repository    string  `json:"repository"`
		Action        string  `json:"action"`
		URL           *string `json:"url"`
		PullRequestID *int64  `json:"pullRequestId"`
		SkipReason    *string `json:"skipReason"`
	}{p.Repository, p.Action, p.URL, p.PullRequestID, skipReason})
}

type WorkPullRequest struct {
	ID  int64
	URL *string
}
type StartStateUpdate struct {
	ID          string `json:"id"`
	Label       string `json:"label"`
	TargetState string `json:"targetState"`
	Changed     bool   `json:"-"`
}
type StartExecutionReport struct {
	Plan         StartPlan          `json:"plan"`
	Manifest     Manifest           `json:"manifest"`
	WorkItems    []WorkItem         `json:"workItems"`
	ChildTasks   []ChildTask        `json:"childTasks"`
	StateUpdates []StartStateUpdate `json:"stateUpdates"`
	Events       []ActionEvent      `json:"-"`
}
type ActionEvent struct {
	Type            string `json:"-"`
	Repository      string `json:"-"`
	Operation       string `json:"-"`
	RepositoryCount int    `json:"-"`
	WorkItemID      string `json:"-"`
	Error           string `json:"-"`
}

func (event ActionEvent) MarshalJSON() ([]byte, error) {
	kinds := map[string]string{"preparingWorktree": "executing-start", "worktreePrepared": "executing-start", "workspaceCreated": "executing-start", "verifyingFinish": "verifying-finish", "finishVerificationCompleted": "finish-verification-completed", "runningGitOperation": "running-git-operation", "runningRepositoryGitOperation": "running-repository-git-operation", "gitOperationCompleted": "git-operation-completed", "skippingPullRequestCreation": "skipping-pull-request-creation", "checkingActivePullRequest": "checking-active-pull-request", "creatingPullRequest": "creating-pull-request", "pullRequestWorkItemLinkSkipped": "pull-request-work-item-link-skipped"}
	kind := kinds[event.Type]
	if kind == "" {
		kind = event.Type
	}
	operation := event.Operation
	if operation == "commitAndPush" {
		operation = "commit-and-push"
	}
	value := map[string]any{"kind": kind}
	if event.Repository != "" {
		value["repository"] = event.Repository
	}
	if operation != "" {
		value["operation"] = operation
	}
	if event.RepositoryCount != 0 {
		if event.Type == "verifyingFinish" {
			value["pull_request_candidate_count"] = event.RepositoryCount
		} else {
			value["repository_count"] = event.RepositoryCount
		}
	}
	if event.WorkItemID != "" {
		value["work_item_id"] = event.WorkItemID
	}
	if event.Error != "" {
		value["error"] = event.Error
	}
	return json.Marshal(value)
}

type RenameExecutionReport struct {
	Plan     RenamePlan `json:"plan"`
	Manifest Manifest   `json:"manifest"`
}
type SyncPlanReport struct {
	Workspace    string   `json:"workspace"`
	RequestedIDs []string `json:"requestedIds"`
}
type SyncReport struct {
	Workspace    string     `json:"workspace"`
	RequestedIDs []string   `json:"requestedIds"`
	Snapshots    []WorkItem `json:"snapshots"`
	Manifest     Manifest   `json:"manifest"`
}
type WorkItemUpdateReport struct {
	Action    string             `json:"action"`
	Plan      WorkItemUpdatePlan `json:"plan"`
	Manifest  Manifest           `json:"manifest"`
	Workspace string             `json:"newWorkspace"`
}
type AddRepositoryReport struct {
	Plan     AddRepositoryPlan `json:"plan"`
	Worktree WorktreeResult    `json:"worktree"`
	Manifest Manifest          `json:"manifest"`
}
type CommitPlanReport struct {
	Workspace  string         `json:"workspace"`
	BranchName string         `json:"branchName"`
	Message    string         `json:"message"`
	Targets    []TargetStatus `json:"targets"`
}
type CommitExecutionReport struct {
	Workspace  string   `json:"workspace"`
	BranchName string   `json:"branchName"`
	Message    string   `json:"message"`
	Committed  []string `json:"committed"`
}
type FinishPlanReport struct {
	Root                   string                  `json:"root"`
	Workspace              string                  `json:"workspace"`
	Manifest               Manifest                `json:"manifest"`
	Targets                []TargetStatus          `json:"targets"`
	Handoff                HandoffValidationReport `json:"handoff"`
	HandoffSummaries       []HandoffSummary        `json:"handoffSummaries"`
	CommitMessage          string                  `json:"commitMessage"`
	CreatePR               bool                    `json:"createPr"`
	Ready                  bool                    `json:"ready"`
	ChangedRepositories    []string                `json:"changedRepositories"`
	UnpushedRepositories   []string                `json:"unpushedRepositories"`
	ActionableRepositories []string                `json:"actionableRepositories"`
	PullRequestCandidates  []PullRequestCandidate  `json:"pullRequestCandidates"`
}
type FinishExecutionReport struct {
	Plan                FinishPlanReport      `json:"plan"`
	Events              []ActionEvent         `json:"events"`
	VerificationResults []VerificationResult  `json:"verificationResults"`
	GitActions          []GitAction           `json:"gitActions"`
	PullRequests        []PullRequestResult   `json:"pullRequests"`
	WorkItemUpdates     []WorkItemStateUpdate `json:"workItemUpdates"`
}
type GitAction struct {
	Repository string `json:"repository"`
	Operation  string `json:"operation"`
	Path       string `json:"path"`
}

func (action GitAction) MarshalJSON() ([]byte, error) {
	operation := action.Operation
	if operation == "commitAndPush" {
		operation = "commit-and-push"
	}
	return json.Marshal(struct {
		Repository string `json:"repository"`
		Operation  string `json:"operation"`
		Path       string `json:"path"`
	}{action.Repository, operation, action.Path})
}

type WorkItemStateUpdate struct {
	ID           string  `json:"id"`
	Label        string  `json:"label"`
	Type         *string `json:"kind"`
	CurrentState *string `json:"currentState"`
	TargetState  *string `json:"targetState"`
	Changed      bool    `json:"changed"`
	Outcome      string  `json:"outcome"`
}
type TeardownPlanReport struct {
	Workspace *string        `json:"workspace"`
	Steps     []TeardownStep `json:"steps"`
}
type TeardownExecutionReport struct {
	Workspace string         `json:"workspace"`
	Steps     []TeardownStep `json:"steps"`
}
type PruneSyncReport struct {
	Workspace string          `json:"workspace"`
	Status    string          `json:"status"`
	Detail    PruneSyncDetail `json:"detail"`
}
type PruneSyncDetail struct {
	Kind      string     `json:"kind"`
	Error     string     `json:"error,omitempty"`
	WorkItems []WorkItem `json:"workItems,omitempty"`
}
type PrunePlanReport struct {
	Root        string            `json:"root"`
	Project     *string           `json:"project"`
	WorkItemIDs []string          `json:"workItemIds"`
	Sync        []PruneSyncReport `json:"sync"`
	Candidates  []Summary         `json:"candidates"`
}
type PruneExecutionReport struct {
	Root    string   `json:"root"`
	Deleted []string `json:"deleted"`
}

type ConfigPort interface {
	Project(context.Context, string, string) (ProjectConfig, bool, error)
	Workflow(context.Context, string) (WorkflowConfig, error)
}
type WorkPort interface {
	GetWorkItems(context.Context, string, []string) ([]WorkItem, error)
	UpdateWorkItemState(context.Context, string, string, string) error
	CreateChildTask(context.Context, string, WorkItem, string, string) (ChildTask, error)
	FindActivePullRequest(context.Context, string, string, string) (*WorkPullRequest, error)
	CreatePullRequest(context.Context, string, PullRequestInput) (WorkPullRequest, error)
	LinkWorkItemToPullRequest(context.Context, string, string, int64, string) error
}
type GitPort interface {
	PrepareWorktree(context.Context, WorktreeRequest) (WorktreeResult, error)
	Status(context.Context, string) (RepositoryStatus, error)
	Update(context.Context, string, string, *gitrepo.Credential, *string) error
	Commit(context.Context, string, string) error
	Push(context.Context, string, string, bool) error
	HasCommitsAhead(context.Context, string, string) (bool, error)
	WorktreeRemove(context.Context, string, string) error
	WorktreePrune(context.Context, string) error
}
type VerificationPort interface {
	Run(context.Context, string, string) (exitCode int, stdout, stderr string)
}
type WorktreeRequest struct {
	ProjectRoot, Repository, HTTPURL, DefaultBranch, AnchorName, BranchName, WorktreePath string
	SSHURL                                                                                *string
	Credential                                                                            *gitrepo.Credential
}
type WorktreeResult struct {
	Repository   string                        `json:"repository"`
	Status       gitrepo.WorktreePrepareStatus `json:"status"`
	Detail       gitrepo.WorktreePrepareDetail `json:"detail"`
	WorktreePath string                        `json:"-"`
	GitDir       string                        `json:"-"`
	Created      bool                          `json:"-"`
}

type Clock interface{ Now() time.Time }
type realClock struct{}

func (realClock) Now() time.Time { return time.Now() }

type Engine struct {
	Config  ConfigPort
	Git     GitPort
	Secrets contract.SecretStore
	Work    WorkPort
	Verify  VerificationPort
	Clock   Clock
}

func NewEngine(config ConfigPort, git GitPort, secrets contract.SecretStore, work WorkPort) *Engine {
	return &Engine{Config: config, Git: git, Secrets: secrets, Work: work, Clock: realClock{}}
}

type FinishExecuteOptions struct {
	SkipVerification bool `json:"skipVerification"`
	ForceWithLease   bool `json:"forceWithLease"`
}

func (p StartPlan) MarshalJSON() ([]byte, error) {
	folders := make(map[string]string, len(p.RepositoryFolders))
	for _, folder := range p.RepositoryFolders {
		folders[folder.Repository] = folder.Path
	}
	return json.Marshal(struct {
		WorkItemIDs         []string              `json:"workItemIds"`
		PrimaryWorkItemID   string                `json:"primaryWorkItemId"`
		Project             string                `json:"project"`
		TaskID              *string               `json:"taskId"`
		Type                string                `json:"type"`
		Slug                string                `json:"slug"`
		BranchName          string                `json:"branchName"`
		SubjectName         string                `json:"subjectName"`
		Workspace           string                `json:"workspace"`
		Repositories        []string              `json:"repositories"`
		RepositoryFolders   map[string]string     `json:"repositoryFolders"`
		RepositoryWorktrees []StartRepositoryPlan `json:"repositoryWorktrees"`
	}{p.WorkItemIDs, p.PrimaryWorkItemID, p.Project, p.TaskID, p.Type, p.Slug, p.BranchName, p.SubjectName, p.Workspace, p.Repositories, folders, p.RepositoryWorktrees})
}

type StatusReport struct {
	Root  string     `json:"root"`
	Items []ListItem `json:"items"`
}
type ListReport struct {
	Root        string     `json:"root"`
	Project     *string    `json:"project"`
	WorkItemIDs []string   `json:"workItemIds"`
	Items       []ListItem `json:"items"`
}
type StartPlanReport struct {
	Root       string      `json:"root"`
	Plan       StartPlan   `json:"plan"`
	WorkItems  []WorkItem  `json:"workItems"`
	ChildTasks []ChildTask `json:"childTasks"`
}
type RepositoryLatestTarget struct {
	Repository          string  `json:"repository"`
	RepositoryPath      string  `json:"repository_path"`
	DefaultBranch       string  `json:"defaultBranch"`
	SSHURL              *string `json:"sshUrl"`
	GitCredentialSecret *string `json:"gitCredentialSecret"`
}
type RepositoryLatestPlanReport struct {
	Workspace  string                   `json:"workspace"`
	BranchName string                   `json:"branchName"`
	Targets    []RepositoryLatestTarget `json:"targets"`
}
type RepositoryLatestUpdate struct {
	Repository    string `json:"repository"`
	Path          string `json:"path"`
	DefaultBranch string `json:"defaultBranch"`
}
type RepositoryLatestExecutionReport struct {
	Workspace  string                   `json:"workspace"`
	BranchName string                   `json:"branchName"`
	Updated    []RepositoryLatestUpdate `json:"updated"`
}
type WorkItemChoicesReport struct {
	Workspace string     `json:"workspace"`
	Project   string     `json:"project"`
	Choices   []WorkItem `json:"choices"`
}
type ChildTaskReport struct {
	Workspace      string    `json:"workspace"`
	Repository     string    `json:"repository"`
	Parent         WorkItem  `json:"parent"`
	RequestedTitle string    `json:"requestedTitle"`
	Created        ChildTask `json:"created"`
	Manifest       Manifest  `json:"manifest"`
}
