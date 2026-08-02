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
	ID             action.ID
	Label          string
	Description    string
	Request        action.Request
	Risk           Risk
	Active         bool
	DisabledReason string
}

type Snapshot struct {
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
	Path         string
	Project      string
	WorkItems    []string
	Type         string
	Slug         string
	Branch       string
	Repositories []string
	Operations   []Operation
}

type WorkProject struct {
	Key      string
	Label    string
	Provider string
	Error    string
	Items    []WorkItem
}

type WorkItem struct {
	ID         string
	Type       string
	State      string
	Title      string
	URL        string
	Operations []Operation
}

type PullRequest struct {
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
	Project    string
	Key        string
	Provider   string
	Operations []Operation
}

type CockpitItem struct {
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
