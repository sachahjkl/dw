package workapp

import (
	"context"
	"os"
	"path/filepath"

	"github.com/sachahjkl/dw/internal/workspace"
)

// WorkspacePorts adapts the concrete local lifecycle engine to workapp's
// consumer-owned ports. Provider operations remain outside this adapter.
type WorkspacePorts struct {
	Engine   *workspace.Engine
	OpenFunc func(context.Context, string, string, string, bool) (any, error)
}

func NewWorkspacePorts(engine *workspace.Engine) *WorkspacePorts {
	return &WorkspacePorts{Engine: engine}
}
func (p *WorkspacePorts) Resolve(_ context.Context, root string, explicit *string, project string, ids []string, useLatest bool) (string, error) {
	value := ""
	if explicit != nil {
		value = *explicit
	}
	current, err := os.Getwd()
	if err != nil {
		return "", err
	}
	return workspace.Resolve(root, value, project, ids, useLatest, current)
}
func (p *WorkspacePorts) Manifest(_ context.Context, path string) (workspace.Manifest, error) {
	return workspace.ReadManifest(filepath.Join(path, workspace.ManifestFile))
}
func (p *WorkspacePorts) PlanStart(ctx context.Context, request workspace.StartRequest) (workspace.StartPlan, error) {
	return p.Engine.PlanStart(ctx, request)
}
func (p *WorkspacePorts) ExecuteStart(ctx context.Context, plan workspace.StartPlan, items []workspace.WorkItem, children []workspace.ChildTask, emit func(workspace.ActionEvent)) (workspace.StartExecutionReport, error) {
	return p.Engine.ExecuteStart(ctx, plan, items, children, emit)
}
func (*WorkspacePorts) ApplySnapshots(_ context.Context, path string, items []workspace.WorkItem) (workspace.Manifest, error) {
	return workspace.ApplySnapshots(path, items)
}
func (*WorkspacePorts) AddChild(_ context.Context, path string, child workspace.ChildTask) (workspace.Manifest, error) {
	return workspace.AddChild(path, child)
}
func (p *WorkspacePorts) Open(ctx context.Context, path, repository, agent string, useLatest bool) (any, error) {
	if p.OpenFunc == nil {
		return nil, capabilityUnavailable("workspace open launch")
	}
	return p.OpenFunc(ctx, path, repository, agent, useLatest)
}
func (*WorkspacePorts) Find(_ context.Context, root string, project *string, ids []string) ([]workspace.Summary, error) {
	key := ""
	if project != nil {
		key = *project
	}
	return workspace.Filter(workspace.Discover(root), key, ids), nil
}
func (*WorkspacePorts) PlanPrune(_ context.Context, root string, project *string, ids []string) ([]workspace.Summary, error) {
	key := ""
	if project != nil {
		key = *project
	}
	return workspace.PruneCandidates(root, key, ids), nil
}
func (p *WorkspacePorts) ExecutePrune(ctx context.Context, root string, candidates []workspace.Summary) (workspace.PruneExecutionReport, error) {
	paths := make([]string, len(candidates))
	for index, candidate := range candidates {
		paths[index] = candidate.Path
	}
	plan := workspace.PrunePlanReport{Root: root, Candidates: candidates}
	return p.Engine.ExecutePrune(ctx, plan, paths, true)
}
func (p *WorkspacePorts) PlanFinish(ctx context.Context, root, path, message string, createPR, ready bool) (workspace.FinishPlanReport, error) {
	return p.Engine.PlanFinish(ctx, root, path, message, createPR, ready)
}
func (p *WorkspacePorts) ExecuteLocalFinish(ctx context.Context, plan workspace.FinishPlanReport, options workspace.FinishExecuteOptions, emit func(workspace.ActionEvent)) (workspace.FinishExecutionReport, error) {
	return p.Engine.ExecuteLocalFinish(ctx, plan, options, emit)
}
