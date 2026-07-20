package config

import (
	"encoding/json"
	"strings"

	"github.com/sachahjkl/dw/internal/wirejson"
)

func (config ProjectsConfig) Project(key string) (ProjectConfig, bool) {
	for _, entry := range config.Projects {
		if entry.Key == key {
			return entry.Project, !entry.invalid
		}
	}
	return ProjectConfig{}, false
}

func ResolveProject(config ProjectsConfig, project string) (ProjectConfig, bool) {
	visited := make([]string, 0)
	return resolveProject(config, project, &visited)
}

// resolveProject intentionally does not pop visited names between includes. This
// matches the established include traversal, including its repeated-include
// cycle behavior.
func resolveProject(config ProjectsConfig, project string, visited *[]string) (ProjectConfig, bool) {
	for _, item := range *visited {
		if equalFoldASCII(item, project) {
			return ProjectConfig{}, false
		}
	}
	*visited = append(*visited, project)
	current, ok := config.Project(project)
	if !ok {
		return ProjectConfig{}, false
	}
	resolved := current
	resolved.Repositories = nil
	for _, included := range current.IncludedProjects {
		base, found := resolveProject(config, included, visited)
		if !found {
			return ProjectConfig{}, false
		}
		for _, repository := range base.Repositories {
			resolved.Repositories = setRepository(resolved.Repositories, repository)
		}
	}
	for _, repository := range current.Repositories {
		resolved.Repositories = setRepository(resolved.Repositories, repository)
	}
	return resolved, true
}

func setRepository(repositories []RepositoryEntry, replacement RepositoryEntry) []RepositoryEntry {
	for index := range repositories {
		if repositories[index].Key == replacement.Key {
			repositories[index] = replacement
			return repositories
		}
	}
	return append(repositories, replacement)
}

func Repository(config ProjectConfig, repository string) (RepositoryConfig, bool) {
	for _, entry := range config.Repositories {
		if entry.Key == repository {
			return entry.Repository, true
		}
	}
	return RepositoryConfig{}, false
}

// ProviderRawOptions returns one provider's opaque configuration without
// decoding it or disturbing its object-member order.
func ProviderRawOptions(providers []ProviderConfiguration, provider string) (json.RawMessage, bool) {
	for _, configured := range providers {
		if configured.Name == provider {
			return configured.Raw, true
		}
	}
	return nil, false
}

func WorkflowProviderRawOptions(workflow WorkflowConfig, provider string) (json.RawMessage, bool) {
	return ProviderRawOptions(workflow.Providers, provider)
}

func ProjectProviderRawOptions(project ProjectConfig, provider string) (json.RawMessage, bool) {
	return ProviderRawOptions(project.Providers, provider)
}

// DecodeProviderOptions lets a provider own its option schema while config
// remains provider-neutral.
func DecodeProviderOptions[T any](raw json.RawMessage) (T, error) {
	var options T
	err := json.Unmarshal(raw, &options)
	return options, err
}

func WorkflowProviderOptions[T any](workflow WorkflowConfig, provider string) (T, bool, error) {
	raw, ok := WorkflowProviderRawOptions(workflow, provider)
	if !ok {
		var zero T
		return zero, false, nil
	}
	options, err := DecodeProviderOptions[T](raw)
	return options, true, err
}

func ProjectProviderOptions[T any](project ProjectConfig, provider string) (T, bool, error) {
	raw, ok := ProjectProviderRawOptions(project, provider)
	if !ok {
		var zero T
		return zero, false, nil
	}
	options, err := DecodeProviderOptions[T](raw)
	return options, true, err
}

// ResolveProviderRawOptions overlays a project's provider object onto the
// workflow object recursively. Existing members retain their positions and
// project-only members are appended in project order.
func ResolveProviderRawOptions(workflow WorkflowConfig, project ProjectConfig, provider string) (json.RawMessage, bool, error) {
	base, hasBase := WorkflowProviderRawOptions(workflow, provider)
	override, hasOverride := ProjectProviderRawOptions(project, provider)
	if !hasOverride {
		return base, hasBase, nil
	}
	if !hasBase {
		return override, true, nil
	}
	baseValue, err := wirejson.Parse(base)
	if err != nil {
		return nil, false, err
	}
	overrideValue, err := wirejson.Parse(override)
	if err != nil {
		return nil, false, err
	}
	merged := mergeProviderValues(baseValue, overrideValue)
	raw, err := wirejson.Compact(merged)
	return raw, true, err
}

func ResolveProviderOptions[T any](workflow WorkflowConfig, project ProjectConfig, provider string) (T, bool, error) {
	raw, ok, err := ResolveProviderRawOptions(workflow, project, provider)
	if err != nil || !ok {
		var zero T
		return zero, ok, err
	}
	options, err := DecodeProviderOptions[T](raw)
	return options, true, err
}

func mergeProviderValues(base, override wirejson.Value) wirejson.Value {
	if base.Kind() != wirejson.Object || override.Kind() != wirejson.Object {
		return override.Clone()
	}
	merged := base.Clone()
	members, _ := override.Members()
	for _, member := range members {
		current, exists := merged.Lookup(member.Name)
		var value wirejson.Value
		if exists {
			value = mergeProviderValues(*current, member.Value)
		} else {
			value = member.Value.Clone()
		}
		_ = merged.Set(member.Name, value)
	}
	return merged
}

// ProjectWorkProvider applies the provider-selection fallback without loading
// configuration: project selection first, then workflow provider order.
func ProjectWorkProvider(workflow WorkflowConfig, project ProjectConfig) string {
	if provider := strings.TrimSpace(project.WorkProvider); provider != "" {
		return provider
	}
	if len(workflow.Providers) != 0 {
		return workflow.Providers[0].Name
	}
	return ""
}

func ResolveWorkProvider(root, project string) string {
	root = ResolveRoot(root)
	workflow := LoadWorkflowConfig(root)
	projects := LoadProjectsConfig(root)
	resolved, _ := ResolveProject(projects, project)
	return ProjectWorkProvider(workflow, resolved)
}

func ProjectChoices(config ProjectsConfig) []ProjectChoice {
	choices := make([]ProjectChoice, 0, len(config.Projects))
	for _, entry := range config.Projects {
		label := entry.Key
		if project, ok := ResolveProject(config, entry.Key); ok {
			displayName := strings.TrimSpace(project.DisplayName)
			if displayName != "" && project.DisplayName != entry.Key {
				label = entry.Key + " - " + project.DisplayName
			}
		}
		choices = append(choices, ProjectChoice{Key: entry.Key, Label: label})
	}
	return choices
}

func equalFoldASCII(left, right string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range len(left) {
		a, b := left[index], right[index]
		if a >= 'A' && a <= 'Z' {
			a += 'a' - 'A'
		}
		if b >= 'A' && b <= 'Z' {
			b += 'a' - 'A'
		}
		if a != b {
			return false
		}
	}
	return true
}
