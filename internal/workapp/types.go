package workapp

import (
	"context"
	"encoding/json"

	"github.com/sachahjkl/dw/internal/agent"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/workspace"
)

var (
	ErrProjectRequired      = problem(msgProjectRequired, "a configured project is required")
	ErrWorkItemsRequired    = problem(msgItemsRequired, "at least one work item is required")
	ErrRepositoriesRequired = problem(msgRepositoriesRequired, "at least one work repository is required")
	ErrInvalidHandoff       = problem(msgInvalidHandoff, "workspace finish blocked: invalid handoff. Fix or complete handoffs before pushing")
)

type EventSink func(context.Context, Event) error
type AuthLoginMode string

const (
	AuthLoginBrowser        AuthLoginMode = "Browser"
	AuthLoginDeviceCode     AuthLoginMode = "DeviceCode"
	AuthLoginEnvironmentPAT AuthLoginMode = "EnvironmentPat"
)

type AuthLoginRequest struct {
	Provider    string        `json:"provider,omitempty"`
	Root        string        `json:"root,omitempty"`
	Mode        AuthLoginMode `json:"mode"`
	OpenBrowser bool          `json:"open_browser,omitempty"`
}

type AuthLoginReport struct {
	Mode               AuthLoginMode `json:"mode"`
	Source             *string       `json:"source,omitempty"`
	ExpiresOn          *string       `json:"expires_on,omitempty"`
	UsesEnvironmentPAT bool          `json:"uses_environment_pat"`
	Events             []Event       `json:"events,omitempty"`
}

type AuthStatusRequest struct {
	Provider string `json:"provider,omitempty"`
	Root     string `json:"root,omitempty"`
}
type AuthStatusReport struct {
	Connected bool    `json:"connected"`
	Source    *string `json:"source,omitempty"`
	ExpiresOn *string `json:"expires_on,omitempty"`
}
type AuthLogoutRequest struct {
	Provider string `json:"provider,omitempty"`
	Root     string `json:"root,omitempty"`
}
type AuthLogoutReport struct {
	RemovedLocalSession bool `json:"removed_local_session"`
}

type AssignedRequest struct {
	Provider           string `json:"provider,omitempty"`
	Root               string `json:"root,omitempty"`
	Project            string `json:"project,omitempty"`
	Top                int    `json:"top"`
	IncludeFinalStates bool   `json:"includeFinalStates"`
	GroupByParent      bool   `json:"groupByParent"`
}

type AssignedReport struct {
	Root               string         `json:"root"`
	Project            string         `json:"project"`
	Top                int            `json:"top"`
	IncludeFinalStates bool           `json:"includeFinalStates"`
	GroupByParent      bool           `json:"groupByParent"`
	Items              []ItemSnapshot `json:"items"`
	Groups             []ItemGroup    `json:"groups"`
	Events             []Event        `json:"events"`
}

type PullRequestsRequest struct {
	Provider     string   `json:"provider,omitempty"`
	Root         string   `json:"root,omitempty"`
	Project      string   `json:"project"`
	Repositories []string `json:"repositories"`
}

type PullRequestsReport struct {
	Root         string            `json:"root"`
	Project      string            `json:"project"`
	Repositories []string          `json:"repositories"`
	Items        []PullRequestItem `json:"items"`
	Events       []Event           `json:"events"`
}

type ItemShowRequest struct {
	Provider string   `json:"provider,omitempty"`
	Root     string   `json:"root,omitempty"`
	Project  string   `json:"project"`
	IDs      []string `json:"ids"`
}
type ItemShowReport struct {
	Root         string         `json:"root"`
	Project      string         `json:"project"`
	RequestedIDs []string       `json:"requestedIds"`
	Items        []ItemSnapshot `json:"items"`
	Events       []Event        `json:"events"`
}

type StatePlanRequest struct {
	Provider string   `json:"provider,omitempty"`
	Root     string   `json:"root,omitempty"`
	Project  string   `json:"project"`
	IDs      []string `json:"ids"`
	State    string   `json:"state"`
	History  string   `json:"history"`
}
type StatePlanReport struct {
	Provider string   `json:"provider,omitempty"`
	Root     string   `json:"root"`
	Project  string   `json:"project"`
	IDs      []string `json:"ids"`
	State    string   `json:"state"`
	History  string   `json:"history"`
}
type StateUpdate struct {
	ID    string `json:"id"`
	State string `json:"state"`
}
type StateExecutionReport struct {
	Plan    StatePlanReport `json:"plan"`
	Events  []Event         `json:"events"`
	Updated []StateUpdate   `json:"updated"`
}

type ContextMode string

const (
	ContextRich ContextMode = "ai-context"
	ContextRaw  ContextMode = "expanded"
)

type ContextRequest struct {
	Provider        string      `json:"provider,omitempty"`
	Root            string      `json:"root,omitempty"`
	Organization    string      `json:"organization,omitempty"`
	Project         string      `json:"project"`
	IDs             []string    `json:"ids"`
	Summary         bool        `json:"summary"`
	Comments        int         `json:"comments"`
	IncludeComments bool        `json:"include_comments"`
	Mode            ContextMode `json:"mode"`
}
type ContextReport struct {
	Root            string            `json:"root"`
	Project         string            `json:"project,omitempty"`
	RequestedIDs    []string          `json:"requestedIds"`
	Summary         bool              `json:"summary"`
	Comments        int               `json:"comments"`
	IncludeComments bool              `json:"includeComments,omitempty"`
	Expanded        []json.RawMessage `json:"expanded"`
	Items           []RichContextItem `json:"items"`
	Events          []Event           `json:"events"`
}

type ChangelogFormat string

const (
	ChangelogRaw      ChangelogFormat = "raw"
	ChangelogMarkdown ChangelogFormat = "markdown"
	ChangelogHTML     ChangelogFormat = "html"
)

type ChangelogSourceKind string

const (
	ChangelogWorkItems    ChangelogSourceKind = "work-items"
	ChangelogPullRequests ChangelogSourceKind = "pull-requests"
	ChangelogGitRange     ChangelogSourceKind = "git-range"
)

type ChangelogRequest struct {
	Provider       string              `json:"provider,omitempty"`
	Root           string              `json:"root,omitempty"`
	Project        string              `json:"project"`
	Source         ChangelogSourceKind `json:"source"`
	WorkItemIDs    []string            `json:"work_item_ids"`
	PullRequestIDs []int64             `json:"pull_request_ids"`
	GitFrom        string              `json:"git_from"`
	GitTo          string              `json:"git_to"`
	Repositories   []string            `json:"repositories"`
	GroupByParent  bool                `json:"group_by_parent"`
	Format         ChangelogFormat     `json:"format"`
	Table          bool                `json:"table"`
	IDsOnly        bool                `json:"ids_only"`
}
type ChangelogWarning struct {
	Detail string `json:"detail"`
}
type ChangelogSection struct {
	Repository     *string            `json:"repository"`
	RepositoryPath *string            `json:"repositoryPath"`
	WorkItemIDs    []string           `json:"workItemIds"`
	Items          []ItemSnapshot     `json:"items"`
	Groups         []ItemGroup        `json:"groups"`
	SourceEmpty    bool               `json:"sourceEmpty"`
	ResolvedEmpty  bool               `json:"resolvedEmpty"`
	Warnings       []ChangelogWarning `json:"warnings"`
}
type ChangelogReport struct {
	Root          string             `json:"root"`
	Project       string             `json:"project"`
	FromPR        bool               `json:"fromPr"`
	FromGit       bool               `json:"fromGit"`
	GroupByParent bool               `json:"groupByParent"`
	Format        ChangelogFormat    `json:"format"`
	Table         bool               `json:"table"`
	IDsOnly       bool               `json:"idsOnly"`
	WorkItemIDs   []string           `json:"workItemIds"`
	Sections      []ChangelogSection `json:"sections"`
	Events        []Event            `json:"events"`
}

type GitChangelogSection struct {
	Repository, Path string
	WorkItemIDs      []string
	Warnings         []ChangelogWarning
	SourceEmpty      bool
}
type GitChangelogPort interface {
	ResolveGitRange(context.Context, ChangelogRequest) ([]GitChangelogSection, error)
}

type DoingRequest struct {
	Provider string            `json:"provider,omitempty"`
	Root     string            `json:"root,omitempty"`
	Project  string            `json:"project"`
	IDs      []string          `json:"ids"`
	States   map[string]string `json:"states"`
}
type DoingActionRequest struct {
	Request  DoingRequest `json:"request"`
	Approved bool         `json:"approved"`
}
type DoingPlanUpdate struct {
	ID           string  `json:"id"`
	Type         string  `json:"type"`
	CurrentState *string `json:"currentState,omitempty"`
	TargetState  string  `json:"targetState"`
	Changed      bool    `json:"changed"`
}
type DoingPlanReport struct {
	Provider string            `json:"provider,omitempty"`
	Root     string            `json:"root"`
	Project  string            `json:"project"`
	Updates  []DoingPlanUpdate `json:"updates"`
}
type DoingUpdate struct {
	ID    string `json:"id"`
	State string `json:"state"`
}
type DoingExecutionReport struct {
	Plan    DoingPlanReport `json:"plan"`
	Events  []Event         `json:"events"`
	Updated []DoingUpdate   `json:"updated"`
}
type DoingActionResult struct {
	Plan      DoingPlanReport       `json:"plan"`
	Execution *DoingExecutionReport `json:"execution,omitempty"`
}

type StartRequest struct {
	Provider               string            `json:"provider,omitempty"`
	Root                   string            `json:"root,omitempty"`
	Project                string            `json:"project"`
	WorkItemIDs            []string          `json:"work_item_ids"`
	TaskID                 *string           `json:"task_id,omitempty"`
	Type                   string            `json:"type"`
	Repositories           []string          `json:"repositories"`
	Slug                   string            `json:"slug"`
	SkipWork               bool              `json:"skip_work"`
	WithActiveChildren     bool              `json:"with_active_children"`
	CreateChildTasks       bool              `json:"create_child_tasks"`
	RequiredChildTaskTypes []string          `json:"required_child_task_types"`
	Execute                bool              `json:"execute"`
	Approved               bool              `json:"approved"`
	PromptToExecute        bool              `json:"prompt_to_execute"`
	PromptToOpen           bool              `json:"prompt_to_open"`
	States                 map[string]string `json:"states"`
}
type StartPlanReport struct {
	Root       string                `json:"root"`
	Plan       workspace.StartPlan   `json:"plan"`
	WorkItems  []workspace.WorkItem  `json:"workItems"`
	ChildTasks []workspace.ChildTask `json:"childTasks"`
	Provider   string                `json:"provider,omitempty"`
}
type StartStateUpdate struct {
	ID          string `json:"id"`
	Label       string `json:"label"`
	TargetState string `json:"targetState"`
	Changed     bool   `json:"changed"`
}
type StartExecutionReport struct {
	Plan         workspace.StartPlan   `json:"plan"`
	Manifest     workspace.Manifest    `json:"manifest"`
	WorkItems    []workspace.WorkItem  `json:"workItems"`
	ChildTasks   []workspace.ChildTask `json:"childTasks"`
	StateUpdates []StartStateUpdate    `json:"stateUpdates"`
	Events       []Event               `json:"events"`
}
type StartPullRequestRequest struct {
	Provider             string            `json:"provider,omitempty"`
	Root                 string            `json:"root,omitempty"`
	Project              string            `json:"project"`
	PullRequestID        int64             `json:"pull_request_id"`
	Repositories         []string          `json:"repositories"`
	ProviderRepositories []string          `json:"provider_repositories"`
	Type                 string            `json:"type"`
	Slug                 string            `json:"slug"`
	Execute              bool              `json:"execute"`
	States               map[string]string `json:"states"`
}
type StartPullRequestPlanReport struct {
	PullRequestID        int64           `json:"pullRequestId"`
	Repositories         []string        `json:"repositories"`
	ProviderRepositories []string        `json:"providerRepositories"`
	WorkItemIDs          []string        `json:"workItemIds"`
	Start                StartPlanReport `json:"start"`
}

type OpenRequest struct {
	Provider      string   `json:"provider,omitempty"`
	Root          string   `json:"root,omitempty"`
	Project       string   `json:"project"`
	Workspace     *string  `json:"workspace,omitempty"`
	WorkItemIDs   []string `json:"work_item_ids"`
	PullRequestID *int64   `json:"pull_request_id,omitempty"`
	Continue      bool     `json:"continue"`
	ResolveOnly   bool     `json:"resolve_only"`
	Repository    string   `json:"repository,omitempty"`
	Agent         string   `json:"agent,omitempty"`
}
type OpenReport struct {
	Workspace string        `json:"workspace"`
	Launch    *agent.Launch `json:"launch,omitempty"`
	Events    []Event       `json:"events"`
}

type SyncRequest struct {
	Provider    string   `json:"provider,omitempty"`
	Root        string   `json:"root,omitempty"`
	Project     string   `json:"project"`
	Workspace   *string  `json:"workspace,omitempty"`
	WorkItemIDs []string `json:"work_item_ids"`
	Continue    bool     `json:"continue"`
}
type SyncReport struct {
	Workspace    string               `json:"workspace"`
	RequestedIDs []string             `json:"requestedIds"`
	Snapshots    []workspace.WorkItem `json:"snapshots"`
	Manifest     workspace.Manifest   `json:"manifest"`
	Events       []Event              `json:"events"`
}

type ContextRefreshRequest struct {
	Provider    string   `json:"provider,omitempty"`
	Root        string   `json:"root,omitempty"`
	Project     string   `json:"project"`
	Workspace   *string  `json:"workspace,omitempty"`
	WorkItemIDs []string `json:"work_item_ids"`
	Continue    bool     `json:"continue"`
}
type ContextRefreshReport struct {
	Workspace   string `json:"workspace"`
	ContextFile string `json:"contextFile"`
	ItemCount   int    `json:"itemCount"`
}

type ChildRequest struct {
	Provider    string   `json:"provider,omitempty"`
	Root        string   `json:"root,omitempty"`
	Project     string   `json:"project"`
	Workspace   *string  `json:"workspace,omitempty"`
	WorkItemIDs []string `json:"work_item_ids"`
	Continue    bool     `json:"continue"`
	Repository  string   `json:"repository"`
	Title       string   `json:"title"`
	ExactTitle  bool     `json:"exact_title"`
}
type ChildReport struct {
	Workspace      string             `json:"workspace"`
	Repository     string             `json:"repository"`
	Parent         workspace.WorkItem `json:"parent"`
	RequestedTitle string             `json:"requestedTitle"`
	Created        ChildCreateResult  `json:"created"`
	Manifest       workspace.Manifest `json:"manifest"`
	Events         []Event            `json:"events"`
}

type PruneRequest struct {
	Provider           string   `json:"provider,omitempty"`
	Root               string   `json:"root,omitempty"`
	Project            *string  `json:"project,omitempty"`
	WorkItemIDs        []string `json:"work_item_ids"`
	SelectedWorkspaces []string `json:"selected_workspaces"`
	Execute            bool     `json:"execute"`
	Approved           bool     `json:"approved"`
	NoSync             bool     `json:"no_sync"`
}
type PruneReport struct {
	Plan      workspace.PrunePlanReport       `json:"plan"`
	Execution *workspace.PruneExecutionReport `json:"execution,omitempty"`
	Events    []Event                         `json:"events"`
}

type FinishRequest struct {
	Provider       string            `json:"provider,omitempty"`
	Root           string            `json:"root,omitempty"`
	Workspace      *string           `json:"workspace,omitempty"`
	Continue       bool              `json:"continue"`
	Execute        bool              `json:"execute"`
	Approved       bool              `json:"approved"`
	CreatePR       bool              `json:"create_pr"`
	Ready          bool              `json:"ready"`
	SkipVerify     bool              `json:"skip_verify"`
	SkipWork       bool              `json:"skip_work"`
	ForceWithLease bool              `json:"force_with_lease"`
	Message        *string           `json:"message,omitempty"`
	FinishStates   map[string]string `json:"finish_states"`
}
type FinishReport struct {
	Plan      workspace.FinishPlanReport       `json:"plan"`
	Execution *workspace.FinishExecutionReport `json:"execution,omitempty"`
	Events    []Event                          `json:"events"`
}

// Workspace ports are deliberately consumer-owned so orchestration depends on
// only the lifecycle operation used by each action.
type ChoiceOption struct {
	Value string
	Label l10n.Message
}
type InteractiveCatalog interface {
	ProjectChoices(context.Context, string) ([]ChoiceOption, error)
	RepositoryChoices(context.Context, string, string) ([]ChoiceOption, error)
}

type WorkspaceLookup interface {
	Resolve(context.Context, string, *string, string, []string, bool) (string, error)
	Manifest(context.Context, string) (workspace.Manifest, error)
}
type WorkspaceStarter interface {
	PlanStart(context.Context, workspace.StartRequest) (workspace.StartPlan, error)
	ExecuteStart(context.Context, workspace.StartPlan, []workspace.WorkItem, []workspace.ChildTask, func(workspace.ActionEvent)) (workspace.StartExecutionReport, error)
}
type WorkspaceSyncer interface {
	ApplySnapshots(context.Context, string, []workspace.WorkItem) (workspace.Manifest, error)
}
type WorkspaceChildWriter interface {
	AddChild(context.Context, string, workspace.ChildTask) (workspace.Manifest, error)
}
type WorkspaceOpener interface {
	Open(context.Context, string, string, string, bool) (agent.Launch, error)
}
type WorkspacePruner interface {
	Find(context.Context, string, *string, []string) ([]workspace.Summary, error)
	PlanPrune(context.Context, string, *string, []string) ([]workspace.Summary, error)
	ExecutePrune(context.Context, string, []workspace.Summary) (workspace.PruneExecutionReport, error)
}
type WorkspaceFinisher interface {
	PlanFinish(context.Context, string, string, string, bool, bool) (workspace.FinishPlanReport, error)
	ExecuteLocalFinish(context.Context, workspace.FinishPlanReport, workspace.FinishExecuteOptions, func(workspace.ActionEvent)) (workspace.FinishExecutionReport, error)
}
