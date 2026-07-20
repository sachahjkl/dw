package config

import "sort"

func ProjectValues(root string) []string {
	projects := LoadProjectsConfig(root)
	values := make([]string, 0, len(projects.Projects))
	for _, project := range projects.Projects {
		values = append(values, project.Key)
	}
	return values
}

func DatabaseValues(root, project string) []string {
	config := LoadDatabasesConfig(root)
	values := make([]string, 0, len(config.Globals))
	for _, database := range config.Globals {
		values = append(values, database.Key)
	}
	if project != "" {
		for _, configured := range config.Projects {
			if configured.Project != project {
				continue
			}
			for _, database := range configured.Databases {
				values = append(values, database.Key)
			}
			break
		}
	}
	return sortedUnique(values)
}

// EnvironmentValues is intentionally identical to DatabaseValues for the
// compatibility --env alias.
func EnvironmentValues(root, project string) []string { return DatabaseValues(root, project) }

// EnvValues retains the Rust completion source name.
func EnvValues(root, project string) []string { return DatabaseValues(root, project) }

func SecretKeyValues(root string) []string {
	config := LoadDatabasesConfig(root)
	values := make([]string, 0)
	for _, database := range config.Globals {
		if database.Database.CredentialKey != nil {
			values = append(values, *database.Database.CredentialKey)
		}
	}
	for _, project := range config.Projects {
		for _, database := range project.Databases {
			if database.Database.CredentialKey != nil {
				values = append(values, *database.Database.CredentialKey)
			}
		}
	}
	return sortedUnique(values)
}

func sortedUnique(values []string) []string {
	sort.Strings(values)
	if len(values) < 2 {
		return values
	}
	output := values[:1]
	for _, value := range values[1:] {
		if value != output[len(output)-1] {
			output = append(output, value)
		}
	}
	return output
}
