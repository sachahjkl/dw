package cockpit

import (
	"context"
	"fmt"

	"github.com/sachahjkl/dw/internal/action"
)

type Risk string

const (
	RiskSafe        Risk = "safe"
	RiskPreview     Risk = "preview"
	RiskDestructive Risk = "destructive"
	RiskExternal    Risk = "external"
)

type Operation struct {
	Relation       Relation
	Subject        ResourceRef
	Label          string
	Description    string
	Request        action.Request
	Inputs         []OperationInput
	Build          RequestBuilder
	Risk           Risk
	Active         bool
	DisabledReason string
}

type Snapshot struct {
	Ref              ResourceRef
	Root             string
	NeedsInit        bool
	ProjectCount     int
	RepositoryCount  int
	PruneCandidates  int
	DefaultAgent     string
	ColorMode        string
	DoctorOK         bool
	Projects         []string
	Repositories     []string
	WorkProviders    []string
	ProjectProviders map[string]string
	DataProviders    []string
	States           []string
	SecretKeys       []string
	Environment      []string
	Workspaces       []Workspace
	WorkProjects     []WorkProject
	PullRequests     []PullRequest
	DataSources      []DataSource
	Cockpit          []CockpitItem
	Operations       []Operation
	InitOperation    *Operation
}

type Workspace struct {
	Ref          ResourceRef
	Path         string
	Project      string
	Kind         string
	WorkspaceID  string
	Title        string
	WorkItems    []string
	Type         string
	Slug         string
	Branch       string
	Repositories []string
	Operations   []Operation
}

type WorkProject struct {
	Ref        ResourceRef
	Operations []Operation
	Key        string
	Label      string
	Provider   string
	Error      string
	Items      []WorkItem
}

type WorkItem struct {
	Ref        ResourceRef
	ID         string
	Type       string
	State      string
	Title      string
	URL        string
	Operations []Operation
}

type PullRequest struct {
	Ref          ResourceRef
	ID           string
	Project      string
	Provider     string
	Repository   string
	Branch       string
	TargetBranch string
	Title        string
	Draft        bool
	Workspace    string
	WorkItems    []string
	URL          string
	Error        string
	Operations   []Operation
}

type DataSource struct {
	Ref        ResourceRef
	Project    string
	Key        string
	Provider   string
	Operations []Operation
}

type CockpitItem struct {
	Ref      ResourceRef
	Section  string
	Title    string
	Subtitle string
	Status   string
	Severity Risk
	Primary  Operation
}

type SnapshotLoader func(context.Context, string) (Snapshot, error)
type WorkLoader func(context.Context, Snapshot) ([]WorkProject, error)
type PullRequestLoader func(context.Context, Snapshot) ([]PullRequest, error)

type Service struct {
	snapshot     SnapshotLoader
	work         WorkLoader
	pullRequests PullRequestLoader
}

func New(snapshot SnapshotLoader, work WorkLoader, pullRequests PullRequestLoader) (*Service, error) {
	if snapshot == nil || work == nil || pullRequests == nil {
		return nil, fmt.Errorf("cockpit.invalid-service-dependency")
	}
	return &Service{snapshot: snapshot, work: work, pullRequests: pullRequests}, nil
}

func (service *Service) Snapshot(ctx context.Context, root string) (Snapshot, error) {
	if service == nil || service.snapshot == nil {
		return Snapshot{}, fmt.Errorf("cockpit.service-required")
	}
	return service.snapshot(ctx, root)
}

func (service *Service) Work(ctx context.Context, snapshot Snapshot) ([]WorkProject, error) {
	if service == nil || service.work == nil {
		return nil, fmt.Errorf("cockpit.service-required")
	}
	return service.work(ctx, snapshot)
}

func (service *Service) PullRequests(ctx context.Context, snapshot Snapshot) ([]PullRequest, error) {
	if service == nil || service.pullRequests == nil {
		return nil, fmt.Errorf("cockpit.service-required")
	}
	return service.pullRequests(ctx, snapshot)
}

func (service *Service) Resolve(ctx context.Context, ref ResourceRef, relation Relation, values []InputValue) (Operation, action.Request, error) {
	if err := ref.Validate(); err != nil {
		return Operation{}, nil, err
	}
	if !relation.Valid() {
		return Operation{}, nil, fmt.Errorf("cockpit.operation-relation-invalid")
	}
	snapshot, err := service.Snapshot(ctx, ref.Root)
	if err != nil {
		return Operation{}, nil, err
	}
	operations, found, err := service.resourceOperations(ctx, snapshot, ref)
	if err != nil {
		return Operation{}, nil, err
	}
	if !found {
		return Operation{}, nil, fmt.Errorf("cockpit.resource-not-found")
	}
	var match *Operation
	for index := range operations {
		if operations[index].Relation != relation {
			continue
		}
		if match != nil {
			return Operation{}, nil, fmt.Errorf("cockpit.operation-ambiguous")
		}
		match = &operations[index]
	}
	if match == nil {
		return Operation{}, nil, fmt.Errorf("cockpit.operation-not-found")
	}
	if !match.Subject.Equal(ref) {
		return Operation{}, nil, fmt.Errorf("cockpit.operation-subject-mismatch")
	}
	if !match.Active {
		if match.DisabledReason != "" {
			return Operation{}, nil, fmt.Errorf("cockpit.operation-disabled: %s", match.DisabledReason)
		}
		return Operation{}, nil, fmt.Errorf("cockpit.operation-disabled")
	}
	request, err := match.BuildRequest(values)
	if err != nil {
		return Operation{}, nil, err
	}
	return *match, request, nil
}

func (service *Service) resourceOperations(ctx context.Context, snapshot Snapshot, ref ResourceRef) ([]Operation, bool, error) {
	switch ref.Kind {
	case ResourceRoot:
		if !snapshot.Ref.Equal(ref) {
			return nil, false, nil
		}
		return snapshot.Operations, true, nil
	case ResourceWorkspace:
		for index := range snapshot.Workspaces {
			if snapshot.Workspaces[index].Ref.Equal(ref) {
				return snapshot.Workspaces[index].Operations, true, nil
			}
		}
		return nil, false, nil
	case ResourceDataSource:
		for index := range snapshot.DataSources {
			if snapshot.DataSources[index].Ref.Equal(ref) {
				return snapshot.DataSources[index].Operations, true, nil
			}
		}
		return nil, false, nil
	case ResourceProject:
		projects, err := service.Work(ctx, snapshot)
		if err != nil {
			return nil, false, err
		}
		for index := range projects {
			if projects[index].Ref.Equal(ref) {
				return projects[index].Operations, true, nil
			}
		}
		return nil, false, nil
	case ResourceWorkItem:
		projects, err := service.Work(ctx, snapshot)
		if err != nil {
			return nil, false, err
		}
		for _, project := range projects {
			for index := range project.Items {
				if project.Items[index].Ref.Equal(ref) {
					return project.Items[index].Operations, true, nil
				}
			}
		}
		return nil, false, nil
	case ResourcePullRequest:
		pullRequests, err := service.PullRequests(ctx, snapshot)
		if err != nil {
			return nil, false, err
		}
		for index := range pullRequests {
			if pullRequests[index].Ref.Equal(ref) {
				return pullRequests[index].Operations, true, nil
			}
		}
		return nil, false, nil
	default:
		return nil, false, fmt.Errorf("cockpit.resource-kind-invalid")
	}
}
