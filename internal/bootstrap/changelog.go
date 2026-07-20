package bootstrap

import (
	"context"
	"os"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/gitrepo"
	"github.com/sachahjkl/dw/internal/work/ado"
	"github.com/sachahjkl/dw/internal/workapp"
)

type gitChangelogResolver struct{}

func (gitChangelogResolver) ResolveGitRange(ctx context.Context, request workapp.ChangelogRequest) ([]workapp.GitChangelogSection, error) {
	targets := changelogTargets(request)
	from := request.GitFrom
	if strings.TrimSpace(from) == "" {
		from = "origin/main"
	}
	to := request.GitTo
	if strings.TrimSpace(to) == "" {
		to = "HEAD"
	}
	client := gitrepo.NewClient()
	result := make([]workapp.GitChangelogSection, 0, len(targets))
	for _, target := range targets {
		section := workapp.GitChangelogSection{Repository: target.name, Path: target.path}
		messages, err := client.CommitMessagesInRangeAt(ctx, gitrepo.RepositoryPath(target.path), gitrepo.RevisionRange{
			From: gitrepo.Revision(from),
			To:   gitrepo.Revision(to),
		})
		if err != nil {
			section.SourceEmpty = true
			section.Warnings = []workapp.ChangelogWarning{{Detail: err.Error()}}
		} else {
			section.WorkItemIDs = ado.ExtractWorkItemIDsFromCommitMessages(messages.String())
			section.SourceEmpty = len(section.WorkItemIDs) == 0
		}
		result = append(result, section)
	}
	return result, nil
}

type changelogTarget struct {
	name string
	path string
}

func changelogTargets(request workapp.ChangelogRequest) []changelogTarget {
	root := config.ResolveRoot(request.Root)
	projects := config.LoadProjectsConfig(root)
	project, _ := config.ResolveProject(projects, request.Project)
	requested := request.Repositories
	if len(requested) == 0 {
		requested = make([]string, 0, len(project.Repositories))
		for _, repository := range project.Repositories {
			requested = append(requested, repository.Key)
		}
	}
	result := make([]changelogTarget, 0, len(requested))
	for _, value := range requested {
		if info, err := os.Stat(value); err == nil && info.IsDir() {
			path, absoluteErr := filepath.Abs(value)
			if absoluteErr == nil {
				value = path
			}
			result = append(result, changelogTarget{name: filepath.Base(value), path: value})
			continue
		}
		repository, found := config.Repository(project, value)
		if !found {
			result = append(result, changelogTarget{name: value, path: value})
			continue
		}
		anchor := value + ".git"
		if repository.AnchorName != nil && strings.TrimSpace(*repository.AnchorName) != "" {
			anchor = *repository.AnchorName
		}
		result = append(result, changelogTarget{
			name: value,
			path: filepath.Join(root, "projects", request.Project, "repositories", anchor),
		})
	}
	return result
}
