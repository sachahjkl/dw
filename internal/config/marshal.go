package config

import (
	"encoding/json"
	"strconv"

	"github.com/sachahjkl/dw/internal/wirejson"
)

func workflowWireValue(config WorkflowConfig) (wirejson.Value, error) {
	members := schemaMembers(config.SchemaURL, config.Schema)
	providers, err := providerConfigurationsWire(config.Providers)
	if err != nil {
		return wirejson.Value{}, err
	}
	members = append(members, wirejson.Member{Name: "providers", Value: providers})
	members, err = appendJSONMember(members, "updates", config.Updates)
	if err != nil {
		return wirejson.Value{}, err
	}
	members = append(members, wirejson.Member{Name: "branchPrefixes", Value: namedStringsWire(config.BranchPrefixes)})
	if len(config.WorktreeFolders) != 0 {
		members = append(members, wirejson.Member{Name: "worktreeFolders", Value: namedStringsWire(config.WorktreeFolders)})
	}
	members, err = appendJSONMember(members, "agent", config.Agent)
	if err != nil {
		return wirejson.Value{}, err
	}
	members, err = appendJSONMember(members, "taskStart", config.TaskStart)
	if err != nil {
		return wirejson.Value{}, err
	}
	if config.TaskFinish != nil {
		value, finishErr := taskFinishWire(*config.TaskFinish)
		if finishErr != nil {
			return wirejson.Value{}, finishErr
		}
		members = append(members, wirejson.Member{Name: "taskFinish", Value: value})
	}
	return wirejson.ObjectValue(members...), nil
}

func taskFinishWire(options TaskFinishOptions) (wirejson.Value, error) {
	members := make([]wirejson.Member, 0, 5)
	var err error
	members, err = appendJSONMember(members, "runVerification", options.RunVerification)
	if err != nil {
		return wirejson.Value{}, err
	}
	members, err = appendJSONMember(members, "updateWorkItemState", options.UpdateWorkItemState)
	if err != nil {
		return wirejson.Value{}, err
	}
	members, err = appendJSONMember(members, "bugState", options.BugState)
	if err != nil {
		return wirejson.Value{}, err
	}
	members, err = appendJSONMember(members, "taskState", options.TaskState)
	if err != nil {
		return wirejson.Value{}, err
	}
	commands := make([]wirejson.Member, 0, len(options.VerificationCommands))
	for _, entry := range options.VerificationCommands {
		value, encodeErr := jsonWireValue(entry.Commands)
		if encodeErr != nil {
			return wirejson.Value{}, encodeErr
		}
		commands = append(commands, wirejson.Member{Name: entry.Repository, Value: value})
	}
	members = append(members, wirejson.Member{Name: "verificationCommands", Value: wirejson.ObjectValue(commands...)})
	return wirejson.ObjectValue(members...), nil
}

func projectsWireValue(config ProjectsConfig) (wirejson.Value, error) {
	members := schemaMembers(config.SchemaURL, config.Schema)
	projects := make([]wirejson.Member, 0, len(config.Projects))
	for _, entry := range config.Projects {
		value, err := projectWireValue(entry.Project)
		if err != nil {
			return wirejson.Value{}, err
		}
		projects = append(projects, wirejson.Member{Name: entry.Key, Value: value})
	}
	members = append(members, wirejson.Member{Name: "projects", Value: wirejson.ObjectValue(projects...)})
	return wirejson.ObjectValue(members...), nil
}

func projectWireValue(project ProjectConfig) (wirejson.Value, error) {
	providers, err := providerConfigurationsWire(project.Providers)
	if err != nil {
		return wirejson.Value{}, err
	}
	members := []wirejson.Member{
		{Name: "displayName", Value: wirejson.StringValue(project.DisplayName)},
		{Name: "workProvider", Value: wirejson.StringValue(project.WorkProvider)},
		{Name: "providers", Value: providers},
	}
	repositories := make([]wirejson.Member, 0, len(project.Repositories))
	for _, entry := range project.Repositories {
		value, encodeErr := repositoryWireValue(entry.Repository)
		if encodeErr != nil {
			return wirejson.Value{}, encodeErr
		}
		repositories = append(repositories, wirejson.Member{Name: entry.Key, Value: value})
	}
	members = append(members, wirejson.Member{Name: "repositories", Value: wirejson.ObjectValue(repositories...)})
	if project.IncludedProjects != nil {
		members, err = appendJSONMember(members, "includedProjects", project.IncludedProjects)
		if err != nil {
			return wirejson.Value{}, err
		}
	}
	members, err = appendJSONMember(members, "agent", project.Agent)
	if err != nil {
		return wirejson.Value{}, err
	}
	members, err = appendUnknown(members, project.Unknown)
	if err != nil {
		return wirejson.Value{}, err
	}
	return wirejson.ObjectValue(members...), nil
}

func repositoryWireValue(repository RepositoryConfig) (wirejson.Value, error) {
	urlMembers := []wirejson.Member{{Name: "http", Value: wirejson.StringValue(repository.URL.HTTP)}}
	if repository.URL.SSH != nil {
		urlMembers = append(urlMembers, wirejson.Member{Name: "ssh", Value: wirejson.StringValue(*repository.URL.SSH)})
	}
	members := []wirejson.Member{
		{Name: "url", Value: wirejson.ObjectValue(urlMembers...)},
		{Name: "defaultBranch", Value: wirejson.StringValue(repository.DefaultBranch)},
	}
	var err error
	for _, optional := range []struct {
		name  string
		value *string
	}{
		{"pullRequestTargetBranch", repository.PullRequestTargetBranch},
		{"providerRepository", repository.ProviderRepository},
		{"anchorName", repository.AnchorName}, {"gitCredentialSecret", repository.GitCredentialSecret},
		{"folder", repository.Folder},
	} {
		members, err = appendJSONMember(members, optional.name, optional.value)
		if err != nil {
			return wirejson.Value{}, err
		}
	}
	members, err = appendUnknown(members, repository.Unknown)
	if err != nil {
		return wirejson.Value{}, err
	}
	return wirejson.ObjectValue(members...), nil
}

func providerConfigurationsWire(providers []ProviderConfiguration) (wirejson.Value, error) {
	members := make([]wirejson.Member, 0, len(providers))
	for _, provider := range providers {
		value, err := wirejson.Parse(provider.Raw)
		if err != nil {
			return wirejson.Value{}, err
		}
		members = append(members, wirejson.Member{Name: provider.Name, Value: value})
	}
	return wirejson.ObjectValue(members...), nil
}

func databasesWireValue(config DatabasesConfig) (wirejson.Value, error) {
	members := schemaMembers(config.SchemaURL, config.Schema)
	var err error
	members, err = appendJSONMember(members, "defaults", config.Defaults)
	if err != nil {
		return wirejson.Value{}, err
	}
	globals, err := databaseEntriesWire(config.Globals)
	if err != nil {
		return wirejson.Value{}, err
	}
	members = append(members, wirejson.Member{Name: "globals", Value: globals})
	projects := make([]wirejson.Member, 0, len(config.Projects))
	for _, project := range config.Projects {
		databases, encodeErr := databaseEntriesWire(project.Databases)
		if encodeErr != nil {
			return wirejson.Value{}, encodeErr
		}
		projects = append(projects, wirejson.Member{Name: project.Project, Value: wirejson.ObjectValue(wirejson.Member{Name: "databases", Value: databases})})
	}
	members = append(members, wirejson.Member{Name: "projects", Value: wirejson.ObjectValue(projects...)})
	return wirejson.ObjectValue(members...), nil
}

func databaseEntriesWire(entries []DatabaseEntry) (wirejson.Value, error) {
	members := make([]wirejson.Member, 0, len(entries))
	for _, entry := range entries {
		value, err := databaseWireValue(entry.Database)
		if err != nil {
			return wirejson.Value{}, err
		}
		members = append(members, wirejson.Member{Name: entry.Key, Value: value})
	}
	return wirejson.ObjectValue(members...), nil
}

func databaseWireValue(database DatabaseConfig) (wirejson.Value, error) {
	members := []wirejson.Member{{Name: "provider", Value: wirejson.StringValue(database.Provider)}}
	var err error
	for _, optional := range []struct {
		name  string
		value any
	}{
		{"connectionString", database.ConnectionString},
		{"connectionStringEnvironmentVariable", database.ConnectionStringEnvironmentVariable},
		{"credentialKey", database.CredentialKey}, {"readonly", database.ReadOnly},
		{"maxRows", database.MaxRows}, {"timeoutSeconds", database.TimeoutSeconds},
	} {
		members, err = appendJSONMember(members, optional.name, optional.value)
		if err != nil {
			return wirejson.Value{}, err
		}
	}
	members, err = appendUnknown(members, database.Unknown)
	if err != nil {
		return wirejson.Value{}, err
	}
	return wirejson.ObjectValue(members...), nil
}

func schemaMembers(schemaURL string, schema int) []wirejson.Member {
	members := make([]wirejson.Member, 0, 2)
	if schemaURL != "" {
		members = append(members, wirejson.Member{Name: "$schema", Value: wirejson.StringValue(schemaURL)})
	}
	if schema != 0 {
		members = append(members, wirejson.Member{Name: "schema", Value: wirejson.NumberValue(strconv.Itoa(schema))})
	}
	return members
}

func namedStringsWire(entries []NamedString) wirejson.Value {
	members := make([]wirejson.Member, len(entries))
	for index, entry := range entries {
		members[index] = wirejson.Member{Name: entry.Name, Value: wirejson.StringValue(entry.Value)}
	}
	return wirejson.ObjectValue(members...)
}

func appendJSONMember(members []wirejson.Member, name string, value any) ([]wirejson.Member, error) {
	if isNilJSONValue(value) {
		return members, nil
	}
	encoded, err := jsonWireValue(value)
	if err != nil {
		return nil, err
	}
	return append(members, wirejson.Member{Name: name, Value: encoded}), nil
}

func isNilJSONValue(value any) bool {
	switch value := value.(type) {
	case *string:
		return value == nil
	case *bool:
		return value == nil
	case *int:
		return value == nil
	case *AgentOptions:
		return value == nil
	case *UpdateOptions:
		return value == nil
	case *TaskStartOptions:
		return value == nil
	case *DatabaseDefaults:
		return value == nil
	default:
		return value == nil
	}
}

func jsonWireValue(value any) (wirejson.Value, error) {
	data, err := json.Marshal(value)
	if err != nil {
		return wirejson.Value{}, err
	}
	return wirejson.Parse(data)
}

func appendUnknown(members []wirejson.Member, unknown []NamedRawConfiguration) ([]wirejson.Member, error) {
	for _, item := range unknown {
		value, err := wirejson.Parse(item.Raw)
		if err != nil {
			return nil, err
		}
		members = append(members, wirejson.Member{Name: item.Name, Value: value})
	}
	return members, nil
}
