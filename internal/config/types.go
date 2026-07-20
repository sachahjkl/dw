package config

import (
	"encoding/json"

	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
)

type ColorMode = contract.ColorMode

const (
	ColorAuto   = contract.ColorAuto
	ColorAlways = contract.ColorAlways
	ColorNever  = contract.ColorNever
)

var ColorModeChoices = []ColorMode{ColorAuto, ColorAlways, ColorNever}

type Agent = contract.Agent

const (
	AgentOpenCode    = contract.AgentOpenCode
	AgentCursor      = contract.AgentCursor
	AgentCursorAgent = contract.AgentCursorAgent
	AgentGeneric     = contract.AgentGeneric
	AgentClaude      = contract.AgentClaude
	AgentCodexCLI    = contract.AgentCodexCLI
	AgentCodex       = contract.AgentCodex
	AgentCopilot     = contract.AgentCopilot
)

var AgentDefaultChoices = []Agent{
	AgentOpenCode, AgentCursor, AgentClaude, AgentCodex, AgentCodexCLI, AgentCopilot,
}

type UserSettings struct {
	Root  *string    `json:"root"`
	Color *ColorMode `json:"color"`

	document *wirejson.Value
}

type NamedString struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

type AgentOptions struct {
	Default string `json:"default"`
}

// NamedRawConfiguration retains extension data without assigning provider
// meaning to it. Raw is the exact JSON value after parsing, not localized text.
type NamedRawConfiguration struct {
	Name string          `json:"name"`
	Raw  json.RawMessage `json:"value"`
}

// ProviderConfiguration is one named provider's opaque ordered JSON options.
type ProviderConfiguration = NamedRawConfiguration

type UpdateOptions struct {
	Owner             string `json:"owner"`
	Repository        string `json:"repository"`
	IncludePrerelease bool   `json:"includePrerelease"`
	AssetName         string `json:"assetName"`
}

type TaskStartOptions struct {
	UpdateWorkItemState *bool   `json:"updateWorkItemState,omitempty"`
	CreateChildTasks    *bool   `json:"createChildTasks,omitempty"`
	UserStoryState      *string `json:"userStoryState,omitempty"`
	AnomalyState        *string `json:"anomalyState,omitempty"`
	BugState            *string `json:"bugState,omitempty"`
	TaskState           *string `json:"taskState,omitempty"`
}

type RepositoryCommands struct {
	Repository string   `json:"repository"`
	Commands   []string `json:"commands"`
}

type TaskFinishOptions struct {
	RunVerification      *bool                `json:"runVerification,omitempty"`
	UpdateWorkItemState  *bool                `json:"updateWorkItemState,omitempty"`
	BugState             *string              `json:"bugState,omitempty"`
	TaskState            *string              `json:"taskState,omitempty"`
	VerificationCommands []RepositoryCommands `json:"verificationCommands"`
}

type WorkflowConfig struct {
	Schema          int                     `json:"schema,omitempty"`
	SchemaURL       string                  `json:"$schema,omitempty"`
	Providers       []ProviderConfiguration `json:"providers"`
	Updates         *UpdateOptions          `json:"updates,omitempty"`
	BranchPrefixes  []NamedString           `json:"branchPrefixes"`
	WorktreeFolders []NamedString           `json:"worktreeFolders"`
	Agent           *AgentOptions           `json:"agent,omitempty"`
	TaskStart       *TaskStartOptions       `json:"taskStart,omitempty"`
	TaskFinish      *TaskFinishOptions      `json:"taskFinish,omitempty"`

	document *wirejson.Value
}

func (workflow WorkflowConfig) BranchPrefix(kind string) (string, bool) {
	return namedString(workflow.BranchPrefixes, kind)
}

func (workflow WorkflowConfig) WorktreeFolder(repository string) (string, bool) {
	return namedString(workflow.WorktreeFolders, repository)
}

func namedString(values []NamedString, name string) (string, bool) {
	for _, value := range values {
		if value.Name == name {
			return value.Value, true
		}
	}
	return "", false
}

type ProjectsConfig struct {
	Schema    int            `json:"schema,omitempty"`
	SchemaURL string         `json:"$schema,omitempty"`
	Projects  []ProjectEntry `json:"projects"`

	document *wirejson.Value
}

type ProjectEntry struct {
	Key     string        `json:"key"`
	Project ProjectConfig `json:"project"`
	invalid bool
}

type ProjectChoice struct {
	Key   string `json:"key"`
	Label string `json:"label"`
}

func (choice ProjectChoice) String() string { return choice.Label }

type ProjectConfig struct {
	DisplayName      string                  `json:"displayName"`
	WorkProvider     string                  `json:"workProvider"`
	Providers        []ProviderConfiguration `json:"providers"`
	Repositories     []RepositoryEntry       `json:"repositories"`
	IncludedProjects []string                `json:"includedProjects,omitempty"`
	Agent            *AgentOptions           `json:"agent,omitempty"`
	Unknown          []NamedRawConfiguration `json:"-"`
}

type RepositoryEntry struct {
	Key        string           `json:"key"`
	Repository RepositoryConfig `json:"repository"`
}

type RepositoryConfig struct {
	URL                     RepositoryURL           `json:"url"`
	DefaultBranch           string                  `json:"defaultBranch"`
	PullRequestTargetBranch *string                 `json:"pullRequestTargetBranch,omitempty"`
	ProviderRepository      *string                 `json:"providerRepository,omitempty"`
	AnchorName              *string                 `json:"anchorName,omitempty"`
	GitCredentialSecret     *string                 `json:"gitCredentialSecret,omitempty"`
	Folder                  *string                 `json:"folder,omitempty"`
	Unknown                 []NamedRawConfiguration `json:"-"`
}

type RepositoryURL struct {
	HTTP string  `json:"http"`
	SSH  *string `json:"ssh,omitempty"`
}

func (url RepositoryURL) MarshalJSON() ([]byte, error) {
	return json.Marshal(struct {
		HTTP string  `json:"http"`
		SSH  *string `json:"ssh,omitempty"`
	}{HTTP: url.HTTP, SSH: url.SSH})
}

type DatabaseDefaults struct {
	ReadOnly       *bool `json:"readonly,omitempty"`
	MaxRows        *int  `json:"maxRows,omitempty"`
	TimeoutSeconds *int  `json:"timeoutSeconds,omitempty"`
}

type DatabaseConfig struct {
	Provider                            string                  `json:"provider"`
	ConnectionString                    *string                 `json:"connectionString,omitempty"`
	ConnectionStringEnvironmentVariable *string                 `json:"connectionStringEnvironmentVariable,omitempty"`
	CredentialKey                       *string                 `json:"credentialKey,omitempty"`
	ReadOnly                            *bool                   `json:"readonly,omitempty"`
	MaxRows                             *int                    `json:"maxRows,omitempty"`
	TimeoutSeconds                      *int                    `json:"timeoutSeconds,omitempty"`
	Unknown                             []NamedRawConfiguration `json:"-"`
}

type DatabaseEntry struct {
	Key      string         `json:"key"`
	Database DatabaseConfig `json:"database"`
}

type ProjectDatabases struct {
	Project   string          `json:"project"`
	Databases []DatabaseEntry `json:"databases"`
}

type DatabasesConfig struct {
	Schema    int                `json:"schema,omitempty"`
	SchemaURL string             `json:"$schema,omitempty"`
	Defaults  *DatabaseDefaults  `json:"defaults,omitempty"`
	Globals   []DatabaseEntry    `json:"globals"`
	Projects  []ProjectDatabases `json:"projects"`

	document *wirejson.Value
}

type ConfigShow struct {
	Root            string    `json:"root"`
	Color           ColorMode `json:"color"`
	SettingsPath    string    `json:"settingsPath"`
	WorkflowPath    string    `json:"workflowPath"`
	ProjectsPath    string    `json:"projectsPath"`
	DatabasesPath   string    `json:"databasesPath"`
	WorkflowExists  bool      `json:"workflowExists"`
	ProjectsExists  bool      `json:"projectsExists"`
	DatabasesExists bool      `json:"databasesExists"`
}

type ConfigDoctorReport struct {
	Root   string              `json:"root"`
	Checks []ConfigDoctorCheck `json:"checks"`
	Passed bool                `json:"passed"`
}

type ConfigDoctorCheck struct {
	Path    string  `json:"path"`
	Passed  bool    `json:"passed"`
	Message *string `json:"message"`
}

type RootStatus struct {
	Root         string   `json:"root"`
	Initialized  bool     `json:"initialized"`
	MissingPaths []string `json:"missingPaths"`
}

type InitRequest struct {
	Root    string `json:"root,omitempty"`
	Profile string `json:"profile"`
	NoSave  bool   `json:"no_save"`
	DryRun  bool   `json:"dry_run"`
}

type InitReport struct {
	Root         string   `json:"root"`
	Profile      string   `json:"profile"`
	DryRun       bool     `json:"dry_run"`
	NoSave       bool     `json:"no_save"`
	PlannedPaths []string `json:"planned_paths"`
}

type RefreshRequest struct {
	Root    string  `json:"root"`
	Profile *string `json:"profile,omitempty"`
}

type RefreshReport struct {
	Root    string `json:"root"`
	Profile string `json:"profile"`
}
