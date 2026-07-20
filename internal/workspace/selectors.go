package workspace

import (
	"context"
	"github.com/sachahjkl/dw/internal/l10n"
	"path/filepath"
	"sort"
	"strings"
)

func WorkspaceValues(root, project, workItem string) []string {
	ids := ParseWorkItemIDs(workItem)
	items := List(root, project, ids)
	result := make([]string, 0, len(items))
	for _, item := range items {
		result = append(result, item.Path)
	}
	return result
}
func WorkItemValues(root, project string) []string {
	result := make([]string, 0)
	for _, item := range List(root, project, nil) {
		for _, id := range item.AllKnownWorkItemIDs {
			result = appendDistinct(result, id)
		}
	}
	return result
}
func (e *Engine) RepositoryValues(ctx context.Context, root, project, workspace string) ([]string, error) {
	if strings.TrimSpace(workspace) != "" {
		manifest, err := ReadManifest(filepath.Join(workspace, ManifestFile))
		if err != nil {
			return nil, err
		}
		return append([]string(nil), manifest.Repositories...), nil
	}
	if strings.TrimSpace(project) == "" {
		return []string{}, nil
	}
	configured, found, err := e.project(ctx, root, project)
	if err != nil || !found {
		return []string{}, localizedOperation("load project configuration", err)
	}
	result := make([]string, 0, len(configured.Repositories))
	for _, repository := range configured.Repositories {
		result = appendDistinct(result, repository.Name)
	}
	return result, nil
}
func ResolveWorkItemSelection(option, positional string) ([]string, error) {
	left := ParseWorkItemIDs(option)
	right := ParseWorkItemIDs(positional)
	if len(left) > 0 && len(right) > 0 && !sameStrings(left, right) {
		return nil, localized("workspace.error.selection-mismatch")
	}
	if len(left) > 0 {
		return left, nil
	}
	return right, nil
}
func sameStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	left = append([]string(nil), left...)
	right = append([]string(nil), right...)
	sort.Strings(left)
	sort.Strings(right)
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
func ResolveOpenTarget(workspace string, manifest Manifest, project ProjectConfig, repository string) (string, error) {
	repository = strings.TrimSpace(repository)
	if repository == "" {
		return workspace, nil
	}
	if !containsFold(manifest.Repositories, repository) {
		return "", localizedCause("workspace.error.missing-repository", ErrMissingRepository, l10n.A("repository", repository))
	}
	configured, ok := project.Repository(repository)
	folder := repository
	if ok && strings.TrimSpace(configured.Folder) != "" {
		folder = configured.Folder
	}
	return filepath.Join(workspace, folder), nil
}

func BuildStatusReport(root string) StatusReport {
	return StatusReport{Root: root, Items: List(root, "", nil)}
}
func BuildListReport(root string, project *string, workItemIDs []string) ListReport {
	filter := ""
	if project != nil {
		filter = *project
	}
	return ListReport{Root: root, Project: project, WorkItemIDs: append([]string(nil), workItemIDs...), Items: List(root, filter, workItemIDs)}
}
