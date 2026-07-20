package dbcompat

import (
	"context"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/workspace"
)

// DiscoverWorkspaceRepositories adapts workspace manifests and configured repository folders into
// concrete scan roots. Missing project configuration falls back to the manifest repository name.
func DiscoverWorkspaceRepositories(ctx context.Context, root string) []Workspace {
	summaries := workspace.Discover(root)
	result := make([]Workspace, 0, len(summaries))
	configPort := workspace.FileConfigPort{}
	for _, summary := range summaries {
		project, found, err := configPort.Project(ctx, root, summary.Manifest.Project)
		if err != nil {
			found = false
		}
		repositories := make([]Repository, 0, len(summary.Manifest.Repositories))
		for _, name := range summary.Manifest.Repositories {
			folder := name
			if found {
				if configured, ok := project.Repository(name); ok && strings.TrimSpace(configured.Folder) != "" {
					folder = configured.Folder
				}
			}
			repositories = append(repositories, Repository{Name: name, Root: filepath.Join(summary.Path, folder)})
		}
		result = append(result, Workspace{Path: summary.Path, Project: summary.Manifest.Project, Repositories: repositories})
	}
	return result
}

func (service *Service) CollectDiscovered(ctx context.Context, explicitRoot string, save bool) (DatabaseCollectReport, error) {
	root := config.ResolveRoot(explicitRoot)
	return service.Collect(ctx, root, DiscoverWorkspaceRepositories(ctx, root), save)
}
