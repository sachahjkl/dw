package dataapp

import (
	"context"
	"path/filepath"
	"strings"

	"github.com/sachahjkl/dw/internal/config"
	"github.com/sachahjkl/dw/internal/contract"
	"github.com/sachahjkl/dw/internal/data"
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

func (service *Service) CollectDiscovered(ctx context.Context, explicitRoot, selected string, save bool) (DataSourceCollectReport, error) {
	root := config.ResolveRoot(explicitRoot)
	provider, err := service.selectedProvider(selected)
	if err != nil {
		return DataSourceCollectReport{}, err
	}
	discoverer, err := data.Require[data.Discoverer](provider, data.CapabilityDiscoverer)
	if err != nil {
		return DataSourceCollectReport{}, err
	}
	workspaces := DiscoverWorkspaceRepositories(ctx, root)
	request := data.DiscoveryRequest{Root: root, Workspaces: make([]data.DiscoveryWorkspace, len(workspaces))}
	for index, workspace := range workspaces {
		project := contract.None[contract.ProjectKey]()
		if value := strings.TrimSpace(workspace.Project); value != "" {
			project = contract.Some(contract.ProjectKey(value))
		}
		repositories := make([]data.DiscoveryRepository, len(workspace.Repositories))
		for repositoryIndex, repository := range workspace.Repositories {
			repositories[repositoryIndex] = data.DiscoveryRepository{Name: repository.Name, Root: repository.Root}
		}
		request.Workspaces[index] = data.DiscoveryWorkspace{Path: workspace.Path, Project: project, Repositories: repositories}
	}
	discovered, err := discoverer.Discover(ctx, request)
	if err != nil {
		return DataSourceCollectReport{}, err
	}
	report, candidates := collectReport(root, len(workspaces), discovered, save)
	if save {
		if service.secrets == nil {
			return DataSourceCollectReport{}, localized("data.error.secret_store")
		}
		if err := saveCandidates(ctx, root, candidates, service.secrets); err != nil {
			return DataSourceCollectReport{}, err
		}
	}
	return finishCollectReport(report, candidates), nil
}
