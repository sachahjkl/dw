package work

import (
	"context"

	"github.com/sachahjkl/dw/internal/wirejson"
)

// Provider supplies identity only. All operations are optional capabilities.
type Provider interface{ Name() ProviderName }

type Capability string

const (
	CapabilityAuthenticator     Capability = "authenticator"
	CapabilityItemReader        Capability = "item-reader"
	CapabilityAssignedQuerier   Capability = "assigned-querier"
	CapabilityRelationReader    Capability = "relation-reader"
	CapabilityStateWriter       Capability = "state-writer"
	CapabilityStateClassifier   Capability = "state-classifier"
	CapabilityChildCreator      Capability = "child-creator"
	CapabilityPullRequestReader Capability = "pull-request-reader"
	CapabilityPullRequestWriter Capability = "pull-request-writer"
	CapabilityRichContextReader Capability = "rich-context-reader"
	CapabilityRawItemReader     Capability = "raw-item-reader"
)

type Authenticator interface {
	Provider
	AuthStatus(context.Context, ProjectRef) (AuthStatus, error)
	Login(context.Context, ProjectRef, AuthMode, func(DeviceLogin) error) (AuthStatus, error)
	Logout(context.Context, ProjectRef) (removedLocalSession bool, err error)
}

type ItemReader interface {
	Provider
	ReadItems(context.Context, ProjectRef, []ItemID, ReadOptions) ([]Item, error)
}

type AssignedQuerier interface {
	Provider
	QueryAssigned(context.Context, ProjectRef, AssignedQuery) ([]Item, error)
}

type RelationReader interface {
	Provider
	ReadRelations(context.Context, ProjectRef, []ItemID) ([]Relation, error)
}

type StateWriter interface {
	Provider
	UpdateStates(context.Context, ProjectRef, []StateChange) ([]StateChangeResult, error)
}

type StateClassifier interface {
	Provider
	IsFinalState(ItemType, State) bool
}

type ChildCreator interface {
	Provider
	CreateChild(context.Context, ProjectRef, ChildCreate) (ChildCreateResult, error)
}

type PullRequestReader interface {
	Provider
	ListPullRequests(context.Context, ProjectRef, PullRequestQuery) ([]PullRequest, error)
	ActivePullRequest(context.Context, ProjectRef, RepositoryName, string) (*PullRequest, error)
	PullRequestWorkItemIDs(context.Context, ProjectRef, RepositoryName, PullRequestID) ([]ItemID, error)
}

type PullRequestWriter interface {
	Provider
	CreatePullRequest(context.Context, ProjectRef, PullRequestCreate) (PullRequestCreateResult, error)
	LinkPullRequestWorkItem(context.Context, ProjectRef, RepositoryName, PullRequestID, ItemID) error
}

type RichContextReader interface {
	Provider
	ReadRichContext(context.Context, ProjectRef, []ItemID, ReadOptions) ([]RichContext, error)
}

type RawItemReader interface {
	Provider
	ReadRawItem(context.Context, ProjectRef, ItemID) (wirejson.Value, error)
}
