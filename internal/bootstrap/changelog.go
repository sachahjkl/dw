package bootstrap

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/gitrepo"
	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/work"
	"github.com/sachahjkl/dw/internal/workapp"
	"github.com/sachahjkl/dw/internal/workspace"
)

type gitChangelogResolver struct {
	providers *work.Registry
}

type changelogGit interface {
	FetchAnchor(context.Context, gitrepo.RepositoryPath) error
	CommitMessagesInRangeAt(context.Context, gitrepo.RepositoryPath, gitrepo.RevisionRange) (gitrepo.CommitMessages, error)
}

func (resolver gitChangelogResolver) ResolveGitRange(ctx context.Context, request workapp.ChangelogRequest) ([]workapp.GitChangelogSection, error) {
	root := config.ResolveRoot(request.Root)
	providerName := work.ProviderName(config.ResolveWorkProvider(root, request.Project))
	provider, err := resolver.providers.Get(providerName)
	if err != nil {
		return nil, err
	}
	extractor, err := work.Require[work.CommitReferenceExtractor](provider, work.CapabilityCommitReferenceExtractor)
	if err != nil {
		return nil, err
	}
	currentDirectory, _ := os.Getwd()
	targets, err := changelogTargets(request, currentDirectory)
	if err != nil {
		return nil, err
	}
	return resolveChangelogSections(ctx, gitrepo.NewClient(), extractor, request, targets)
}

func resolveChangelogSections(ctx context.Context, client changelogGit, extractor work.CommitReferenceExtractor, request workapp.ChangelogRequest, targets []changelogTarget) ([]workapp.GitChangelogSection, error) {
	to := request.GitTo
	if strings.TrimSpace(to) == "" {
		to = "HEAD"
	}
	result := make([]workapp.GitChangelogSection, 0, len(targets))
	failures := make([]error, 0)
	for _, target := range targets {
		section := workapp.GitChangelogSection{Repository: target.name, Path: target.path}
		if target.err != nil {
			failures = append(failures, target.err)
			section.SourceEmpty = true
			section.Warnings = []workapp.ChangelogWarning{{Detail: target.err.Error()}}
			result = append(result, section)
			continue
		}
		from := request.GitFrom
		if strings.TrimSpace(from) == "" {
			from = gitrepo.ResolveRemoteSourceBranch(gitrepo.BranchName(target.defaultBranch))
		}
		if !target.worktree {
			if err := client.FetchAnchor(ctx, gitrepo.RepositoryPath(target.path)); err != nil {
				section.Warnings = append(section.Warnings, workapp.ChangelogWarning{Detail: l10n.Render(l10n.M("changelog.warning.fetch-failed", l10n.A("repository", target.name), l10n.A("detail", err.Error())))})
			}
		}
		messages, err := client.CommitMessagesInRangeAt(ctx, gitrepo.RepositoryPath(target.path), gitrepo.RevisionRange{
			From: gitrepo.Revision(from),
			To:   gitrepo.Revision(to),
		})
		if err != nil {
			failures = append(failures, err)
			section.SourceEmpty = true
			section.Warnings = append(section.Warnings, workapp.ChangelogWarning{Detail: err.Error()})
		} else {
			references := extractor.ExtractCommitReferences(messages.String())
			section.WorkItemIDs = make([]string, len(references))
			for index := range references {
				section.WorkItemIDs[index] = string(references[index])
			}
			section.SourceEmpty = len(section.WorkItemIDs) == 0
		}
		result = append(result, section)
	}
	if len(targets) != 0 && len(failures) == len(targets) {
		if len(failures) == 1 {
			return nil, failures[0]
		}
		return nil, errors.Join(failures...)
	}
	return result, nil
}

type changelogTarget struct {
	name          string
	path          string
	defaultBranch string
	worktree      bool
	err           error
}

func changelogTargets(request workapp.ChangelogRequest, currentDirectory string) ([]changelogTarget, error) {
	root := config.ResolveRoot(request.Root)
	projects := config.LoadProjectsConfig(root)
	project, found := config.ResolveProject(projects, request.Project)
	if !found {
		return nil, l10n.NewError("changelog.error.project-unknown", l10n.A("project", request.Project))
	}
	requested := request.Repositories
	if len(requested) == 0 {
		requested = make([]string, 0, len(project.Repositories))
		for _, repository := range project.Repositories {
			requested = append(requested, repository.Key)
		}
	}
	var current *workspace.CurrentItem
	if currentDirectory != "" {
		if item, err := workspace.Current(currentDirectory); err == nil && strings.EqualFold(item.Project, request.Project) {
			current = &item
		}
	}
	result := make([]changelogTarget, 0, len(requested))
	for _, value := range requested {
		repository, found := config.Repository(project, value)
		if !found {
			if info, err := os.Stat(value); err == nil && info.IsDir() {
				path, absoluteErr := filepath.Abs(value)
				if absoluteErr == nil {
					value = path
				}
				result = append(result, changelogTarget{name: filepath.Base(value), path: value, defaultBranch: "main", worktree: true})
				continue
			}
			result = append(result, changelogTarget{name: value, err: l10n.NewError("changelog.error.repository-unknown", l10n.A("repository", value), l10n.A("project", request.Project))})
			continue
		}
		result = append(result, repositoryChangelogTarget(root, projects, request.Project, value, repository, current))
	}
	return result, nil
}

func repositoryChangelogTarget(root string, projects config.ProjectsConfig, project, key string, repository config.RepositoryConfig, current *workspace.CurrentItem) changelogTarget {
	target := changelogTarget{name: key, defaultBranch: strings.TrimSpace(repository.DefaultBranch)}
	if target.defaultBranch == "" {
		target.defaultBranch = "main"
	}
	if current != nil && slices.Contains(current.Repositories, key) {
		folder := key
		if repository.Folder != nil && strings.TrimSpace(*repository.Folder) != "" {
			folder = *repository.Folder
		}
		path := filepath.Join(current.Workspace, folder)
		if info, err := os.Stat(path); err == nil && info.IsDir() {
			target.path, target.worktree = path, true
			return target
		}
	}
	anchor := key + ".git"
	if repository.AnchorName != nil && strings.TrimSpace(*repository.AnchorName) != "" {
		anchor = *repository.AnchorName
	}
	candidates := []string{filepath.Join(root, "projects", project, "repositories", anchor)}
	if origin, found := repositoryOrigin(projects, project, key, 0); found && !strings.EqualFold(origin, project) {
		candidates = append(candidates, filepath.Join(root, "projects", origin, "repositories", anchor))
	}
	for _, candidate := range candidates {
		if info, err := os.Stat(candidate); err == nil && info.IsDir() {
			target.path = candidate
			return target
		}
	}
	target.path = candidates[len(candidates)-1]
	target.err = l10n.NewError("changelog.error.anchor-missing", l10n.A("repository", key), l10n.A("path", target.path))
	return target
}

// repositoryOrigin finds the project that declares a repository, honouring
// the override order of resolveProject: own entries, then later includes.
func repositoryOrigin(projects config.ProjectsConfig, project, repository string, depth int) (string, bool) {
	if depth > len(projects.Projects) {
		return "", false
	}
	current, ok := projects.Project(project)
	if !ok {
		return "", false
	}
	if _, own := config.Repository(current, repository); own {
		return project, true
	}
	for index := len(current.IncludedProjects) - 1; index >= 0; index-- {
		if origin, found := repositoryOrigin(projects, current.IncludedProjects[index], repository, depth+1); found {
			return origin, true
		}
	}
	return "", false
}
