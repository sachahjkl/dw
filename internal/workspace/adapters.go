package workspace

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"time"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/gitrepo"
)

// FileConfigPort projects the shared ordered configuration model into the
// small provider-neutral view required by workspace planning.
type FileConfigPort struct{}

func (FileConfigPort) Project(_ context.Context, root, key string) (ProjectConfig, bool, error) {
	projects, err := config.LoadProjectsConfigChecked(root)
	if err != nil {
		return ProjectConfig{}, false, err
	}
	project, found := config.ResolveProject(projects, key)
	if !found {
		return ProjectConfig{}, false, nil
	}
	result := ProjectConfig{Key: key, WorkProvider: project.WorkProvider, Repositories: make([]RepositoryConfig, 0, len(project.Repositories))}
	for _, entry := range project.Repositories {
		repository := entry.Repository
		target := ""
		if repository.PullRequestTargetBranch != nil {
			target = *repository.PullRequestTargetBranch
		}
		providerRepository := ""
		if repository.ProviderRepository != nil {
			providerRepository = *repository.ProviderRepository
		}
		anchor := ""
		if repository.AnchorName != nil {
			anchor = *repository.AnchorName
		}
		secret := ""
		if repository.GitCredentialSecret != nil {
			secret = *repository.GitCredentialSecret
		}
		folder := ""
		if repository.Folder != nil {
			folder = *repository.Folder
		}
		result.Repositories = append(result.Repositories, RepositoryConfig{Name: entry.Key, HTTPURL: repository.URL.HTTP, SSHURL: repository.URL.SSH, DefaultBranch: repository.DefaultBranch, PullRequestTargetBranch: target, ProviderRepository: providerRepository, AnchorName: anchor, GitCredentialSecret: secret, Folder: folder})
	}
	return result, true, nil
}
func (FileConfigPort) Workflow(_ context.Context, root string) (WorkflowConfig, error) {
	source, err := config.LoadWorkflowConfigChecked(root)
	if err != nil {
		return WorkflowConfig{}, err
	}
	target := defaultWorkflow()
	if source.TaskStart != nil {
		if source.TaskStart.UpdateWorkItemState != nil {
			target.TaskStart.UpdateWorkItemState = *source.TaskStart.UpdateWorkItemState
		}
		if source.TaskStart.CreateChildTasks != nil {
			target.TaskStart.CreateChildTasks = *source.TaskStart.CreateChildTasks
		}
		target.TaskStart.States = []WorkItemTypeState{{"user story", stringValue(source.TaskStart.UserStoryState, "En réalisation")}, {"anomalie", stringValue(source.TaskStart.AnomalyState, "En réalisation")}, {"bug", stringValue(source.TaskStart.BugState, "En développement")}, {"activite", stringValue(source.TaskStart.BugState, "En développement")}, {"task", stringValue(source.TaskStart.TaskState, "En développement")}, {"tache", stringValue(source.TaskStart.TaskState, "En développement")}}
	}
	if source.TaskFinish != nil {
		if source.TaskFinish.RunVerification != nil {
			target.TaskFinish.RunVerification = *source.TaskFinish.RunVerification
		}
		if source.TaskFinish.UpdateWorkItemState != nil {
			target.TaskFinish.UpdateWorkItemState = *source.TaskFinish.UpdateWorkItemState
		}
		target.TaskFinish.States = []WorkItemTypeState{{"bug", stringValue(source.TaskFinish.BugState, "PR en attente")}, {"activite", stringValue(source.TaskFinish.BugState, "PR en attente")}, {"task", stringValue(source.TaskFinish.TaskState, "PR en attente")}, {"tache", stringValue(source.TaskFinish.TaskState, "PR en attente")}}
		target.TaskFinish.VerificationCommands = make([]RepositoryCommands, 0, len(source.TaskFinish.VerificationCommands))
		for _, item := range source.TaskFinish.VerificationCommands {
			target.TaskFinish.VerificationCommands = append(target.TaskFinish.VerificationCommands, RepositoryCommands{Repository: item.Repository, Commands: append([]string(nil), item.Commands...)})
		}
	}
	return target, nil
}
func stringValue(value *string, fallback string) string {
	if value == nil || *value == "" {
		return fallback
	}
	return *value
}

// NativeGitPort is the production typed native-Git implementation.
type NativeGitPort struct{ Client gitrepo.Client }

func NewNativeGitPort() NativeGitPort { return NativeGitPort{Client: gitrepo.NewClient()} }
func (p NativeGitPort) PrepareWorktree(ctx context.Context, request WorktreeRequest) (WorktreeResult, error) {
	var ssh *gitrepo.RemoteURL
	if request.SSHURL != nil {
		value := gitrepo.RemoteURL(*request.SSHURL)
		ssh = &value
	}
	result, err := p.Client.PrepareWorktree(ctx, gitrepo.WorktreePrepareRequest{ProjectRoot: gitrepo.ProjectRootPath(request.ProjectRoot), Repository: gitrepo.WorkspaceRepositoryName(request.Repository), HTTPURL: gitrepo.RemoteURL(request.HTTPURL), SSHURL: ssh, DefaultBranch: gitrepo.BranchName(request.DefaultBranch), AnchorName: gitrepo.AnchorName(request.AnchorName), BranchName: gitrepo.BranchName(request.BranchName), WorktreePath: gitrepo.RepositoryPath(request.WorktreePath), Credential: request.Credential})
	if err != nil {
		return WorktreeResult{}, err
	}
	created := result.Status == gitrepo.WorktreePrepared && result.Detail.Kind != gitrepo.WorktreeAlreadyPresent
	return WorktreeResult{Repository: string(result.Repository), Status: result.Status, Detail: result.Detail, WorktreePath: request.WorktreePath, GitDir: request.ProjectRoot + "/repositories/" + request.AnchorName, Created: created}, nil
}
func (p NativeGitPort) Status(ctx context.Context, path string) (RepositoryStatus, error) {
	return p.Client.RepositoryStatus(ctx, gitrepo.RepositoryPath(path)), nil
}
func (p NativeGitPort) Update(ctx context.Context, path, branch string, credential *gitrepo.Credential, ssh *string) error {
	var nativeSSH *gitrepo.RemoteURL
	if ssh != nil {
		value := gitrepo.RemoteURL(*ssh)
		nativeSSH = &value
	}
	return p.Client.UpdateRepository(ctx, gitrepo.RepositoryPath(path), gitrepo.BranchName(branch), credential, nativeSSH)
}
func (p NativeGitPort) Commit(ctx context.Context, path, message string) error {
	return p.Client.CommitRepository(ctx, gitrepo.RepositoryPath(path), gitrepo.CommitMessage(message))
}
func (p NativeGitPort) Push(ctx context.Context, path, branch string, force bool) error {
	return p.Client.PushRepository(ctx, gitrepo.RepositoryPath(path), gitrepo.BranchName(branch), force)
}
func (p NativeGitPort) HasCommitsAhead(ctx context.Context, path, base string) (bool, error) {
	return p.Client.HasCommitsAheadOf(ctx, gitrepo.RepositoryPath(path), gitrepo.Revision(base))
}
func (p NativeGitPort) WorktreeRemove(ctx context.Context, gitDirectory, worktreePath string) error {
	return p.Client.WorktreeRemove(ctx, gitrepo.RepositoryPath(gitDirectory), gitrepo.RepositoryPath(worktreePath))
}
func (p NativeGitPort) WorktreePrune(ctx context.Context, gitDirectory string) error {
	return p.Client.WorktreePrune(ctx, gitrepo.RepositoryPath(gitDirectory))
}
func (NativeGitPort) RenameBranch(ctx context.Context, path, oldName, newName string) error {
	command := exec.CommandContext(ctx, "git", "-C", path, "branch", "-m", oldName, newName)
	if output, err := command.CombinedOutput(); err != nil {
		return fmt.Errorf("rename branch %s to %s: %w: %s", oldName, newName, err, strings.TrimSpace(string(output)))
	}
	return nil
}
func (NativeGitPort) BranchExists(ctx context.Context, path, name string) (bool, error) {
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false, nil
	}
	command := exec.CommandContext(ctx, "git", "-C", path, "show-ref", "--verify", "--quiet", "refs/heads/"+name)
	err := command.Run()
	if err == nil {
		return true, nil
	}
	if exit, ok := err.(*exec.ExitError); ok && exit.ExitCode() == 1 {
		return false, nil
	}
	return false, err
}
func (NativeGitPort) CurrentBranch(ctx context.Context, path string) (string, error) {
	command := exec.CommandContext(ctx, "git", "-C", path, "branch", "--show-current")
	output, err := command.Output()
	return strings.TrimSpace(string(output)), err
}
func (NativeGitPort) LastCommitTime(ctx context.Context, path string) (time.Time, bool, error) {
	command := exec.CommandContext(ctx, "git", "-C", path, "log", "-1", "--format=%cI")
	output, err := command.Output()
	if err != nil {
		return time.Time{}, false, nil
	}
	value, err := time.Parse(time.RFC3339, strings.TrimSpace(string(output)))
	return value, err == nil, err
}
