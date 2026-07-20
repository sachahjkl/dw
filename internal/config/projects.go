package config

import "strings"

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
