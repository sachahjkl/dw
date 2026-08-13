package controller

import (
	"github.com/sachahjkl/dw/internal/execution"
	"github.com/sachahjkl/dw/internal/workspaceapp"
)

func ExecutionDescriptors() []execution.Descriptor {
	return []execution.Descriptor{
		execution.NewJSONDescriptor[AgentContextRequest, AgentContextResult](actionAgentContext, noAgentContextLock),
		execution.NewJSONDescriptor[workspaceapp.StatusRequest, workspaceapp.StatusResult](workspaceapp.ActionStatus, noWorkspaceStatusLock),
		execution.NewJSONDescriptor[workspaceapp.ListRequest, workspaceapp.ListResult](workspaceapp.ActionList, noWorkspaceListLock),
		execution.NewJSONDescriptor[workspaceapp.CurrentRequest, workspaceapp.CurrentResult](workspaceapp.ActionCurrent, noWorkspaceCurrentLock),
		execution.NewJSONDescriptor[workspaceapp.ItemAddRequest, workspaceapp.ItemUpdateResult](workspaceapp.ActionItemAdd, itemAddLock),
		execution.NewJSONDescriptor[workspaceapp.ItemRemoveRequest, workspaceapp.ItemUpdateResult](workspaceapp.ActionItemRemove, itemRemoveLock),
		execution.NewJSONDescriptor[workspaceapp.PreflightRequest, workspaceapp.PreflightResult](workspaceapp.ActionPreflight, noWorkspacePreflightLock),
		execution.NewJSONDescriptor[workspaceapp.RenameRequest, workspaceapp.RenameResult](workspaceapp.ActionRename, renameLock),
		execution.NewJSONDescriptor[workspaceapp.RepoAddRequest, workspaceapp.RepoAddResult](workspaceapp.ActionRepoAdd, repoAddLock),
		execution.NewJSONDescriptor[workspaceapp.RepoLatestRequest, workspaceapp.RepoLatestResult](workspaceapp.ActionRepoLatest, repoLatestLock),
		execution.NewJSONDescriptor[workspaceapp.CommitRequest, workspaceapp.CommitResult](workspaceapp.ActionCommit, commitLock),
		execution.NewJSONDescriptor[workspaceapp.HandoffRequest, workspaceapp.HandoffResult](workspaceapp.ActionHandoff, noWorkspaceHandoffLock),
		execution.NewJSONDescriptor[workspaceapp.TeardownRequest, workspaceapp.TeardownResult](workspaceapp.ActionTeardown, teardownLock),
	}
}

func noAgentContextLock(AgentContextRequest) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockNone}, nil
}
func noWorkspaceStatusLock(workspaceapp.StatusRequest) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockNone}, nil
}
func noWorkspaceListLock(workspaceapp.ListRequest) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockNone}, nil
}
func noWorkspaceCurrentLock(workspaceapp.CurrentRequest) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockNone}, nil
}
func noWorkspacePreflightLock(request workspaceapp.PreflightRequest) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockExclusive, Key: request.Selection.Root}, nil
}
func noWorkspaceHandoffLock(workspaceapp.HandoffRequest) (execution.LockSpec, error) {
	return execution.LockSpec{Mode: execution.LockNone}, nil
}
func itemAddLock(request workspaceapp.ItemAddRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func itemRemoveLock(request workspaceapp.ItemRemoveRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func renameLock(request workspaceapp.RenameRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func repoAddLock(request workspaceapp.RepoAddRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func repoLatestLock(request workspaceapp.RepoLatestRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func commitLock(request workspaceapp.CommitRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func teardownLock(request workspaceapp.TeardownRequest) (execution.LockSpec, error) {
	return conditionalRootLock(request.Selection.Root, request.Execute), nil
}
func conditionalRootLock(root string, execute bool) execution.LockSpec {
	if !execute {
		return execution.LockSpec{Mode: execution.LockNone}
	}
	return execution.LockSpec{Mode: execution.LockExclusive, Key: root}
}
