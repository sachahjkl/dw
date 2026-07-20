// Package work defines provider-neutral work tracking and pull-request
// contracts. Concrete Azure DevOps code belongs in a child package.
package work

import (
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/wirejson"
)

type ProviderName string
type ItemID = contract.WorkItemID
type ItemType = contract.WorkItemType
type State = contract.WorkItemState
type RepositoryName = contract.RepositoryName
type PullRequestID = contract.PullRequestID

// ProjectRef identifies a project without assuming a provider's URL grammar.
type ProjectRef struct {
	Key          contract.ProjectKey
	Root         string
	Organization string
	Project      string
}

// Item is the stable summary shared by task/workspace use cases.
type Item struct {
	ID            ItemID
	Type          ItemType
	State         State
	Title         string
	URL           string
	AssignedTo    string
	AreaPath      string
	IterationPath string
	ParentID      contract.Optional[ItemID]
	Tags          []string
}

type ReadOptions struct {
	IncludeRelations bool
	IncludeComments  bool
	CommentLimit     int
}

type AssignedQuery struct {
	Top                int
	ExcludeFinalStates bool
}

type RelationKind string

const (
	RelationParent      RelationKind = "parent"
	RelationChild       RelationKind = "child"
	RelationPredecessor RelationKind = "predecessor"
	RelationSuccessor   RelationKind = "successor"
	RelationAttachment  RelationKind = "attachment"
	RelationOther       RelationKind = "other"
)

type Relation struct {
	SourceID ItemID
	Kind     RelationKind
	TargetID contract.Optional[ItemID]
	Name     string
	URL      string
	Comment  string
	Artifact string
}

type StateChange struct {
	ID      ItemID
	State   State
	Comment string
}

type StateChangeResult struct {
	ID       ItemID
	Previous State
	Current  State
	Changed  bool
}

type ChildCreate struct {
	ParentID  ItemID
	Type      ItemType
	Title     string
	State     State
	Iteration string
	Area      string
	History   string
}

type ChildCreateResult struct {
	ID    ItemID
	Title string
	URL   string
}

type PullRequest struct {
	ID          PullRequestID
	Repository  RepositoryName
	Title       string
	Status      string
	SourceRef   string
	TargetRef   string
	Draft       bool
	CreatedBy   string
	URL         string
	WebURL      string
	WorkItemIDs []ItemID
}

type PullRequestQuery struct {
	Repositories []RepositoryName
	Status       string
}

type PullRequestCreate struct {
	Repository  RepositoryName
	SourceRef   string
	TargetRef   string
	Title       string
	Description string
	Draft       bool
	WorkItemIDs []ItemID
}

type PullRequestCreateResult struct {
	ID     PullRequestID
	URL    string
	WebURL string
}

type Comment struct {
	Author    string
	CreatedAt contract.Timestamp
	Text      string
}

type Attachment struct {
	Name    string
	URL     string
	Comment string
}

// RichContext is provider-neutral while Extra retains unknown provider fields
// with order and null fidelity.
type RichContext struct {
	Item               Item
	Description        string
	AcceptanceCriteria string
	CreatedBy          string
	CreatedDate        contract.Timestamp
	ChangedBy          string
	ChangedDate        contract.Timestamp
	Priority           string
	ValueArea          string
	ProductContext     map[string]string
	Relations          []Relation
	Comments           []Comment
	Attachments        []Attachment
	Extra              wirejson.Value
}

type AuthMode string

const (
	AuthEnvironment AuthMode = "environment"
	AuthBrowser     AuthMode = "browser"
	AuthDevice      AuthMode = "device"
)

type AuthStatus struct {
	Authenticated bool
	Source        string
	Principal     string
	ExpiresOn     contract.Optional[contract.Timestamp]
}

type DeviceLogin struct {
	VerificationURI     string
	UserCode            string
	ExpiresInSeconds    uint32
	PollIntervalSeconds uint32
}
