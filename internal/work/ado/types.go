package ado

import (
	"encoding/json"
	"github.com/sachahjkl/dw/internal/l10n"
	"strconv"
)

const (
	DefaultAPIVersion         = "7.1"
	AIContextVersion          = "dw.ado.ai-context.v1"
	AttachmentDirectoryPrefix = "attachments/ado/"
	RelationHierarchyReverse  = "System.LinkTypes.Hierarchy-Reverse"
	RelationHierarchyForward  = "System.LinkTypes.Hierarchy-Forward"
	RelationDependencyReverse = "System.LinkTypes.Dependency-Reverse"
	RelationDependencyForward = "System.LinkTypes.Dependency-Forward"
)

type Options struct {
	Organization string `json:"organization"`
	Project      string `json:"project"`
	APIVersion   string `json:"apiVersion"`
}

// UnmarshalJSON keeps both persisted organization spellings compatible.
func (o *Options) UnmarshalJSON(data []byte) error {
	var value struct {
		Organization    string `json:"organization"`
		OrganizationURL string `json:"organizationUrl"`
		Project         string `json:"project"`
		APIVersion      string `json:"apiVersion"`
	}
	if err := json.Unmarshal(data, &value); err != nil {
		return err
	}
	o.Organization = value.Organization
	if o.Organization == "" {
		o.Organization = value.OrganizationURL
	}
	o.Project = value.Project
	o.APIVersion = value.APIVersion
	if o.APIVersion == "" {
		o.APIVersion = DefaultAPIVersion
	}
	return nil
}

type AuthOptions struct {
	TenantID string   `json:"tenantId,omitempty"`
	ClientID string   `json:"clientId,omitempty"`
	Scopes   []string `json:"scopes"`
}

type AuthScheme string

const (
	AuthBearer AuthScheme = "Bearer"
	AuthBasic  AuthScheme = "Basic"
)

type Token struct {
	AccessToken string     `json:"-"`
	Source      string     `json:"source"`
	Scheme      AuthScheme `json:"scheme"`
	ExpiresOn   *string    `json:"expires_on,omitempty"`
}

func (Token) String() string   { return "[REDACTED]" }
func (Token) GoString() string { return "ado.Token([REDACTED])" }

type AuthStatus struct {
	Connected bool    `json:"connected"`
	Source    *string `json:"source,omitempty"`
	ExpiresOn *string `json:"expires_on,omitempty"`
}

type DeviceLoginInstructions struct {
	VerificationURI     string `json:"verification_uri"`
	UserCode            string `json:"user_code"`
	ExpiresInSeconds    uint32 `json:"expires_in_seconds"`
	PollIntervalSeconds uint32 `json:"poll_interval_seconds"`
}

type WorkItemSnapshot struct {
	ID    string  `json:"id"`
	Type  *string `json:"type"`
	State *string `json:"state"`
	Title *string `json:"title"`
	URL   *string `json:"url"`
}

type WorkItemGroup struct {
	Parent WorkItemSnapshot   `json:"parent"`
	Items  []WorkItemSnapshot `json:"items"`
}

type ChildTaskCreateResult struct {
	Repository string `json:"repository"`
	ID         string `json:"id"`
	Title      string `json:"title"`
}

type PullRequestSummary struct {
	PullRequestID int64   `json:"pullRequestId"`
	URL           *string `json:"url"`
}

type PullRequestListItem struct {
	Repository    string   `json:"repository"`
	PullRequestID int64    `json:"pullRequestId"`
	Title         *string  `json:"title"`
	Status        *string  `json:"status"`
	SourceRefName *string  `json:"sourceRefName"`
	TargetRefName *string  `json:"targetRefName"`
	IsDraft       bool     `json:"isDraft"`
	CreatedBy     *string  `json:"createdBy"`
	URL           *string  `json:"url"`
	WebURL        *string  `json:"webUrl"`
	WorkItemIDs   []string `json:"workItemIds"`
}

type CreatePullRequestInput struct {
	Repository    string   `json:"repository"`
	SourceRefName string   `json:"sourceRefName"`
	TargetRefName string   `json:"targetRefName"`
	Title         string   `json:"title"`
	Description   string   `json:"description"`
	IsDraft       bool     `json:"isDraft"`
	WorkItemIDs   []string `json:"workItemIds"`
}

type PullRequestCreateResult struct {
	PullRequestID *int64  `json:"pullRequestId"`
	URL           *string `json:"url"`
}

type AIContextItem struct {
	SchemaVersion string               `json:"schemaVersion"`
	WorkItem      AIContextWorkItem    `json:"workItem"`
	Core          AIContextCore        `json:"core"`
	Content       AIContextContent     `json:"content"`
	Links         AIContextLinks       `json:"links"`
	Attachments   AIContextAttachments `json:"attachments"`
	Relations     []AIContextRelation  `json:"relations"`
	Comments      []AIContextComment   `json:"comments"`
}

type AIContextWorkItem struct {
	ID            string   `json:"id"`
	URL           *string  `json:"url"`
	Title         *string  `json:"title"`
	Type          *string  `json:"type"`
	State         *string  `json:"state"`
	AssignedTo    *string  `json:"assignedTo"`
	AreaPath      *string  `json:"areaPath"`
	IterationPath *string  `json:"iterationPath"`
	Tags          []string `json:"tags"`
}

type AIContextCore struct {
	CreatedBy   *string `json:"createdBy"`
	CreatedDate *string `json:"createdDate"`
	ChangedBy   *string `json:"changedBy"`
	ChangedDate *string `json:"changedDate"`
	Priority    *string `json:"priority"`
	ValueArea   *string `json:"valueArea"`
}

type OrderedField struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

type AIContextContent struct {
	Description        *string           `json:"description"`
	AcceptanceCriteria *string           `json:"acceptanceCriteria"`
	ProductContext     map[string]string `json:"productContext"`
}

type AIContextLinks struct {
	ParentIDs      []string `json:"parentIds"`
	ChildIDs       []string `json:"childIds"`
	PredecessorIDs []string `json:"predecessorIds"`
	SuccessorIDs   []string `json:"successorIds"`
}

type AIContextAttachments struct {
	DirectoryHint string                `json:"directoryHint"`
	Items         []AIContextAttachment `json:"items"`
}

type AIContextAttachment struct {
	Name          *string `json:"name"`
	URL           *string `json:"url"`
	Comment       *string `json:"comment"`
	DirectoryHint string  `json:"directoryHint"`
}

type AIContextRelation struct {
	Kind       string  `json:"kind"`
	Rel        *string `json:"rel"`
	WorkItemID *string `json:"workItemId"`
	Name       *string `json:"name"`
	URL        *string `json:"url"`
	Comment    *string `json:"comment"`
	Artifact   *string `json:"artifact"`
}

type AIContextComment struct {
	Author      *string `json:"author"`
	CreatedDate *string `json:"createdDate"`
	Text        *string `json:"text"`
}

type Event struct {
	Kind                string   `json:"kind"`
	Project             *string  `json:"project,omitempty"`
	VerificationURI     string   `json:"verification_uri,omitempty"`
	UserCode            string   `json:"user_code,omitempty"`
	ExpiresInSeconds    uint32   `json:"expires_in_seconds,omitempty"`
	PollIntervalSeconds uint32   `json:"poll_interval_seconds,omitempty"`
	Top                 int      `json:"top,omitempty"`
	Repositories        []string `json:"repositories,omitempty"`
	GitTo               string   `json:"git_to,omitempty"`
	ID                  string   `json:"id,omitempty"`
	IDs                 []string `json:"ids,omitempty"`
	State               string   `json:"state,omitempty"`
}

type ErrorKind string

const (
	ErrorInvalidInput ErrorKind = "invalid-input"
	ErrorMissingAuth  ErrorKind = "missing-auth"
	ErrorHTTP         ErrorKind = "http"
	ErrorRequest      ErrorKind = "request"
	ErrorJSON         ErrorKind = "json"
	ErrorOAuth        ErrorKind = "oauth"
	ErrorKeyring      ErrorKind = "keyring"
	ErrorLoginExpired ErrorKind = "login-expired"
	ErrorBrowserLogin ErrorKind = "browser-login"
)

type Error struct {
	Kind   ErrorKind
	Status int
	Body   string
	Cause  error
	Detail string
}

func (e *Error) Error() string {
	if e.Status != 0 {
		return "ado.error:" + string(e.Kind) + ":" + strconv.Itoa(e.Status)
	}
	return "ado.error:" + string(e.Kind)
}

func (e *Error) Localized() l10n.Message {
	switch e.Kind {
	case ErrorHTTP:
		return l10n.M("ado.error.http", l10n.A("status", e.Status), l10n.A("body", e.Body))
	case ErrorMissingAuth:
		return l10n.M("ado.error.missing-auth")
	case ErrorOAuth:
		return l10n.M("ado.error.oauth", l10n.A("detail", e.Detail))
	case ErrorKeyring:
		return l10n.M("ado.error.keyring", l10n.A("detail", e.Detail))
	case ErrorLoginExpired:
		return l10n.M("ado.error.login-expired")
	case ErrorBrowserLogin:
		return l10n.M("ado.error.browser", l10n.A("detail", e.Detail))
	case ErrorJSON:
		return l10n.M("ado.error.json", l10n.A("detail", e.Detail))
	case ErrorRequest:
		return l10n.M("ado.error.request", l10n.A("detail", e.Detail))
	default:
		return l10n.M("ado.error.invalid-input", l10n.A("detail", e.Detail))
	}
}

func (e *Error) Unwrap() error { return e.Cause }
