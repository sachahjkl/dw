package config

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strconv"

	"github.com/sachahjkl/dw/internal/l10n"
	"github.com/sachahjkl/dw/internal/wirejson"
)

func LoadWorkflowConfig(root string) WorkflowConfig {
	config, err := LoadWorkflowConfigChecked(root)
	if err != nil {
		return WorkflowConfig{}
	}
	return config
}

func LoadWorkflowConfigChecked(root string) (WorkflowConfig, error) {
	return loadWorkflowPath(filepath.Join(root, "config", "workflow.json"))
}

func LoadProjectsConfig(root string) ProjectsConfig {
	config, err := loadProjectsPathTolerant(filepath.Join(root, "config", "projects.json"))
	if err != nil {
		return ProjectsConfig{}
	}
	return config
}

func LoadProjectsConfigChecked(root string) (ProjectsConfig, error) {
	return loadProjectsPath(filepath.Join(root, "config", "projects.json"))
}

func LoadDatabasesConfig(root string) DatabasesConfig {
	config, err := loadDatabasesPathTolerant(filepath.Join(root, "config", "databases.json"))
	if err != nil {
		return DatabasesConfig{}
	}
	return config
}

func LoadDatabasesConfigChecked(root string) (DatabasesConfig, error) {
	return loadDatabasesPath(filepath.Join(root, "config", "databases.json"))
}

func readOrderedJSON(path string) (wirejson.Value, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return wirejson.Value{}, localizedError(l10n.M("config.read_file", l10n.A("path", path), l10n.A("error", err)))
	}
	value, err := wirejson.Parse(data)
	if err != nil {
		return wirejson.Value{}, localizedError(l10n.M("config.parse_file", l10n.A("path", path), l10n.A("error", err)))
	}
	if value.Kind() != wirejson.Object {
		return wirejson.Value{}, errors.New("config.root-not-object")
	}
	return value, nil
}

func loadWorkflowPath(path string) (WorkflowConfig, error) {
	document, err := readOrderedJSON(path)
	if err != nil {
		return WorkflowConfig{}, err
	}
	config := WorkflowConfig{document: retained(document)}
	config.SchemaURL, _ = objectString(&document, "$schema")
	config.Schema, _ = objectInt(&document, "schema")
	if value, ok := document.Lookup("branchPrefixes"); ok && !value.IsNull() {
		config.BranchPrefixes, err = orderedStrings(value)
		if err != nil {
			return WorkflowConfig{}, err
		}
	}
	if value, ok := document.Lookup("worktreeFolders"); ok && !value.IsNull() {
		config.WorktreeFolders, err = orderedStrings(value)
		if err != nil {
			return WorkflowConfig{}, err
		}
	}
	if value, ok := document.Lookup("azureDevOps"); ok && !value.IsNull() {
		var options AzureDevOpsOptions
		if err = decodeValue(value, &options); err != nil {
			return WorkflowConfig{}, err
		}
		config.AzureDevOps = &options
	}
	if value, ok := document.Lookup("auth"); ok && !value.IsNull() {
		var options AuthOptions
		if err = decodeValue(value, &options); err != nil {
			return WorkflowConfig{}, err
		}
		config.Auth = &options
	}
	if value, ok := document.Lookup("updates"); ok && !value.IsNull() {
		var options UpdateOptions
		if err = decodeValue(value, &options); err != nil {
			return WorkflowConfig{}, err
		}
		config.Updates = &options
	}
	if value, ok := document.Lookup("agent"); ok && !value.IsNull() {
		var options AgentOptions
		if err = decodeValue(value, &options); err != nil {
			return WorkflowConfig{}, err
		}
		config.Agent = &options
	}
	if value, ok := document.Lookup("taskStart"); ok && !value.IsNull() {
		var options TaskStartOptions
		if err = decodeValue(value, &options); err != nil {
			return WorkflowConfig{}, err
		}
		config.TaskStart = &options
	}
	if value, ok := document.Lookup("taskFinish"); ok && !value.IsNull() {
		config.TaskFinish, err = parseTaskFinish(value)
		if err != nil {
			return WorkflowConfig{}, err
		}
	}
	return config, nil
}

func parseTaskFinish(value *wirejson.Value) (*TaskFinishOptions, error) {
	var known struct {
		RunVerification     *bool   `json:"runVerification,omitempty"`
		UpdateWorkItemState *bool   `json:"updateWorkItemState,omitempty"`
		BugState            *string `json:"bugState,omitempty"`
		TaskState           *string `json:"taskState,omitempty"`
	}
	if err := decodeValue(value, &known); err != nil {
		return nil, err
	}
	options := TaskFinishOptions{
		RunVerification: known.RunVerification, UpdateWorkItemState: known.UpdateWorkItemState,
		BugState: known.BugState, TaskState: known.TaskState,
	}
	if commands, ok := value.Lookup("verificationCommands"); ok && !commands.IsNull() {
		members, ok := effectiveMembers(commands)
		if !ok {
			return nil, errors.New("config.verification-commands-not-object")
		}
		for _, member := range members {
			var values []string
			if err := decodeValue(&member.Value, &values); err != nil {
				return nil, err
			}
			options.VerificationCommands = append(options.VerificationCommands, RepositoryCommands{Repository: member.Name, Commands: values})
		}
	}
	return &options, nil
}

func loadProjectsPath(path string) (ProjectsConfig, error) {
	document, err := readOrderedJSON(path)
	if err != nil {
		return ProjectsConfig{}, err
	}
	config := ProjectsConfig{document: retained(document)}
	config.SchemaURL, _ = objectString(&document, "$schema")
	config.Schema, _ = objectInt(&document, "schema")
	projects, ok := document.Lookup("projects")
	if !ok || projects.IsNull() {
		return config, nil
	}
	members, ok := effectiveMembers(projects)
	if !ok {
		return ProjectsConfig{}, errors.New("config.projects-not-object")
	}
	for _, member := range members {
		project, parseErr := parseProject(&member.Value)
		if parseErr != nil {
			return ProjectsConfig{}, parseErr
		}
		config.Projects = append(config.Projects, ProjectEntry{Key: member.Name, Project: project})
	}
	return config, nil
}

func loadProjectsPathTolerant(path string) (ProjectsConfig, error) {
	document, err := readOrderedJSON(path)
	if err != nil {
		return ProjectsConfig{}, err
	}
	config := ProjectsConfig{document: retained(document)}
	config.SchemaURL, _ = objectString(&document, "$schema")
	config.Schema, _ = objectInt(&document, "schema")
	projects, ok := document.Lookup("projects")
	if !ok || projects.IsNull() {
		return config, nil
	}
	members, ok := effectiveMembers(projects)
	if !ok {
		return ProjectsConfig{}, errors.New("config.projects-not-object")
	}
	for _, member := range members {
		project, parseErr := parseProject(&member.Value)
		config.Projects = append(config.Projects, ProjectEntry{Key: member.Name, Project: project, invalid: parseErr != nil})
	}
	return config, nil
}

func parseProject(value *wirejson.Value) (ProjectConfig, error) {
	if value.Kind() != wirejson.Object {
		return ProjectConfig{}, errors.New("config.project-not-object")
	}
	project := ProjectConfig{}
	project.DisplayName, _ = objectString(value, "displayName")
	if included, ok := value.Lookup("includedProjects"); ok && !included.IsNull() {
		if err := decodeValue(included, &project.IncludedProjects); err != nil {
			return ProjectConfig{}, err
		}
	}
	if options, ok := value.Lookup("agent"); ok && !options.IsNull() {
		var agent AgentOptions
		if err := decodeValue(options, &agent); err != nil {
			return ProjectConfig{}, err
		}
		project.Agent = &agent
	}
	if options, ok := value.Lookup("azureDevOps"); ok && !options.IsNull() {
		var ado AzureDevOpsOptions
		if err := decodeValue(options, &ado); err != nil {
			return ProjectConfig{}, err
		}
		project.AzureDevOps = &ado
	}
	if repositories, ok := value.Lookup("repositories"); ok && !repositories.IsNull() {
		members, object := effectiveMembers(repositories)
		if !object {
			return ProjectConfig{}, errors.New("config.repositories-not-object")
		}
		for _, member := range members {
			repository, err := parseRepository(&member.Value)
			if err != nil {
				return ProjectConfig{}, err
			}
			project.Repositories = append(project.Repositories, RepositoryEntry{Key: member.Name, Repository: repository})
		}
	}
	project.Unknown = unknownMembers(value, "displayName", "repositories", "includedProjects", "agent", "azureDevOps")
	return project, nil
}

func parseRepository(value *wirejson.Value) (RepositoryConfig, error) {
	if value.Kind() != wirejson.Object {
		return RepositoryConfig{}, errors.New("config.repository-not-object")
	}
	var repository RepositoryConfig
	if url, ok := value.Lookup("url"); ok {
		switch url.Kind() {
		case wirejson.String:
			repository.URL.HTTP, _ = url.AsString()
		case wirejson.Object:
			repository.URL.HTTP, _ = objectString(url, "http")
			if ssh, exists := objectString(url, "ssh"); exists {
				repository.URL.SSH = &ssh
			}
		default:
			return RepositoryConfig{}, errors.New("config.repository-url-invalid")
		}
	}
	repository.DefaultBranch, _ = objectString(value, "defaultBranch")
	repository.PullRequestTargetBranch = optionalString(value, "pullRequestTargetBranch")
	repository.AzureDevOpsRepository = optionalString(value, "azureDevOpsRepository")
	repository.AnchorName = optionalString(value, "anchorName")
	repository.GitCredentialSecret = optionalString(value, "gitCredentialSecret")
	repository.Folder = optionalString(value, "folder")
	repository.Unknown = unknownMembers(value, "url", "defaultBranch", "pullRequestTargetBranch", "azureDevOpsRepository", "anchorName", "gitCredentialSecret", "folder")
	return repository, nil
}

func loadDatabasesPath(path string) (DatabasesConfig, error) {
	document, err := readOrderedJSON(path)
	if err != nil {
		return DatabasesConfig{}, err
	}
	config := DatabasesConfig{document: retained(document)}
	config.SchemaURL, _ = objectString(&document, "$schema")
	config.Schema, _ = objectInt(&document, "schema")
	if defaults, ok := document.Lookup("defaults"); ok && !defaults.IsNull() {
		var options DatabaseDefaults
		if err = decodeValue(defaults, &options); err != nil {
			return DatabasesConfig{}, err
		}
		config.Defaults = &options
	}
	if globals, ok := document.Lookup("globals"); ok && !globals.IsNull() {
		config.Globals, err = parseDatabases(globals)
		if err != nil {
			return DatabasesConfig{}, err
		}
	}
	if projects, ok := document.Lookup("projects"); ok && !projects.IsNull() {
		members, object := effectiveMembers(projects)
		if !object {
			return DatabasesConfig{}, errors.New("config.database-projects-not-object")
		}
		for _, member := range members {
			entry := ProjectDatabases{Project: member.Name}
			if databases, exists := member.Value.Lookup("databases"); exists && !databases.IsNull() {
				entry.Databases, err = parseDatabases(databases)
				if err != nil {
					return DatabasesConfig{}, err
				}
			}
			config.Projects = append(config.Projects, entry)
		}
	}
	return config, nil
}

func loadDatabasesPathTolerant(path string) (DatabasesConfig, error) {
	document, err := readOrderedJSON(path)
	if err != nil {
		return DatabasesConfig{}, err
	}
	config := DatabasesConfig{document: retained(document)}
	config.SchemaURL, _ = objectString(&document, "$schema")
	config.Schema, _ = objectInt(&document, "schema")
	if defaults, ok := document.Lookup("defaults"); ok && defaults.Kind() == wirejson.Object {
		var options DatabaseDefaults
		if decodeValue(defaults, &options) == nil {
			config.Defaults = &options
		}
	}
	if globals, ok := document.Lookup("globals"); ok && !globals.IsNull() {
		if globals.Kind() != wirejson.Object {
			return DatabasesConfig{}, errors.New("config.databases-not-object")
		}
		config.Globals = parseDatabasesTolerant(globals)
	}
	if projects, ok := document.Lookup("projects"); ok && !projects.IsNull() {
		if projects.Kind() != wirejson.Object {
			return DatabasesConfig{}, errors.New("config.database-projects-not-object")
		}
		members, _ := effectiveMembers(projects)
		for _, member := range members {
			entry := ProjectDatabases{Project: member.Name}
			if databases, exists := member.Value.Lookup("databases"); exists && databases.Kind() == wirejson.Object {
				entry.Databases = parseDatabasesTolerant(databases)
			}
			config.Projects = append(config.Projects, entry)
		}
	}
	return config, nil
}

func parseDatabasesTolerant(value *wirejson.Value) []DatabaseEntry {
	members, _ := effectiveMembers(value)
	entries := make([]DatabaseEntry, 0, len(members))
	for _, member := range members {
		var database DatabaseConfig
		if decodeValue(&member.Value, &database) == nil {
			database.Unknown = unknownMembers(&member.Value, "provider", "connectionString", "connectionStringEnvironmentVariable", "credentialKey", "readonly", "maxRows", "timeoutSeconds")
		}
		entries = append(entries, DatabaseEntry{Key: member.Name, Database: database})
	}
	return entries
}

func parseDatabases(value *wirejson.Value) ([]DatabaseEntry, error) {
	members, ok := effectiveMembers(value)
	if !ok {
		return nil, errors.New("config.databases-not-object")
	}
	entries := make([]DatabaseEntry, 0, len(members))
	for _, member := range members {
		var database DatabaseConfig
		if err := decodeValue(&member.Value, &database); err != nil {
			return nil, err
		}
		database.Unknown = unknownMembers(&member.Value, "provider", "connectionString", "connectionStringEnvironmentVariable", "credentialKey", "readonly", "maxRows", "timeoutSeconds")
		entries = append(entries, DatabaseEntry{Key: member.Name, Database: database})
	}
	return entries, nil
}

func effectiveMembers(value *wirejson.Value) ([]wirejson.Member, bool) {
	members, ok := value.Members()
	if !ok {
		return nil, false
	}
	result := make([]wirejson.Member, 0, len(members))
	seen := make(map[string]struct{}, len(members))
	for _, member := range members {
		if _, exists := seen[member.Name]; exists {
			continue
		}
		seen[member.Name] = struct{}{}
		last, _ := value.Lookup(member.Name)
		result = append(result, wirejson.Member{Name: member.Name, Value: last.Clone()})
	}
	return result, true
}

func orderedStrings(value *wirejson.Value) ([]NamedString, error) {
	members, ok := effectiveMembers(value)
	if !ok {
		return nil, errors.New("config.named-strings-not-object")
	}
	result := make([]NamedString, 0, len(members))
	for _, member := range members {
		text, ok := member.Value.AsString()
		if !ok {
			return nil, errors.New("config.named-string-not-string")
		}
		result = append(result, NamedString{Name: member.Name, Value: text})
	}
	return result, nil
}

func unknownMembers(value *wirejson.Value, known ...string) []NamedRawConfiguration {
	members, ok := effectiveMembers(value)
	if !ok {
		return nil
	}
	result := make([]NamedRawConfiguration, 0)
	for _, member := range members {
		isKnown := false
		for _, name := range known {
			if member.Name == name {
				isKnown = true
				break
			}
		}
		if isKnown {
			continue
		}
		raw, err := wirejson.Compact(member.Value)
		if err == nil {
			result = append(result, NamedRawConfiguration{Name: member.Name, Raw: raw})
		}
	}
	return result
}

func decodeValue(value *wirejson.Value, target any) error {
	data, err := wirejson.Compact(value.Clone())
	if err != nil {
		return err
	}
	return json.Unmarshal(data, target)
}

func objectString(value *wirejson.Value, name string) (string, bool) {
	child, ok := value.Lookup(name)
	if !ok {
		return "", false
	}
	return child.AsString()
}

func objectInt(value *wirejson.Value, name string) (int, bool) {
	child, ok := value.Lookup(name)
	if !ok {
		return 0, false
	}
	lexeme, ok := child.AsNumber()
	if !ok {
		return 0, false
	}
	number, err := strconv.Atoi(lexeme)
	return number, err == nil
}

func optionalString(value *wirejson.Value, name string) *string {
	text, ok := objectString(value, name)
	if !ok {
		return nil
	}
	return &text
}

func retained(value wirejson.Value) *wirejson.Value {
	clone := value.Clone()
	return &clone
}

func (workflow WorkflowConfig) MarshalJSON() ([]byte, error) {
	if workflow.document != nil {
		return wirejson.Compact(workflow.document.Clone())
	}
	value, err := workflowWireValue(workflow)
	if err != nil {
		return nil, err
	}
	return wirejson.Compact(value)
}
func (projects ProjectsConfig) MarshalJSON() ([]byte, error) {
	if projects.document != nil {
		return wirejson.Compact(projects.document.Clone())
	}
	value, err := projectsWireValue(projects)
	if err != nil {
		return nil, err
	}
	return wirejson.Compact(value)
}
func (databases DatabasesConfig) MarshalJSON() ([]byte, error) {
	if databases.document != nil {
		return wirejson.Compact(databases.document.Clone())
	}
	value, err := databasesWireValue(databases)
	if err != nil {
		return nil, err
	}
	return wirejson.Compact(value)
}
